use std::path::Path;

use super::editor::extract_marker_content;
use super::parser::collect_local_packages;

/// Content to preserve from an existing flake
#[derive(Default, Clone)]
pub struct PreservedContent {
    pub custom_inputs: String,
    pub custom_packages: String,
    pub custom_paths: String,
}

impl PreservedContent {
    pub fn from_file(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        Self {
            custom_inputs: extract_marker_content(&content, "nixy:custom-inputs"),
            custom_packages: extract_marker_content(&content, "nixy:custom-packages"),
            custom_paths: extract_marker_content(&content, "nixy:custom-paths"),
        }
    }
}

/// Generate flake.nix content
pub fn generate_flake(
    packages: &[String],
    flake_dir: Option<&Path>,
    preserved: Option<&PreservedContent>,
) -> String {
    let preserved = preserved.cloned().unwrap_or_default();

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
    let filtered_packages: Vec<&String> = packages
        .iter()
        .filter(|pkg| {
            !local_packages.iter().any(|lp| &lp.name == *pkg)
                && !local_flakes.iter().any(|lf| &lf.name == *pkg)
        })
        .collect();

    // Build package entries
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

    // Build buildEnv paths
    let mut buildenv_paths: Vec<String> = filtered_packages.iter().map(|p| p.to_string()).collect();
    buildenv_paths.extend(local_packages.iter().map(|p| p.name.clone()));
    buildenv_paths.extend(local_flakes.iter().map(|f| f.name.clone()));

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
        format!(
            "overlays = [
          # [nixy:local-overlays]
{}          # [/nixy:local-overlays]
        ];",
            local_overlays
        )
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

    // Add trailing newlines to preserved content if non-empty
    let custom_inputs = if preserved.custom_inputs.is_empty() {
        String::new()
    } else {
        format!("{}\n", preserved.custom_inputs.trim_end())
    };

    let custom_packages = if preserved.custom_packages.is_empty() {
        String::new()
    } else {
        format!("{}\n", preserved.custom_packages.trim_end())
    };

    let custom_paths = if preserved.custom_paths.is_empty() {
        String::new()
    } else {
        format!("{}\n", preserved.custom_paths.trim_end())
    };

    // Add newlines to entries if non-empty
    let pkg_entries = if pkg_entries.is_empty() {
        String::new()
    } else {
        format!("{}\n", pkg_entries)
    };

    let local_inputs = if local_inputs.is_empty() {
        String::new()
    } else {
        local_inputs
    };

    let local_packages_entries = if local_packages_entries.is_empty() {
        String::new()
    } else {
        local_packages_entries
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
    # [nixy:local-inputs]
{local_inputs}    # [/nixy:local-inputs]
    # [nixy:custom-inputs]
{custom_inputs}    # [/nixy:custom-inputs]
  }};

  outputs = {{ {output_params} }}@inputs:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
      {pkgs_def}
    in {{
      # Profile packages (nixy install)
      packages = forAllSystems (system:
        {pkgs_binding}
        in rec {{
          # [nixy:packages]
{pkg_entries}          # [/nixy:packages]
          # [nixy:local-packages]
{local_packages_entries}          # [/nixy:local-packages]
          # [nixy:custom-packages]
{custom_packages}          # [/nixy:custom-packages]

          # Unified environment for atomic install (nixy sync)
          default = pkgs.buildEnv {{
            name = "nixy-env";
            paths = [
              # [nixy:env-paths]
{buildenv_paths_str}              # [/nixy:env-paths]
              # [nixy:custom-paths]
{custom_paths}              # [/nixy:custom-paths]
            ];
            extraOutputsToInstall = [ "man" "doc" "info" ];
          }};
        }});
    }};
}}
"#
    )
}

