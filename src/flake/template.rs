//! Flake.nix template generation.
//!
//! This module generates `flake.nix` content from the package state or profile config.
//! It handles:
//! - Standard nixpkgs packages
//! - Custom packages from external flakes
//! - Local packages (`.nix` files in `packages/` directory)
//! - Local flakes (subdirectories with `flake.nix`)
//!
//! The generated flake uses `buildEnv` to create a unified environment with
//! all installed packages.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::parser::collect_local_packages;
use super::{LocalFlake, LocalPackage};
use crate::error::Result;
use crate::nixy_config::ProfileConfig;
use crate::state::{CustomPackage, PackageState, ResolvedNixpkgPackage};

/// A package path entry with optional platform restrictions
struct PathEntry {
    /// Package name (variable name in the flake)
    name: String,
    /// Platform restrictions (None means all platforms)
    platforms: Option<Vec<String>>,
}

/// Intermediate representation for building flake content
struct FlakeBuilder {
    /// Additional flake inputs (beyond nixpkgs)
    inputs: String,
    /// Set of input names already added
    seen_inputs: HashSet<String>,
    /// Overlay expressions for pkgs customization
    overlays: String,
    /// Standard package entries (pkg = pkgs.pkg) - legacy packages
    standard_entries: String,
    /// Resolved package entries from Nixhub (with specific nixpkgs commits)
    resolved_entries: String,
    /// Local package entries
    local_entries: String,
    /// Custom package entries from external flakes
    custom_entries: String,
    /// Package names for buildEnv paths with platform restrictions
    buildenv_paths: Vec<PathEntry>,
}

impl FlakeBuilder {
    fn new() -> Self {
        Self {
            inputs: String::new(),
            seen_inputs: HashSet::new(),
            overlays: String::new(),
            standard_entries: String::new(),
            resolved_entries: String::new(),
            local_entries: String::new(),
            custom_entries: String::new(),
            buildenv_paths: Vec::new(),
        }
    }

    /// Add standard nixpkgs packages (legacy, from default nixpkgs)
    fn add_standard_packages(&mut self, packages: &[&String]) {
        let entries: Vec<String> = packages
            .iter()
            .map(|pkg| format!("          {} = pkgs.{};", pkg, pkg))
            .collect();

        if !entries.is_empty() {
            self.standard_entries = format!("{}\n", entries.join("\n"));
        }

        self.buildenv_paths
            .extend(packages.iter().map(|p| PathEntry {
                name: p.to_string(),
                platforms: None,
            }));
    }

