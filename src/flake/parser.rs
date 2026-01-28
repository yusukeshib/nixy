//! Nix file parsing using `rnix` AST.
//!
//! This module provides robust parsing of Nix files using abstract syntax tree
//! analysis rather than regex. This handles edge cases like:
//! - Multiline attribute values
//! - Comments between attributes and values
//! - Nested attribute sets
//! - String interpolation detection (returns None for dynamic values)

use std::path::Path;

use rnix::SyntaxKind;
use rowan::ast::AstNode;

use super::{LocalFlake, LocalPackage};

/// Parse an attribute value from a nix file using rnix AST parsing.
///
/// Searches recursively through nested attribute sets to find the first
/// attribute matching the given name (simple name, not full path).
/// For example, searching for "pname" will match both top-level `pname`
/// and nested `outer.inner.pname`.
///
/// Supports both quoted (name = "value";) and unquoted (name = value;) formats.
/// Handles multi-line values and complex Nix expressions.
///
/// Returns None if:
/// - The attribute is not found
/// - The value contains string interpolation (cannot be evaluated statically)
/// - The Nix content contains syntax errors
pub fn parse_local_package_attr(content: &str, attr: &str) -> Option<String> {
    let parse = rnix::Root::parse(content);

    // Return early if there are parse errors to avoid using a partial tree
    if !parse.errors().is_empty() {
        return None;
    }

    let root = parse.tree();
    find_attr_value(root.syntax(), attr)
}

/// Recursively search for an attribute binding with the given name
fn find_attr_value(node: &rnix::SyntaxNode, attr: &str) -> Option<String> {
    for child in node.children() {
        // Look for AttrpathValue nodes (attr = value;)
        if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath_value) = rnix::ast::AttrpathValue::cast(child.clone()) {
                // Check if this is the attribute we're looking for
                if let Some(attrpath) = attrpath_value.attrpath() {
                    // Collect all path components, returning None if any component
                    // cannot be extracted (e.g., dynamic attributes with interpolation)
                    let path_components: Option<Vec<String>> = attrpath
                        .attrs()
                        .map(|a| match a {
                            rnix::ast::Attr::Ident(ident) => {
                                ident.ident_token().map(|t| t.text().to_string())
                            }
                            rnix::ast::Attr::Str(s) => extract_string_value(&s),
                            _ => None,
                        })
                        .collect();

                    if let Some(components) = path_components {
                        let path_str = components.join(".");

                        if path_str == attr {
                            // Extract the value
                            if let Some(value) = attrpath_value.value() {
                                return extract_expr_value(&value);
                            }
                        }
                    }
                }
            }
        }

        // Recurse into child nodes
        if let Some(found) = find_attr_value(&child, attr) {
            return Some(found);
        }
    }
    None
}

/// Extract value from a Nix expression
fn extract_expr_value(expr: &rnix::ast::Expr) -> Option<String> {
    match expr {
        rnix::ast::Expr::Str(s) => extract_string_value(s),
        rnix::ast::Expr::Ident(ident) => ident.ident_token().map(|t| t.text().to_string()),
        rnix::ast::Expr::Literal(lit) => Some(lit.syntax().text().to_string()),
        _ => None,
    }
}