/// Check if flake has custom modifications outside nixy markers
pub fn has_custom_modifications(flake_path: &Path, packages: &[String], flake_dir: &Path) -> bool {
    let actual_content = match std::fs::read_to_string(flake_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let preserved = PreservedContent::from_file(flake_path);
    let clean_content = generate_flake(packages, Some(flake_dir), Some(&preserved));

    actual_content != clean_content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_empty_flake() {
        let flake = generate_flake(&[], None, None);

        // Should have required markers
        assert!(flake.contains("# [nixy:packages]"));
        assert!(flake.contains("# [/nixy:packages]"));
        assert!(flake.contains("# [nixy:local-packages]"));
        assert!(flake.contains("# [/nixy:local-packages]"));
        assert!(flake.contains("# [nixy:custom-packages]"));
        assert!(flake.contains("# [/nixy:custom-packages]"));
        assert!(flake.contains("# [nixy:env-paths]"));
        assert!(flake.contains("# [/nixy:env-paths]"));
        assert!(flake.contains("# [nixy:custom-paths]"));
        assert!(flake.contains("# [/nixy:custom-paths]"));
        assert!(flake.contains("# [nixy:local-inputs]"));
        assert!(flake.contains("# [/nixy:local-inputs]"));
        assert!(flake.contains("# [nixy:custom-inputs]"));
        assert!(flake.contains("# [/nixy:custom-inputs]"));

        // Should have buildEnv
        assert!(flake.contains("default = pkgs.buildEnv"));
        assert!(flake.contains("name = \"nixy-env\""));
        assert!(flake.contains("extraOutputsToInstall"));

        // Should NOT have devShells
        assert!(!flake.contains("devShells"));
    }

    #[test]
    fn test_generate_flake_with_packages() {
        let packages = vec!["ripgrep".to_string(), "fzf".to_string(), "bat".to_string()];
        let flake = generate_flake(&packages, None, None);

        // Should have package entries
        assert!(flake.contains("ripgrep = pkgs.ripgrep;"));
        assert!(flake.contains("fzf = pkgs.fzf;"));
        assert!(flake.contains("bat = pkgs.bat;"));

        // Should have packages in env-paths
        let env_paths_section =
            extract_section(&flake, "# [nixy:env-paths]", "# [/nixy:env-paths]");
        assert!(env_paths_section.contains("ripgrep"));
        assert!(env_paths_section.contains("fzf"));
        assert!(env_paths_section.contains("bat"));
    }

    #[test]
    fn test_generate_flake_preserves_custom_content() {
        let preserved = PreservedContent {
            custom_inputs: "    my-overlay.url = \"github:user/repo\";".to_string(),
            custom_packages: "          my-pkg = pkgs.hello;".to_string(),
            custom_paths: "              my-pkg".to_string(),
        };

        let flake = generate_flake(&[], None, Some(&preserved));

        // Should preserve custom content
        assert!(flake.contains("my-overlay.url = \"github:user/repo\""));
        assert!(flake.contains("my-pkg = pkgs.hello"));

        // Check custom-paths section
        let custom_paths_section =
            extract_section(&flake, "# [nixy:custom-paths]", "# [/nixy:custom-paths]");
        assert!(custom_paths_section.contains("my-pkg"));
    }

    #[test]
    fn test_flake_has_correct_nixpkgs_url() {
        let flake = generate_flake(&[], None, None);
        assert!(flake.contains("nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\""));
    }

    #[test]
    fn test_flake_has_all_systems() {
        let flake = generate_flake(&[], None, None);
        assert!(flake.contains("x86_64-linux"));
        assert!(flake.contains("aarch64-linux"));
        assert!(flake.contains("x86_64-darwin"));
        assert!(flake.contains("aarch64-darwin"));
    }

    #[test]
    fn test_flake_uses_legacy_packages() {
        let flake = generate_flake(&[], None, None);
        assert!(flake.contains("nixpkgs.legacyPackages.${system}"));
    }

    #[test]
    fn test_buildenv_has_extra_outputs() {
        let flake = generate_flake(&[], None, None);
        assert!(flake.contains("extraOutputsToInstall = [ \"man\" \"doc\" \"info\" ]"));
    }

    fn extract_section(content: &str, start: &str, end: &str) -> String {
        let mut in_section = false;
        let mut result = String::new();

        for line in content.lines() {
            if line.contains(end) {
                break;
            }
            if in_section {
                result.push_str(line);
                result.push('\n');
            }
            if line.contains(start) {
                in_section = true;
            }
        }

        result
    }
}