    /// Add resolved nixpkgs packages (with specific commits from Nixhub)
    fn add_resolved_packages(&mut self, packages: &[ResolvedNixpkgPackage]) {
        if packages.is_empty() {
            return;
        }

        // Group packages by commit hash
        let mut by_commit: HashMap<&str, Vec<&ResolvedNixpkgPackage>> = HashMap::new();
        for pkg in packages {
            by_commit.entry(&pkg.commit_hash).or_default().push(pkg);
        }

        // Add inputs and entries for each commit
        for (commit, pkgs) in &by_commit {
            let input_name = format!("nixpkgs-{}", &commit[..8.min(commit.len())]);

            // Add input if not already seen
            if self.seen_inputs.insert(input_name.clone()) {
                self.inputs.push_str(&format!(
                    "    {}.url = \"github:NixOS/nixpkgs/{}\";\n",
                    input_name, commit
                ));
            }

            // Add package entries
            for pkg in pkgs {
                self.resolved_entries.push_str(&format!(
                    "          {} = inputs.{}.legacyPackages.${{system}}.{};\n",
                    pkg.name, input_name, pkg.attribute_path
                ));
                self.buildenv_paths.push(PathEntry {
                    name: pkg.name.clone(),
                    platforms: pkg.platforms.clone(),
                });
            }
        }
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
            self.buildenv_paths.push(PathEntry {
                name: flake.name.clone(),
                platforms: None,
            });
        }
    }

    /// Add local flake-type packages with absolute paths (for new nixy.json format)
    fn add_local_flakes_with_absolute_paths(
        &mut self,
        flakes: &[LocalFlake],
        packages_dir: Option<&Path>,
    ) {
        for flake in flakes {
            let path = if let Some(dir) = packages_dir {
                // Use URL-encoded path for flake URLs to handle spaces and special characters
                let abs_path = dir.join(&flake.name);
                let path_str = abs_path.to_string_lossy();
                // Escape spaces and special characters in the path for flake URL
                let escaped_path = path_str.replace(' ', "%20");
                format!("path:{}", escaped_path)
            } else {
                format!("path:./packages/{}", flake.name)
            };
            self.inputs
                .push_str(&format!("    {}.url = \"{}\";\n", flake.name, path));
            self.seen_inputs.insert(flake.name.clone());
            self.local_entries.push_str(&format!(
                "          {} = inputs.{}.packages.${{system}}.default;\n",
                flake.name, flake.name
            ));
            self.buildenv_paths.push(PathEntry {
                name: flake.name.clone(),
                platforms: None,
            });
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
            self.buildenv_paths.push(PathEntry {
                name: pkg.name.clone(),
                platforms: None,
            });
        }
    }

    /// Add local .nix file packages with absolute paths (for new nixy.json format)
    fn add_local_packages_with_absolute_paths(
        &mut self,
        packages: &[LocalPackage],
        packages_dir: Option<&Path>,
    ) {
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

            // Update package expression to use absolute path if needed
            let package_expr = if let Some(dir) = packages_dir {
                let abs_path = dir.join(format!("{}.nix", pkg.name));
                let path_str = abs_path.to_string_lossy();
                // Only replace if the expression is a simple ./packages/<name>.nix reference
                if pkg.package_expr == format!("pkgs.callPackage ./packages/{}.nix {{}}", pkg.name)
                {
                    // Use Nix path syntax with proper escaping for paths with spaces
                    if path_str.contains(' ') {
                        // For paths with spaces, use a quoted string path
                        format!("pkgs.callPackage /. + \"{}\" {{}}", path_str)
                    } else {
                        format!("pkgs.callPackage {} {{}}", path_str)
                    }
                } else {
                    pkg.package_expr.clone()
                }
            } else {
                pkg.package_expr.clone()
            };

            self.local_entries
                .push_str(&format!("          {} = {};\n", pkg.name, package_expr));
            self.buildenv_paths.push(PathEntry {
                name: pkg.name.clone(),
                platforms: None,
            });
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
            self.buildenv_paths.push(PathEntry {
                name: pkg.name.clone(),
                platforms: pkg.platforms.clone(),
            });
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

    /// Generate the final flake.nix content
    fn build(self) -> String {
        let output_params = self.build_output_params();
        let (pkgs_def, pkgs_binding) = self.build_pkgs_definition();
        let (paths_content, _has_platform_conditionals) = self.build_paths_section_with_info();

        let paths_section = format!("paths = [\n{}            ];", paths_content);

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
{pkg_entries}{resolved_entries}{local_entries}{custom_entries}
          default = pkgs.buildEnv {{
            name = "nixy-env";
            {paths_section}
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
            resolved_entries = self.resolved_entries,
            local_entries = self.local_entries,
            custom_entries = self.custom_entries,
            paths_section = paths_section,
        )
    }

    /// Build the buildEnv paths section and return whether it has platform conditionals
    fn build_paths_section_with_info(&self) -> (String, bool) {
        if self.buildenv_paths.is_empty() {
            return (String::new(), false);
        }

        // Group packages by their platform restrictions
        // None means all platforms, Some([...]) means specific platforms
        let mut universal: Vec<&str> = Vec::new();
        let mut by_platforms: HashMap<Vec<String>, Vec<&str>> = HashMap::new();

        for entry in &self.buildenv_paths {
            match &entry.platforms {
                None => universal.push(&entry.name),
                Some(platforms) => {
                    let mut sorted_platforms = platforms.clone();
                    sorted_platforms.sort();
                    by_platforms
                        .entry(sorted_platforms)
                        .or_default()
                        .push(&entry.name);
                }
            }
        }

        let has_conditionals = !by_platforms.is_empty();
        let mut result = String::new();

        // Add universal packages (no platform restriction)
        for pkg in &universal {
            result.push_str(&format!("              {}\n", pkg));
        }

        // Add platform-specific packages with lib.optionals
        let mut platform_groups: Vec<_> = by_platforms.into_iter().collect();
        platform_groups.sort_by(|a, b| a.0.cmp(&b.0));

        for (platforms, packages) in platform_groups {
            let platforms_str = platforms
                .iter()
                .map(|p| format!("\"{}\"", p))
                .collect::<Vec<_>>()
                .join(" ");
            let packages_str = packages
                .iter()
                .map(|p| format!("\n                {}", p))
                .collect::<Vec<_>>()
                .join("");
            result.push_str(&format!(
                "            ] ++ pkgs.lib.optionals (builtins.elem system [ {} ]) [{}\n",
                platforms_str, packages_str
            ));
        }

        (result, has_conditionals)
    }
}

/// Generate flake.nix content from package state
///
/// # Arguments
/// * `state` - The package state (legacy format)
/// * `flake_dir` - Optional flake directory for collecting local packages (legacy)
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

    // Filter out local packages from legacy packages list
    let filtered_legacy_packages: Vec<&String> = state
        .packages
        .iter()
        .filter(|pkg| {
            !local_packages.iter().any(|lp| &lp.name == *pkg)
                && !local_flakes.iter().any(|lf| &lf.name == *pkg)
        })
        .collect();

    // Filter out local packages from resolved packages list
    let filtered_resolved_packages: Vec<ResolvedNixpkgPackage> = state
        .resolved_packages
        .iter()
        .filter(|pkg| {
            !local_packages.iter().any(|lp| lp.name == pkg.name)
                && !local_flakes.iter().any(|lf| lf.name == pkg.name)
        })
        .cloned()
        .collect();

    let mut builder = FlakeBuilder::new();
    builder.add_standard_packages(&filtered_legacy_packages);
    builder.add_resolved_packages(&filtered_resolved_packages);
    builder.add_local_flakes(&local_flakes);
    builder.add_local_packages(&local_packages);
    builder.add_custom_packages(&state.custom_packages);
    builder.build()
}