/// Extract the string value from a Str node, handling escape sequences.
///
/// Returns None if the string contains interpolation (cannot be evaluated statically).
pub fn extract_string_value(s: &rnix::ast::Str) -> Option<String> {
    let mut result = String::new();
    for part in s.parts() {
        match part {
            rnix::ast::InterpolPart::Literal(lit) => {
                result.push_str(&lit.to_string());
            }
            rnix::ast::InterpolPart::Interpolation(_) => {
                // Skip interpolations for now - we can't evaluate them statically
                return None;
            }
        }
    }
    Some(result)
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
            } else if path.extension().is_some_and(|e| e == "nix") {
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

/// Extract input name from content (looks for `name.url = "..."` pattern)
fn extract_input_name(content: &str) -> Option<String> {
    let parse = rnix::Root::parse(content);

    // Return early if there are parse errors to avoid using a partial tree
    if !parse.errors().is_empty() {
        return None;
    }

    let root = parse.tree();
    find_url_attr_parent(root.syntax())
}

/// Find the parent attr name of a `.url` attribute.
/// Only matches patterns with exactly 2 parts like `name.url = "..."`.
fn find_url_attr_parent(node: &rnix::SyntaxNode) -> Option<String> {
    for child in node.children() {
        if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath_value) = rnix::ast::AttrpathValue::cast(child.clone()) {
                if let Some(attrpath) = attrpath_value.attrpath() {
                    let attrs: Vec<_> = attrpath.attrs().collect();
                    // Check if this is a `name.url = "..."` pattern (exactly 2 parts, last is "url")
                    if attrs.len() == 2 {
                        if let Some(rnix::ast::Attr::Ident(last_ident)) = attrs.last() {
                            if let Some(token) = last_ident.ident_token() {
                                if token.text() == "url" {
                                    // Return the first part (input name)
                                    if let rnix::ast::Attr::Ident(first_ident) = &attrs[0] {
                                        if let Some(first_token) = first_ident.ident_token() {
                                            return Some(first_token.text().to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Recurse into child nodes
        if let Some(found) = find_url_attr_parent(&child) {
            return Some(found);
        }
    }
    None
}

/// Extract input URL from content
fn extract_input_url(content: &str) -> Option<String> {
    let parse = rnix::Root::parse(content);

    // Return early if there are parse errors to avoid using a partial tree
    if !parse.errors().is_empty() {
        return None;
    }

    let root = parse.tree();
    find_url_value(root.syntax())
}

/// Find the value of a `.url` attribute.
/// Only matches patterns with exactly 2 parts like `name.url = "..."` to stay
/// consistent with find_url_attr_parent.
fn find_url_value(node: &rnix::SyntaxNode) -> Option<String> {
    for child in node.children() {
        if child.kind() == SyntaxKind::NODE_ATTRPATH_VALUE {
            if let Some(attrpath_value) = rnix::ast::AttrpathValue::cast(child.clone()) {
                if let Some(attrpath) = attrpath_value.attrpath() {
                    let attrs: Vec<_> = attrpath.attrs().collect();
                    // Check if this is a `name.url = "..."` pattern (exactly 2 parts, last is "url")
                    if attrs.len() == 2 {
                        if let Some(rnix::ast::Attr::Ident(last_ident)) = attrs.last() {
                            if let Some(token) = last_ident.ident_token() {
                                if token.text() == "url" {
                                    // Extract the URL value
                                    if let Some(value) = attrpath_value.value() {
                                        return extract_expr_value(&value);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Recurse into child nodes
        if let Some(found) = find_url_value(&child) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_quoted_attr() {
        let content = r#"
{
  pname = "my-package";
  version = "1.0.0";
}
        "#;
        assert_eq!(
            parse_local_package_attr(content, "pname"),
            Some("my-package".to_string())
        );
    }

    #[test]
    fn test_parse_unquoted_attr() {
        let content = r#"
{
  name = mypackage;
}
        "#;
        assert_eq!(
            parse_local_package_attr(content, "name"),
            Some("mypackage".to_string())
        );
    }

    #[test]
    fn test_extract_input_name() {
        let content = r#"
{
  inputs = {
    overlay-name.url = "github:user/repo";
  };
}
        "#;
        assert_eq!(
            extract_input_name(content),
            Some("overlay-name".to_string())
        );
    }

    #[test]
    fn test_extract_input_url() {
        let content = r#"
{
  inputs = {
    overlay-name.url = "github:user/repo";
  };
}
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

    // New tests for edge cases that regex-based parsing couldn't handle

    #[test]
    fn test_multiline_value() {
        // This case would fail with regex-based parsing
        let content = r#"
{ pkgs }:
pkgs.stdenv.mkDerivation {
  pname =
    "multiline-package";
  version = "1.0.0";
}
"#;
        assert_eq!(
            parse_local_package_attr(content, "pname"),
            Some("multiline-package".to_string())
        );
    }

    #[test]
    fn test_value_with_comments_between() {
        // Comments between attr and value
        let content = r#"
{ pkgs }:
pkgs.stdenv.mkDerivation {
  pname = /* inline comment */ "commented-package";
  version = "1.0.0";
}
"#;
        assert_eq!(
            parse_local_package_attr(content, "pname"),
            Some("commented-package".to_string())
        );
    }

    #[test]
    fn test_multiline_input_url() {
        let content = r#"
{
  inputs = {
    my-overlay.url =
      "github:user/repo";
  };
}
"#;
        assert_eq!(extract_input_name(content), Some("my-overlay".to_string()));
        assert_eq!(
            extract_input_url(content),
            Some("github:user/repo".to_string())
        );
    }

    #[test]
    fn test_string_interpolation_returns_none() {
        // String interpolation cannot be evaluated statically
        let content = r#"
{ pkgs, version }:
pkgs.stdenv.mkDerivation {
  pname = "test-${version}";
}
"#;
        // Should return None because we can't evaluate interpolation
        assert_eq!(parse_local_package_attr(content, "pname"), None);
    }

    #[test]
    fn test_nested_attrset() {
        // Nested attribute sets
        let content = r#"
{
  outer = {
    inner = {
      pname = "nested-package";
    };
  };
}
"#;
        assert_eq!(
            parse_local_package_attr(content, "pname"),
            Some("nested-package".to_string())
        );
    }

    #[test]
    fn test_multiple_url_attrs() {
        // Multiple url attributes - should find the first one
        let content = r#"
{
  inputs = {
    first-input.url = "github:user/first";
    second-input.url = "github:user/second";
  };
}
"#;
        assert_eq!(extract_input_name(content), Some("first-input".to_string()));
        assert_eq!(
            extract_input_url(content),
            Some("github:user/first".to_string())
        );
    }
}
