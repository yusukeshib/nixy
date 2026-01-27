//! Flake.nix template generation.
//!
//! This module generates `flake.nix` content from the package state. It handles:
//! - Standard nixpkgs packages
//! - Custom packages from external flakes
//! - Local packages (`.nix` files in `packages/` directory)
//! - Local flakes (subdirectories with `flake.nix`)
//!
//! The generated flake uses `buildEnv` to create a unified environment with
//! all installed packages.

use std::collections::HashSet;
use std::path::Path;

use super::parser::collect_local_packages;
use super::{LocalFlake, LocalPackage};
use crate::state::{CustomPackage, PackageState};

/// Intermediate representation for building flake content
struct FlakeBuilder {
    /// Additional flake inputs (beyond nixpkgs)
    inputs: String,
    /// Set of input names already added
    seen_inputs: HashSet<String>,
    /// Overlay expressions for pkgs customization
    overlays: String,
    /// Standard package entries (pkg = pkgs.pkg)
    standard_entries: String,
    /// Local package entries
    local_entries: String,
    /// Custom package entries from external flakes
    custom_entries: String,
    /// Package names for buildEnv paths
    buildenv_paths: Vec<String>,
}

impl FlakeBuilder {
    fn new() -> Self {
        Self {
            inputs: String::new(),
            seen_inputs: HashSet::new(),
            overlays: String::new(),
            standard_entries: String::new(),
            local_entries: String::new(),
            custom_entries: String::new(),
            buildenv_paths: Vec::new(),
        }
    }

    /// Add standard nixpkgs packages
    fn add_standard_packages(&mut self, packages: &[&String]) {
        let entries: Vec<String> = packages
            .iter()
            .map(|pkg| format!("          {} = pkgs.{};", pkg, pkg))
            .collect();

        if !entries.is_empty() {
            self.standard_entries = format!("{}\n", entries.join("\n"));
        }

        self.buildenv_paths
            .extend(packages.iter().map(|p| p.to_string()));
    }

    /// Add local flake-type packages from packages/ directory
    fn add_local_flakes(&mut self, flakes: &[LocalFlake]) {
        for flake in flakes {
            self.inputs.push_str(&format!(
                "    {}.url = \"path:./packages/{}\";\n",
                flake.name, flake.name
            ));
            self.seen_inputs.insert(flake.name.clone());
            self.local_entries.push_str(&format!(
                "          {} = inputs.{}.packages.${{system}}.default;\n",
                flake.name, flake.name
            ));
            self.buildenv_paths.push(flake.name.clone());
        }
    }

    /// Add local .nix file packages from packages/ directory
    fn add_local_packages(&mut self, packages: &[LocalPackage]) {
        for pkg in packages {
            if let (Some(input_name), Some(input_url)) = (&pkg.input_name, &pkg.input_url) {
                if self.seen_inputs.insert(input_name.clone()) {
                    self.inputs
                        .push_str(&format!("    {}.url = \"{}\";\n", input_name, input_url));
                }
            }

            if let Some(overlay) = &pkg.overlay {
                self.overlays.push_str(&format!("          {}\n", overlay));
            }

            self.local_entries
                .push_str(&format!("          {} = {};\n", pkg.name, pkg.package_expr));
            self.buildenv_paths.push(pkg.name.clone());
        }
    }

    /// Add custom packages from external flakes
    fn add_custom_packages(&mut self, packages: &[CustomPackage]) {
        for pkg in packages {
            if self.seen_inputs.insert(pkg.input_name.clone()) {
                self.inputs.push_str(&format!(
                    "    {}.url = \"{}\";\n",
                    pkg.input_name, pkg.input_url
                ));
            }

            self.custom_entries.push_str(&format!(
                "          {} = inputs.{}.{}.${{system}}.{};\n",
                pkg.name,
                pkg.input_name,
                pkg.package_output,
                pkg.source_package_name()
            ));
            self.buildenv_paths.push(pkg.name.clone());
        }
    }

    /// Build the output function parameters
    fn build_output_params(&self) -> String {
        if self.seen_inputs.is_empty() {
            "self, nixpkgs".to_string()
        } else {
            let mut inputs_list: Vec<_> = self.seen_inputs.iter().cloned().collect();
            inputs_list.sort();
            format!("self, nixpkgs, {}", inputs_list.join(", "))
        }
    }

    /// Build the pkgs definition (with or without overlays)
    fn build_pkgs_definition(&self) -> (String, &'static str) {
        if self.overlays.is_empty() {
            (
                String::new(),
                "let pkgs = nixpkgs.legacyPackages.${system};",
            )
        } else {
            let overlays_content = format!("overlays = [\n{}        ];", self.overlays);
            let pkgs_def = format!(
                "pkgsFor = system: import nixpkgs {{
        inherit system;
        {}
      }};
",
                overlays_content
            );
            (pkgs_def, "let pkgs = pkgsFor system;")
        }
    }

    /// Build the buildEnv paths section
    fn build_paths_section(&self) -> String {
        if self.buildenv_paths.is_empty() {
            String::new()
        } else {
            let paths: Vec<String> = self
                .buildenv_paths
                .iter()
                .map(|p| format!("              {}", p))
                .collect();
            format!("{}\n", paths.join("\n"))
        }
    }

    /// Generate the final flake.nix content
    fn build(self) -> String {
        let output_params = self.build_output_params();
        let (pkgs_def, pkgs_binding) = self.build_pkgs_definition();
        let buildenv_paths_str = self.build_paths_section();

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
{pkg_entries}{local_entries}{custom_entries}
          default = pkgs.buildEnv {{
            name = "nixy-env";
            paths = [
{buildenv_paths_str}            ];
            extraOutputsToInstall = [ "man" "doc" "info" ];
          }};
        }});
    }};
}}
"#,
            all_inputs = self.inputs,
            output_params = output_params,
            pkgs_def = pkgs_def,
            pkgs_binding = pkgs_binding,
            pkg_entries = self.standard_entries,
            local_entries = self.local_entries,
            custom_entries = self.custom_entries,
            buildenv_paths_str = buildenv_paths_str,
        )
    }
}

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

    let mut builder = FlakeBuilder::new();
    builder.add_standard_packages(&filtered_packages);
    builder.add_local_flakes(&local_flakes);
    builder.add_local_packages(&local_packages);
    builder.add_custom_packages(&state.custom_packages);
    builder.build()
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
            source_name: None,
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
            source_name: None,
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
            source_name: None,
        });
        state.add_custom_package(CustomPackage {
            name: "world".to_string(),
            input_name: "nixpkgs-unstable".to_string(),
            input_url: "github:NixOS/nixpkgs/nixos-unstable".to_string(),
            package_output: "legacyPackages".to_string(),
            source_name: None,
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
            source_name: None,
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