/// Generate flake.nix content from profile config
///
/// # Arguments
/// * `profile` - The profile configuration (new nixy.json format)
/// * `global_packages_dir` - Optional global packages directory for local packages
/// * `flake_dir` - The flake directory (state directory) for relative path references
pub fn generate_flake_from_profile(
    profile: &ProfileConfig,
    global_packages_dir: Option<&Path>,
    flake_dir: &Path,
) -> String {
    // Collect local packages from global packages directory
    let (local_packages, local_flakes) = if let Some(dir) = global_packages_dir {
        if dir.exists() {
            collect_local_packages_with_paths(dir, flake_dir)
        } else {
            (Vec::new(), Vec::new())
        }
    } else {
        (Vec::new(), Vec::new())
    };

    // Filter out local packages from legacy packages list
    let filtered_legacy_packages: Vec<&String> = profile
        .packages
        .iter()
        .filter(|pkg| {
            !local_packages.iter().any(|lp| &lp.name == *pkg)
                && !local_flakes.iter().any(|lf| &lf.name == *pkg)
        })
        .collect();

    // Filter out local packages from resolved packages list
    let filtered_resolved_packages: Vec<ResolvedNixpkgPackage> = profile
        .resolved_packages
        .iter()
        .filter(|pkg| {
            !local_packages.iter().any(|lp| lp.name == pkg.name)
                && !local_flakes.iter().any(|lf| lf.name == pkg.name)
        })
        .cloned()
        .collect();

    let mut builder = FlakeBuilder::new();
    builder.add_standard_packages(&filtered_legacy_packages);
    builder.add_resolved_packages(&filtered_resolved_packages);
    builder.add_local_flakes_with_absolute_paths(&local_flakes, global_packages_dir);
    builder.add_local_packages_with_absolute_paths(&local_packages, global_packages_dir);
    builder.add_custom_packages(&profile.custom_packages);
    builder.build()
}

/// Collect local packages with paths resolved relative to the flake directory
fn collect_local_packages_with_paths(
    packages_dir: &Path,
    _flake_dir: &Path,
) -> (Vec<LocalPackage>, Vec<LocalFlake>) {
    collect_local_packages(packages_dir)
}

