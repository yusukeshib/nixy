use std::path::Path;

use super::parser::collect_local_packages;
use crate::state::PackageState;

/// Generate flake.nix content from package state
pub fn generate_flake(state: &PackageState, flake_dir: Option<&Path>) -> String {
    // Collect local packages if flake_dir is provided
    let (local_packages, local_flakes) = if let Some(dir) = flake_dir {
        let packages_dir = dir.join("packages");
        if packages_dir.exists() {
            collect_local_packages(&packages_dir)
        } else {
            (Vec::new(), Vec::new())
        }
    } else {
        (Vec::new(), Vec::new())
    };

    // Filter out local packages from standard packages list
    let filtered_packages: Vec<&String> = state
        .packages
        .iter()
        .filter(|pkg| {
            !local_packages.iter().any(|lp| &lp.name == *pkg)
                && !local_flakes.iter().any(|lf| &lf.name == *pkg)
        })
        .collect();

    // Build package entries for standard packages
    let pkg_entries: String = filtered_packages
        .iter()
        .map(|pkg| format!("          {} = pkgs.{};", pkg, pkg))
        .collect::<Vec<_>>()
        .join("\n");

    // Build local inputs
    let mut local_inputs = String::new();
    let mut local_input_params = Vec::new();
    let mut local_overlays = String::new();
    let mut local_packages_entries = String::new();

    // Handle flake-type packages
    for flake in &local_flakes {
        local_inputs.push_str(&format!(
            "    {}.url = \"path:./packages/{}\";\n",
            flake.name, flake.name
        ));
        local_input_params.push(flake.name.clone());
        local_packages_entries.push_str(&format!(
            "          {} = inputs.{}.packages.${{system}}.default;\n",
            flake.name, flake.name
        ));
    }

    // Handle regular local packages
    for pkg in &local_packages {
        if let (Some(input_name), Some(input_url)) = (&pkg.input_name, &pkg.input_url) {
            local_inputs.push_str(&format!("    {}.url = \"{}\";\n", input_name, input_url));
            local_input_params.push(input_name.clone());
        }

        if let Some(overlay) = &pkg.overlay {
            local_overlays.push_str(&format!("          {}\n", overlay));
        }

        local_packages_entries
            .push_str(&format!("          {} = {};\n", pkg.name, pkg.package_expr));
    }

    // Build custom inputs and packages from state
    let mut custom_inputs = String::new();
    let mut custom_packages_entries = String::new();

    for pkg in &state.custom_packages {
        // Add the input if not already present from local packages
        if !local_input_params.contains(&pkg.input_name)
            && !custom_inputs.contains(&format!("{}.", pkg.input_name))
        {
            custom_inputs.push_str(&format!(
                "    {}.url = \"{}\";\n",
                pkg.input_name, pkg.input_url
            ));
            local_input_params.push(pkg.input_name.clone());
        }

        custom_packages_entries.push_str(&format!(
            "          {} = inputs.{}.{}.${{system}}.{};\n",
            pkg.name, pkg.input_name, pkg.package_output, pkg.name
        ));
    }

    // Build buildEnv paths
    let mut buildenv_paths: Vec<String> = filtered_packages.iter().map(|p| p.to_string()).collect();
    buildenv_paths.extend(local_packages.iter().map(|p| p.name.clone()));
    buildenv_paths.extend(local_flakes.iter().map(|f| f.name.clone()));
    buildenv_paths.extend(state.custom_packages.iter().map(|p| p.name.clone()));

    let buildenv_paths_str: String = buildenv_paths
        .iter()
        .map(|p| format!("              {}", p))
        .collect::<Vec<_>>()
        .join("\n");

    // Build output parameters
    let output_params = if local_input_params.is_empty() {
        "self, nixpkgs".to_string()
    } else {
        format!("self, nixpkgs, {}", local_input_params.join(", "))
    };

    // Build overlays section
    let overlays_content = if !local_overlays.is_empty() {
        format!("overlays = [\n{}        ];", local_overlays)
    } else {
        String::new()
    };

    // Build pkgs definition
    let pkgs_def = if !local_overlays.is_empty() {
        format!(
            "pkgsFor = system: import nixpkgs {{
        inherit system;
        {}
      }};
",
            overlays_content
        )
    } else {
        String::new()
    };

    let pkgs_binding = if !local_overlays.is_empty() {
        "let pkgs = pkgsFor system;"
    } else {
        "let pkgs = nixpkgs.legacyPackages.${system};"
    };

    // Format sections with proper trailing newlines
    let all_inputs = format!("{}{}", local_inputs, custom_inputs);
    let all_inputs = if all_inputs.is_empty() {
        String::new()
    } else {
        all_inputs
    };

    let pkg_entries = if pkg_entries.is_empty() {
        String::new()
    } else {
        format!("{}\n", pkg_entries)
    };

    let local_packages_entries = if local_packages_entries.is_empty() {
        String::new()
    } else {
        local_packages_entries
    };

    let custom_packages_entries = if custom_packages_entries.is_empty() {
        String::new()
    } else {
        custom_packages_entries
    };

    let buildenv_paths_str = if buildenv_paths_str.is_empty() {
        String::new()
    } else {
        format!("{}\n", buildenv_paths_str)
    };

    format!(
        r#"{{
  description = "nixy managed packages";

  inputs = {{
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
{all_inputs}  }};

  outputs = {{ {output_params} }}@inputs:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
      {pkgs_def}
    in {{
      packages = forAllSystems (system:
        {pkgs_binding}
        in rec {{
{pkg_entries}{local_packages_entries}{custom_packages_entries}
          default = pkgs.buildEnv {{
            name = "nixy-env";
            paths = [
{buildenv_paths_str}            ];
            extraOutputsToInstall = [ "man" "doc" "info" ];
          }};
        }});
    }};
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::CustomPackage;

    #[test]
    fn test_generate_empty_flake() {
        let state = PackageState::default();
        let flake = generate_flake(&state, None);

        // Should have buildEnv
        assert!(flake.contains("default = pkgs.buildEnv"));
        assert!(flake.contains("name = \"nixy-env\""));
        assert!(flake.contains("extraOutputsToInstall"));

        // Should NOT have markers
        assert!(!flake.contains("# [nixy:"));
        assert!(!flake.contains("# [/nixy:"));

        // Should NOT have devShells
        assert!(!flake.contains("devShells"));
    }

    #[test]
    fn test_generate_flake_with_packages() {
        let mut state = PackageState::default();
        state.add_package("ripgrep");
        state.add_package("fzf");
        state.add_package("bat");

        let flake = generate_flake(&state, None);

        // Should have package entries
        assert!(flake.contains("ripgrep = pkgs.ripgrep;"));
        assert!(flake.contains("fzf = pkgs.fzf;"));
        assert!(flake.contains("bat = pkgs.bat;"));

        // Should have packages in paths
        assert!(flake.contains("ripgrep"));
        assert!(flake.contains("fzf"));
        assert!(flake.contains("bat"));
    }

    #[test]
    fn test_generate_flake_with_custom_packages() {
        let mut state = PackageState::default();
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
        });

        let flake = generate_flake(&state, None);

        // Should have custom input
        assert!(
            flake.contains("neovim-nightly.url = \"github:nix-community/neovim-nightly-overlay\"")
        );

        // Should have custom package entry
        assert!(flake.contains("neovim = inputs.neovim-nightly.packages.${system}.neovim;"));

        // Should have neovim in paths
        assert!(flake.contains("neovim"));
    }

    #[test]
    fn test_flake_has_correct_nixpkgs_url() {
        let state = PackageState::default();
        let flake = generate_flake(&state, None);
        assert!(flake.contains("nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\""));
    }

    #[test]
    fn test_flake_has_all_systems() {
        let state = PackageState::default();
        let flake = generate_flake(&state, None);
        assert!(flake.contains("x86_64-linux"));
        assert!(flake.contains("aarch64-linux"));
        assert!(flake.contains("x86_64-darwin"));
        assert!(flake.contains("aarch64-darwin"));
    }

    #[test]
    fn test_flake_uses_legacy_packages() {
        let state = PackageState::default();
        let flake = generate_flake(&state, None);
        assert!(flake.contains("nixpkgs.legacyPackages.${system}"));
    }

    #[test]
    fn test_buildenv_has_extra_outputs() {
        let state = PackageState::default();
        let flake = generate_flake(&state, None);
        assert!(flake.contains("extraOutputsToInstall = [ \"man\" \"doc\" \"info\" ]"));
    }

    #[test]
    fn test_flake_has_no_devshells() {
        let mut state = PackageState::default();
        state.add_package("ripgrep");
        let flake = generate_flake(&state, None);

        // Flakes should NOT have devShells
        assert!(!flake.contains("devShells"));
        // But should have packages section
        assert!(flake.contains("packages = forAllSystems"));
    }

    #[test]
    fn test_flake_has_no_markers() {
        let mut state = PackageState::default();
        state.add_package("hello");
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
        });

        let flake = generate_flake(&state, None);

        // Should NOT have any markers
        assert!(!flake.contains("# [nixy:"));
        assert!(!flake.contains("# [/nixy:"));
    }

    #[test]
    fn test_multiple_custom_packages_share_input() {
        let mut state = PackageState::default();
        state.add_custom_package(CustomPackage {
            name: "hello".to_string(),
            input_name: "nixpkgs-unstable".to_string(),
            input_url: "github:NixOS/nixpkgs/nixos-unstable".to_string(),
            package_output: "legacyPackages".to_string(),
        });
        state.add_custom_package(CustomPackage {
            name: "world".to_string(),
            input_name: "nixpkgs-unstable".to_string(),
            input_url: "github:NixOS/nixpkgs/nixos-unstable".to_string(),
            package_output: "legacyPackages".to_string(),
        });

        let flake = generate_flake(&state, None);

        // Input should only appear once
        let count = flake.matches("nixpkgs-unstable.url").count();
        assert_eq!(count, 1, "Input should only appear once");
    }

    #[test]
    fn test_buildenv_contains_all_packages() {
        let mut state = PackageState::default();
        state.add_package("ripgrep");
        state.add_package("fzf");
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
        });

        let flake = generate_flake(&state, None);

        // Extract paths section
        let paths_start = flake.find("paths = [").unwrap();
        let paths_end = flake[paths_start..].find("];").unwrap();
        let paths_section = &flake[paths_start..paths_start + paths_end];

        assert!(paths_section.contains("ripgrep"));
        assert!(paths_section.contains("fzf"));
        assert!(paths_section.contains("neovim"));
    }

    #[test]
    fn test_empty_flake_has_empty_buildenv() {
        let state = PackageState::default();
        let flake = generate_flake(&state, None);

        // Empty flake should have buildEnv structure with empty paths
        assert!(flake.contains("default = pkgs.buildEnv"));
        assert!(flake.contains("paths = ["));
        assert!(flake.contains("extraOutputsToInstall = [ \"man\" \"doc\" \"info\" ]"));
    }
}
