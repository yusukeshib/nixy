use std::fs;
use std::path::Path;

use regex::Regex;

use crate::error::Result;
use crate::state::{get_state_path, CustomPackage, PackageState};

/// Check if migration is needed (flake.nix exists with markers, but no packages.json)
pub fn needs_migration(profile_dir: &Path) -> bool {
    let flake_path = profile_dir.join("flake.nix");
    let state_path = get_state_path(profile_dir);

    // If state file already exists, no migration needed
    if state_path.exists() {
        return false;
    }

    // If no flake.nix, no migration needed
    if !flake_path.exists() {
        return false;
    }

    // Check if flake has nixy markers
    let content = match fs::read_to_string(&flake_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    content.contains("[nixy:packages]")
}

/// Migrate from marker-based flake.nix to packages.json
pub fn migrate(profile_dir: &Path) -> Result<()> {
    let flake_path = profile_dir.join("flake.nix");
    let state_path = get_state_path(profile_dir);

    // Read the flake content
    let content = fs::read_to_string(&flake_path)?;

    // Extract packages from markers
    let mut state = PackageState::default();

    // Extract standard packages from nixy:packages section
    let standard_packages = extract_standard_packages(&content);
    for pkg in standard_packages {
        state.add_package(&pkg);
    }

    // Extract custom packages from nixy:custom-packages section
    let custom_packages = extract_custom_packages(&content);
    for pkg in custom_packages {
        state.add_custom_package(pkg);
    }

    // Extract local packages (from nixy:local-packages)
    // These are packages defined in local .nix files - they don't go in packages.json
    // as they are auto-discovered from the packages/ directory

    // Save the state file
    state.save(&state_path)?;

    // Regenerate flake.nix in the new marker-free format
    let content = crate::flake::template::generate_flake(&state, Some(profile_dir));
    fs::write(&flake_path, content)?;

    Ok(())
}

/// Extract standard package names from the nixy:packages section
fn extract_standard_packages(content: &str) -> Vec<String> {
    let mut packages = Vec::new();
    let section = extract_marker_content(content, "nixy:packages");

    for line in section.lines() {
        let trimmed = line.trim();
        // Skip comments
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        // Match pattern: "pkgname = pkgs.pkgname;"
        if let Some(eq_pos) = trimmed.find('=') {
            let name = trimmed[..eq_pos].trim();
            let rhs = trimmed[eq_pos + 1..].trim();

            // Only include if RHS matches "pkgs.{name};"
            let expected_rhs = format!("pkgs.{};", name);
            if rhs == expected_rhs && is_valid_nix_identifier(name) {
                packages.push(name.to_string());
            }
        }
    }

    packages
}

/// Extract custom packages from the nixy:custom-packages section
fn extract_custom_packages(content: &str) -> Vec<CustomPackage> {
    let mut packages = Vec::new();
    let section = extract_marker_content(content, "nixy:custom-packages");

    // Pattern for custom packages: pkg = inputs.INPUT_NAME.packages.${system}.pkg;
    // or: pkg = inputs.INPUT_NAME.legacyPackages.${system}.pkg;
    let re = Regex::new(
        r"^\s*([a-zA-Z0-9_-]+)\s*=\s*inputs\.([a-zA-Z0-9_-]+)\.(packages|legacyPackages)\.\$\{system\}\.([a-zA-Z0-9_-]+);",
    );

    if let Ok(re) = re {
        for line in section.lines() {
            if let Some(caps) = re.captures(line) {
                let name = caps[1].to_string();
                let input_name = caps[2].to_string();
                let package_output = caps[3].to_string();
                let source_pkg_name = caps[4].to_string();

                // Check if package is an alias (name differs from source package name)
                let source_name = if name != source_pkg_name {
                    eprintln!(
                        "Warning: Migration: custom package '{}' is an alias for '{}'; preserving alias during migration.",
                        name, source_pkg_name
                    );
                    Some(source_pkg_name)
                } else {
                    None
                };

                // Try to find the input URL from custom-inputs section
                let input_url = match find_input_url(content, &input_name) {
                    Some(url) => url,
                    None => {
                        eprintln!(
                            "Warning: Migration: could not find URL for input '{}' used by custom package '{}'; skipping this package.",
                            input_name, name
                        );
                        continue;
                    }
                };

                packages.push(CustomPackage {
                    name,
                    input_name,
                    input_url,
                    package_output,
                    source_name,
                });
            }
        }
    }

    packages
}

/// Find the URL for a given input name
fn find_input_url(content: &str, input_name: &str) -> Option<String> {
    // Look in both custom-inputs and local-inputs sections
    let custom_inputs = extract_marker_content(content, "nixy:custom-inputs");
    let local_inputs = extract_marker_content(content, "nixy:local-inputs");

    let pattern = format!(r#"{}\.url\s*=\s*"([^"]+)""#, regex::escape(input_name));
    let re = Regex::new(&pattern).ok()?;

    // Check custom-inputs first
    for line in custom_inputs.lines() {
        if let Some(caps) = re.captures(line) {
            return Some(caps[1].to_string());
        }
    }

    // Then check local-inputs
    for line in local_inputs.lines() {
        if let Some(caps) = re.captures(line) {
            return Some(caps[1].to_string());
        }
    }

    None
}

/// Extract content between markers (excluding the markers themselves)
fn extract_marker_content(content: &str, marker: &str) -> String {
    let start_marker = format!("# [{}]", marker);
    let end_marker = format!("# [/{}]", marker);

    let mut result = String::new();
    let mut in_section = false;

    for line in content.lines() {
        if line.contains(&end_marker) {
            in_section = false;
            continue;
        }

        if in_section {
            result.push_str(line);
            result.push('\n');
        }

        if line.contains(&start_marker) {
            in_section = true;
        }
    }

    result
}

/// Check if a string is a valid Nix identifier
fn is_valid_nix_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_needs_migration_no_flake() {
        let temp = TempDir::new().unwrap();
        assert!(!needs_migration(temp.path()));
    }

    #[test]
    fn test_needs_migration_state_exists() {
        let temp = TempDir::new().unwrap();

        // Create both files
        let flake_content = r#"{ # [nixy:packages] # [/nixy:packages] }"#;
        fs::write(temp.path().join("flake.nix"), flake_content).unwrap();
        fs::write(temp.path().join("packages.json"), "{}").unwrap();

        assert!(!needs_migration(temp.path()));
    }

    #[test]
    fn test_needs_migration_marker_flake() {
        let temp = TempDir::new().unwrap();

        let flake_content = r#"{
          # [nixy:packages]
          hello = pkgs.hello;
          # [/nixy:packages]
        }"#;
        fs::write(temp.path().join("flake.nix"), flake_content).unwrap();

        assert!(needs_migration(temp.path()));
    }

    #[test]
    fn test_needs_migration_no_markers() {
        let temp = TempDir::new().unwrap();

        // A flake without markers doesn't need migration
        let flake_content = r#"{ outputs = {}; }"#;
        fs::write(temp.path().join("flake.nix"), flake_content).unwrap();

        assert!(!needs_migration(temp.path()));
    }

    #[test]
    fn test_extract_standard_packages() {
        let content = r#"
          # [nixy:packages]
          ripgrep = pkgs.ripgrep;
          fzf = pkgs.fzf;
          bat = pkgs.bat;
          # [/nixy:packages]
        "#;

        let packages = extract_standard_packages(content);
        assert_eq!(packages.len(), 3);
        assert!(packages.contains(&"ripgrep".to_string()));
        assert!(packages.contains(&"fzf".to_string()));
        assert!(packages.contains(&"bat".to_string()));
    }

    #[test]
    fn test_extract_standard_packages_ignores_custom() {
        let content = r#"
          # [nixy:packages]
          hello = pkgs.hello;
          custom = pkgs.other;
          # [/nixy:packages]
        "#;

        let packages = extract_standard_packages(content);
        // "custom = pkgs.other" should be ignored since RHS doesn't match
        assert_eq!(packages.len(), 1);
        assert!(packages.contains(&"hello".to_string()));
    }

    #[test]
    fn test_extract_custom_packages() {
        let content = r#"
          # [nixy:custom-inputs]
          neovim-nightly.url = "github:nix-community/neovim-nightly-overlay";
          # [/nixy:custom-inputs]
          # [nixy:custom-packages]
          neovim = inputs.neovim-nightly.packages.${system}.neovim;
          # [/nixy:custom-packages]
        "#;

        let packages = extract_custom_packages(content);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "neovim");
        assert_eq!(packages[0].input_name, "neovim-nightly");
        assert_eq!(
            packages[0].input_url,
            "github:nix-community/neovim-nightly-overlay"
        );
        assert_eq!(packages[0].package_output, "packages");
    }

    #[test]
    fn test_extract_custom_packages_legacy_packages() {
        let content = r#"
          # [nixy:custom-inputs]
          nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
          # [/nixy:custom-inputs]
          # [nixy:custom-packages]
          hello = inputs.nixpkgs-unstable.legacyPackages.${system}.hello;
          # [/nixy:custom-packages]
        "#;

        let packages = extract_custom_packages(content);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "hello");
        assert_eq!(packages[0].package_output, "legacyPackages");
    }

    #[test]
    fn test_migrate() {
        let temp = TempDir::new().unwrap();

        let flake_content = r#"{
          # [nixy:custom-inputs]
          neovim-nightly.url = "github:nix-community/neovim-nightly-overlay";
          # [/nixy:custom-inputs]
          # [nixy:packages]
          ripgrep = pkgs.ripgrep;
          fzf = pkgs.fzf;
          # [/nixy:packages]
          # [nixy:custom-packages]
          neovim = inputs.neovim-nightly.packages.${system}.neovim;
          # [/nixy:custom-packages]
        }"#;
        fs::write(temp.path().join("flake.nix"), flake_content).unwrap();

        migrate(temp.path()).unwrap();

        // Load and verify the state file
        let state_path = get_state_path(temp.path());
        let state = PackageState::load(&state_path).unwrap();

        assert_eq!(state.packages.len(), 2);
        assert!(state.packages.contains(&"ripgrep".to_string()));
        assert!(state.packages.contains(&"fzf".to_string()));

        assert_eq!(state.custom_packages.len(), 1);
        assert_eq!(state.custom_packages[0].name, "neovim");
    }
}