/// Regenerate flake.nix from state (legacy format)
pub fn regenerate_flake(flake_dir: &Path, state: &PackageState) -> Result<()> {
    let flake_path = flake_dir.join("flake.nix");
    fs::create_dir_all(flake_dir)?;
    let content = generate_flake(state, Some(flake_dir));
    fs::write(&flake_path, content)?;
    Ok(())
}

/// Regenerate flake.nix from profile config (new nixy.json format)
pub fn regenerate_flake_from_profile(
    flake_dir: &Path,
    profile: &ProfileConfig,
    global_packages_dir: Option<&Path>,
) -> Result<()> {
    let flake_path = flake_dir.join("flake.nix");
    fs::create_dir_all(flake_dir)?;
    let content = generate_flake_from_profile(profile, global_packages_dir, flake_dir);
    fs::write(&flake_path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CustomPackage, ResolvedNixpkgPackage};

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
            platforms: None,
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
            platforms: None,
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
            platforms: None,
        });
        state.add_custom_package(CustomPackage {
            name: "world".to_string(),
            input_name: "nixpkgs-unstable".to_string(),
            input_url: "github:NixOS/nixpkgs/nixos-unstable".to_string(),
            package_output: "legacyPackages".to_string(),
            source_name: None,
            platforms: None,
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
            platforms: None,
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

    #[test]
    fn test_generate_flake_with_resolved_packages() {
        let mut state = PackageState::default();
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123def456".to_string(),
            platforms: None,
        });

        let flake = generate_flake(&state, None);

        // Should have nixpkgs input with commit hash
        assert!(flake.contains("nixpkgs-abc123de.url = \"github:NixOS/nixpkgs/abc123def456\""));

        // Should have package entry using attribute_path
        assert!(
            flake.contains("nodejs = inputs.nixpkgs-abc123de.legacyPackages.${system}.nodejs_20;")
        );

        // Should have nodejs in paths
        let paths_start = flake.find("paths = [").unwrap();
        let paths_end = flake[paths_start..].find("];").unwrap();
        let paths_section = &flake[paths_start..paths_start + paths_end];
        assert!(paths_section.contains("nodejs"));
    }

    #[test]
    fn test_resolved_packages_share_commit_input() {
        let mut state = PackageState::default();
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123def456".to_string(),
            platforms: None,
        });
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "python".to_string(),
            version_spec: Some("3.11".to_string()),
            resolved_version: "3.11.5".to_string(),
            attribute_path: "python311".to_string(),
            commit_hash: "abc123def456".to_string(), // Same commit
            platforms: None,
        });

        let flake = generate_flake(&state, None);

        // Input should only appear once
        let count = flake.matches("nixpkgs-abc123de.url").count();
        assert_eq!(count, 1, "Same commit input should only appear once");

        // Both packages should use the same input
        assert!(
            flake.contains("nodejs = inputs.nixpkgs-abc123de.legacyPackages.${system}.nodejs_20;")
        );
        assert!(
            flake.contains("python = inputs.nixpkgs-abc123de.legacyPackages.${system}.python311;")
        );
    }

    #[test]
    fn test_resolved_packages_different_commits() {
        let mut state = PackageState::default();
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123def456".to_string(),
            platforms: None,
        });
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "python".to_string(),
            version_spec: Some("3.11".to_string()),
            resolved_version: "3.11.5".to_string(),
            attribute_path: "python311".to_string(),
            commit_hash: "xyz789ghi012".to_string(), // Different commit
            platforms: None,
        });

        let flake = generate_flake(&state, None);

        // Should have two different nixpkgs inputs
        assert!(flake.contains("nixpkgs-abc123de.url = \"github:NixOS/nixpkgs/abc123def456\""));
        assert!(flake.contains("nixpkgs-xyz789gh.url = \"github:NixOS/nixpkgs/xyz789ghi012\""));

        // Each package should use its own input
        assert!(
            flake.contains("nodejs = inputs.nixpkgs-abc123de.legacyPackages.${system}.nodejs_20;")
        );
        assert!(
            flake.contains("python = inputs.nixpkgs-xyz789gh.legacyPackages.${system}.python311;")
        );
    }

    #[test]
    fn test_mixed_legacy_and_resolved_packages() {
        let mut state = PackageState::default();
        // Legacy package (uses default nixpkgs)
        state.add_package("ripgrep");
        // Resolved package (uses specific commit)
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123def456".to_string(),
            platforms: None,
        });

        let flake = generate_flake(&state, None);

        // Should have default nixpkgs for legacy
        assert!(flake.contains("nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\""));
        assert!(flake.contains("ripgrep = pkgs.ripgrep;"));

        // Should have specific commit for resolved
        assert!(flake.contains("nixpkgs-abc123de.url = \"github:NixOS/nixpkgs/abc123def456\""));
        assert!(
            flake.contains("nodejs = inputs.nixpkgs-abc123de.legacyPackages.${system}.nodejs_20;")
        );

        // Both should be in paths
        let paths_start = flake.find("paths = [").unwrap();
        let paths_end = flake[paths_start..].find("];").unwrap();
        let paths_section = &flake[paths_start..paths_start + paths_end];
        assert!(paths_section.contains("ripgrep"));
        assert!(paths_section.contains("nodejs"));
    }

    #[test]
    fn test_platform_specific_resolved_package() {
        let mut state = PackageState::default();
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "terminal-notifier".to_string(),
            version_spec: None,
            resolved_version: "2.0.0".to_string(),
            attribute_path: "terminal-notifier".to_string(),
            commit_hash: "abc123def456".to_string(),
            platforms: Some(vec![
                "aarch64-darwin".to_string(),
                "x86_64-darwin".to_string(),
            ]),
        });

        let flake = generate_flake(&state, None);

        // Should have conditional path using lib.optionals
        assert!(
            flake.contains("lib.optionals"),
            "Should use lib.optionals for platform-specific packages"
        );
        assert!(
            flake.contains("aarch64-darwin") && flake.contains("x86_64-darwin"),
            "Should include darwin platforms"
        );
        assert!(
            flake.contains("terminal-notifier"),
            "Should include the package name"
        );
    }

    #[test]
    fn test_mixed_universal_and_platform_specific() {
        let mut state = PackageState::default();
        // Universal package
        state.add_package("hello");
        // Platform-specific package
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "terminal-notifier".to_string(),
            version_spec: None,
            resolved_version: "2.0.0".to_string(),
            attribute_path: "terminal-notifier".to_string(),
            commit_hash: "abc123def456".to_string(),
            platforms: Some(vec![
                "aarch64-darwin".to_string(),
                "x86_64-darwin".to_string(),
            ]),
        });

        let flake = generate_flake(&state, None);

        // Universal package should be in the main paths list
        assert!(flake.contains("hello"));
        // Platform-specific should use lib.optionals
        assert!(flake.contains("lib.optionals"));
        assert!(flake.contains("terminal-notifier"));
    }

    #[test]
    fn test_platform_specific_custom_package() {
        let mut state = PackageState::default();
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
            platforms: Some(vec![
                "x86_64-linux".to_string(),
                "aarch64-linux".to_string(),
            ]),
        });

        let flake = generate_flake(&state, None);

        // Should have conditional path
        assert!(flake.contains("lib.optionals"));
        assert!(flake.contains("x86_64-linux") && flake.contains("aarch64-linux"));
        assert!(flake.contains("neovim"));
    }

    #[test]
    fn test_generated_flake_has_balanced_brackets() {
        /// Validates that a string has balanced brackets
        fn validate_brackets(s: &str) -> std::result::Result<(), String> {
            let mut curly = 0i32;
            let mut square = 0i32;
            let mut paren = 0i32;

            for (i, c) in s.chars().enumerate() {
                match c {
                    '{' => curly += 1,
                    '}' => {
                        curly -= 1;
                        if curly < 0 {
                            return Err(format!("Unmatched '}}' at position {}", i));
                        }
                    }
                    '[' => square += 1,
                    ']' => {
                        square -= 1;
                        if square < 0 {
                            return Err(format!("Unmatched ']' at position {}", i));
                        }
                    }
                    '(' => paren += 1,
                    ')' => {
                        paren -= 1;
                        if paren < 0 {
                            return Err(format!("Unmatched ')' at position {}", i));
                        }
                    }
                    _ => {}
                }
            }

            if curly != 0 {
                return Err(format!("Unbalanced curly braces: {} unclosed", curly));
            }
            if square != 0 {
                return Err(format!("Unbalanced square brackets: {} unclosed", square));
            }
            if paren != 0 {
                return Err(format!("Unbalanced parentheses: {} unclosed", paren));
            }

            Ok(())
        }

        // Test case 1: Empty state
        let state = PackageState::default();
        let flake = generate_flake(&state, None);
        validate_brackets(&flake).expect("Empty state should produce balanced brackets");

        // Test case 2: Standard packages only
        let mut state = PackageState::default();
        state.add_package("hello");
        state.add_package("ripgrep");
        let flake = generate_flake(&state, None);
        validate_brackets(&flake).expect("Standard packages should produce balanced brackets");

        // Test case 3: Resolved packages only (universal)
        let mut state = PackageState::default();
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "hello".to_string(),
            version_spec: Some("2.10".to_string()),
            resolved_version: "2.10".to_string(),
            attribute_path: "hello".to_string(),
            commit_hash: "abc123".to_string(),
            platforms: None,
        });
        let flake = generate_flake(&state, None);
        validate_brackets(&flake).expect("Resolved packages should produce balanced brackets");

        // Test case 4: Platform-specific resolved package
        let mut state = PackageState::default();
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "terminal-notifier".to_string(),
            version_spec: None,
            resolved_version: "2.0.0".to_string(),
            attribute_path: "terminal-notifier".to_string(),
            commit_hash: "abc123def456".to_string(),
            platforms: Some(vec![
                "aarch64-darwin".to_string(),
                "x86_64-darwin".to_string(),
            ]),
        });
        let flake = generate_flake(&state, None);
        validate_brackets(&flake)
            .expect("Platform-specific resolved package should produce balanced brackets");

        // Test case 5: Mixed universal and platform-specific
        let mut state = PackageState::default();
        state.add_package("hello");
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "terminal-notifier".to_string(),
            version_spec: None,
            resolved_version: "2.0.0".to_string(),
            attribute_path: "terminal-notifier".to_string(),
            commit_hash: "abc123def456".to_string(),
            platforms: Some(vec![
                "aarch64-darwin".to_string(),
                "x86_64-darwin".to_string(),
            ]),
        });
        let flake = generate_flake(&state, None);
        validate_brackets(&flake)
            .expect("Mixed universal and platform-specific should produce balanced brackets");

        // Test case 6: Custom package with platform restrictions
        let mut state = PackageState::default();
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
            platforms: Some(vec![
                "x86_64-linux".to_string(),
                "aarch64-linux".to_string(),
            ]),
        });
        let flake = generate_flake(&state, None);
        validate_brackets(&flake)
            .expect("Custom package with platforms should produce balanced brackets");

        // Test case 7: Universal custom package
        let mut state = PackageState::default();
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
            platforms: None,
        });
        let flake = generate_flake(&state, None);
        validate_brackets(&flake)
            .expect("Universal custom package should produce balanced brackets");

        // Test case 8: Complex mixed scenario
        let mut state = PackageState::default();
        state.add_package("hello");
        state.add_package("ripgrep");
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "jq".to_string(),
            version_spec: Some("1.6".to_string()),
            resolved_version: "1.6".to_string(),
            attribute_path: "jq".to_string(),
            commit_hash: "abc123".to_string(),
            platforms: None,
        });
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "terminal-notifier".to_string(),
            version_spec: None,
            resolved_version: "2.0.0".to_string(),
            attribute_path: "terminal-notifier".to_string(),
            commit_hash: "def456".to_string(),
            platforms: Some(vec!["aarch64-darwin".to_string()]),
        });
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
            platforms: Some(vec!["x86_64-linux".to_string()]),
        });
        let flake = generate_flake(&state, None);
        validate_brackets(&flake).expect("Complex mixed scenario should produce balanced brackets");
    }
}
