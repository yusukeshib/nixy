use std::path::Path;

use regex::Regex;

use super::{LocalFlake, LocalPackage};

/// Parse an attribute value from a nix file
/// Supports both quoted (name = "value";) and unquoted (name = value;) formats
pub fn parse_local_package_attr(content: &str, attr: &str) -> Option<String> {
    let pattern = format!(r#"^\s*{}\s*="#, regex::escape(attr));
    let re = Regex::new(&pattern).ok()?;

    for line in content.lines() {
        if re.is_match(line) {
            // Try quoted value first
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    return Some(line[start + 1..start + 1 + end].to_string());
                }
            }

            // Try unquoted value
            if let Some(eq_pos) = line.find('=') {
                let value_part = &line[eq_pos + 1..];
                if let Some(semi_pos) = value_part.find(';') {
                    let value = value_part[..semi_pos].trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }

    None
}

/// Collect local packages from a packages directory
pub fn collect_local_packages(packages_dir: &Path) -> (Vec<LocalPackage>, Vec<LocalFlake>) {
    let mut local_packages = Vec::new();
    let mut local_flakes = Vec::new();

    if !packages_dir.exists() {
        return (local_packages, local_flakes);
    }

    // Scan for flake directories (subdirectories with flake.nix)
    if let Ok(entries) = std::fs::read_dir(packages_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                let flake_file = path.join("flake.nix");
                if flake_file.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        local_flakes.push(LocalFlake {
                            name: name.to_string(),
                        });
                    }
                }
            } else if path.extension().map(|e| e == "nix").unwrap_or(false) {
                if let Some(pkg) = parse_local_package_file(&path) {
                    local_packages.push(pkg);
                }
            }
        }
    }

    (local_packages, local_flakes)
}

/// Parse a local package .nix file
fn parse_local_package_file(path: &Path) -> Option<LocalPackage> {
    let content = std::fs::read_to_string(path).ok()?;

    // Try pname first, then name
    let name = parse_local_package_attr(&content, "pname")
        .or_else(|| parse_local_package_attr(&content, "name"))?;

    // Parse inputs block - extract input name and url
    let input_name = extract_input_name(&content);
    let input_url = extract_input_url(&content);

    let overlay = parse_local_package_attr(&content, "overlay");
    let package_expr = parse_local_package_attr(&content, "packageExpr").unwrap_or_else(|| {
        // Default to callPackage if no packageExpr
        format!("pkgs.callPackage ./packages/{}.nix {{}}", name)
    });

    Some(LocalPackage {
        name,
        input_name,
        input_url,
        overlay,
        package_expr,
    })
}

/// Extract input name from content (looks for .url = pattern)
fn extract_input_name(content: &str) -> Option<String> {
    let re = Regex::new(r"([a-zA-Z0-9_-]+)\.url\s*=").ok()?;
    re.captures(content).map(|c| c[1].to_string())
}

/// Extract input URL from content
fn extract_input_url(content: &str) -> Option<String> {
    let re = Regex::new(r#"\.url\s*=\s*"([^"]+)""#).ok()?;
    re.captures(content).map(|c| c[1].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_quoted_attr() {
        let content = r#"
        pname = "my-package";
        version = "1.0.0";
        "#;
        assert_eq!(
            parse_local_package_attr(content, "pname"),
            Some("my-package".to_string())
        );
    }

    #[test]
    fn test_parse_unquoted_attr() {
        let content = r#"
        name = mypackage;
        "#;
        assert_eq!(
            parse_local_package_attr(content, "name"),
            Some("mypackage".to_string())
        );
    }

    #[test]
    fn test_extract_input_name() {
        let content = r#"
        inputs = {
            overlay-name.url = "github:user/repo";
        };
        "#;
        assert_eq!(
            extract_input_name(content),
            Some("overlay-name".to_string())
        );
    }

    #[test]
    fn test_extract_input_url() {
        let content = r#"
        inputs = {
            overlay-name.url = "github:user/repo";
        };
        "#;
        assert_eq!(
            extract_input_url(content),
            Some("github:user/repo".to_string())
        );
    }

    // Tests matching bash test_nixy.sh

    #[test]
    fn test_parse_pname_from_nixpkgs_style() {
        // test_parse_pname_from_nixpkgs_style
        let content = r#"
{ lib, buildGoModule, fetchFromGitHub }:

buildGoModule rec {
  pname = "my-package";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "test";
    repo = "test";
    rev = "v${version}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };

  vendorHash = null;
}
"#;
        assert_eq!(
            parse_local_package_attr(content, "pname"),
            Some("my-package".to_string())
        );
    }

    #[test]
    fn test_parse_name_from_simple_style() {
        // test_parse_name_from_simple_style
        let content = r#"
{ pkgs }:

pkgs.stdenv.mkDerivation {
  name = "simple-package";
  src = ./.;
}
"#;
        assert_eq!(
            parse_local_package_attr(content, "name"),
            Some("simple-package".to_string())
        );
    }

    #[test]
    fn test_pname_takes_precedence_over_name() {
        // test_parse_pname_takes_precedence
        let content = r#"
{ pkgs }:

pkgs.stdenv.mkDerivation {
  pname = "preferred-name";
  name = "fallback-name";
  version = "1.0";
  src = ./.;
}
"#;
        // pname should be found
        let pname = parse_local_package_attr(content, "pname");
        let name = parse_local_package_attr(content, "name");

        assert_eq!(pname, Some("preferred-name".to_string()));
        assert_eq!(name, Some("fallback-name".to_string()));

        // When both exist, pname should be preferred
        let preferred = pname.or(name);
        assert_eq!(preferred, Some("preferred-name".to_string()));
    }

    #[test]
    fn test_parse_fails_without_name_or_pname() {
        // test_parse_fails_without_name_or_pname
        let content = r#"
{ pkgs }:

pkgs.stdenv.mkDerivation {
  src = ./.;
  buildPhase = "echo hello";
}
"#;
        assert_eq!(parse_local_package_attr(content, "pname"), None);
        assert_eq!(parse_local_package_attr(content, "name"), None);
    }

    #[test]
    fn test_collect_local_packages_empty_dir() {
        let temp = TempDir::new().unwrap();
        let packages_dir = temp.path().join("packages");
        fs::create_dir_all(&packages_dir).unwrap();

        let (packages, flakes) = collect_local_packages(&packages_dir);
        assert!(packages.is_empty());
        assert!(flakes.is_empty());
    }

    #[test]
    fn test_collect_local_packages_nonexistent_dir() {
        let temp = TempDir::new().unwrap();
        let packages_dir = temp.path().join("nonexistent");

        let (packages, flakes) = collect_local_packages(&packages_dir);
        assert!(packages.is_empty());
        assert!(flakes.is_empty());
    }

    #[test]
    fn test_collect_local_packages_with_nix_file() {
        let temp = TempDir::new().unwrap();
        let packages_dir = temp.path().join("packages");
        fs::create_dir_all(&packages_dir).unwrap();

        // Create a .nix package file
        let pkg_content = r#"
{ lib, stdenv }:
stdenv.mkDerivation {
  pname = "my-local-pkg";
  version = "1.0.0";
  src = ./.;
}
"#;
        fs::write(packages_dir.join("my-local-pkg.nix"), pkg_content).unwrap();

        let (packages, flakes) = collect_local_packages(&packages_dir);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-local-pkg");
        assert!(flakes.is_empty());
    }

    #[test]
    fn test_collect_local_packages_with_flake_dir() {
        let temp = TempDir::new().unwrap();
        let packages_dir = temp.path().join("packages");
        let flake_dir = packages_dir.join("my-flake");
        fs::create_dir_all(&flake_dir).unwrap();

        // Create a flake.nix in subdirectory
        let flake_content = r#"
{
  inputs = { nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable"; };
  outputs = { self, nixpkgs }: { packages.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.hello; };
}
"#;
        fs::write(flake_dir.join("flake.nix"), flake_content).unwrap();

        let (packages, flakes) = collect_local_packages(&packages_dir);
        assert!(packages.is_empty());
        assert_eq!(flakes.len(), 1);
        assert_eq!(flakes[0].name, "my-flake");
    }

    #[test]
    fn test_collect_local_packages_mixed() {
        let temp = TempDir::new().unwrap();
        let packages_dir = temp.path().join("packages");
        let flake_dir = packages_dir.join("flake-pkg");
        fs::create_dir_all(&flake_dir).unwrap();

        // Create a regular .nix package
        let pkg_content = r#"
{ lib, stdenv }:
stdenv.mkDerivation {
  pname = "regular-pkg";
  version = "1.0.0";
  src = ./.;
}
"#;
        fs::write(packages_dir.join("regular-pkg.nix"), pkg_content).unwrap();

        // Create a flake package
        let flake_content = r#"
{
  inputs = { nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable"; };
  outputs = { self, nixpkgs }: { packages.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.hello; };
}
"#;
        fs::write(flake_dir.join("flake.nix"), flake_content).unwrap();

        let (packages, flakes) = collect_local_packages(&packages_dir);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "regular-pkg");
        assert_eq!(flakes.len(), 1);
        assert_eq!(flakes[0].name, "flake-pkg");
    }
}
