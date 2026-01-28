//! Flake.nix handling for nixy.
//!
//! This module provides functionality for parsing and generating Nix flake files.
//!
//! Submodules:
//! - `parser`: AST-based parsing of Nix files using the `rnix` library
//! - `template`: Generation of `flake.nix` content from package state

pub mod parser;
pub mod template;

use std::path::Path;

use rnix::ast::HasEntry;

/// Check if a file is a flake (has inputs and outputs at the top level)
///
/// Uses rnix AST parsing for robustness. A valid flake must be a top-level
/// attribute set containing both `inputs` and `outputs` attributes.
pub fn is_flake_file(path: &Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let parse = rnix::Root::parse(&content);
    if !parse.errors().is_empty() {
        return false;
    }

    let root = parse.tree();
    let expr = match root.expr() {
        Some(e) => e,
        None => return false,
    };

    // Flake must be an attribute set at the top level (recursive or not)
    let attrset = match expr {
        rnix::ast::Expr::AttrSet(a) => a,
        _ => return false,
    };

    let mut has_inputs = false;
    let mut has_outputs = false;

    for entry in attrset.attrpath_values() {
        if let Some(attrpath) = entry.attrpath() {
            // Get first component of the path
            let attr_name = match attrpath.attrs().next() {
                Some(rnix::ast::Attr::Ident(ident)) => {
                    ident.ident_token().map(|t| t.text().to_string())
                }
                Some(rnix::ast::Attr::Str(s)) => {
                    // Handle quoted attribute names like { "inputs" = ...; }
                    // Extract the string value, returning None if it contains interpolation
                    let mut result = String::new();
                    let mut has_interpolation = false;
                    for part in s.parts() {
                        match part {
                            rnix::ast::InterpolPart::Literal(lit) => {
                                result.push_str(&lit.to_string());
                            }
                            rnix::ast::InterpolPart::Interpolation(_) => {
                                // Can't evaluate dynamic attribute names, skip this attribute
                                has_interpolation = true;
                            }
                        }
                    }
                    if has_interpolation {
                        None
                    } else {
                        Some(result)
                    }
                }
                _ => None,
            };

            if let Some(name) = attr_name {
                match name.as_str() {
                    "inputs" => has_inputs = true,
                    "outputs" => has_outputs = true,
                    _ => {}
                }
                // Early return once both are found
                if has_inputs && has_outputs {
                    return true;
                }
            }
        }
    }

    false
}

/// Local package information parsed from .nix files
#[derive(Debug, Clone)]
pub struct LocalPackage {
    pub name: String,
    pub input_name: Option<String>,
    pub input_url: Option<String>,
    pub overlay: Option<String>,
    pub package_expr: String,
}

/// Local flake information (subdirectory with flake.nix)
#[derive(Debug, Clone)]
pub struct LocalFlake {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_flake_file_valid() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("flake.nix");
        fs::write(
            &path,
            r#"{
            inputs.nixpkgs.url = "github:NixOS/nixpkgs";
            outputs = { nixpkgs, ... }: { };
        }"#,
        )
        .unwrap();
        assert!(is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_not_flake() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("package.nix");
        fs::write(
            &path,
            r#"{ pkgs }: pkgs.stdenv.mkDerivation {
            pname = "test";
        }"#,
        )
        .unwrap();
        assert!(!is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_comment_not_matched() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("not-flake.nix");
        fs::write(
            &path,
            r#"{ pkgs }:
        # inputs = something
        # outputs = something
        pkgs.hello"#,
        )
        .unwrap();
        assert!(!is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_missing_inputs() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("no-inputs.nix");
        fs::write(
            &path,
            r#"{
            outputs = { nixpkgs, ... }: { };
        }"#,
        )
        .unwrap();
        assert!(!is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_missing_outputs() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("no-outputs.nix");
        fs::write(
            &path,
            r#"{
            inputs.nixpkgs.url = "github:NixOS/nixpkgs";
        }"#,
        )
        .unwrap();
        assert!(!is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_nonexistent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nonexistent.nix");
        assert!(!is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_invalid_syntax() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("invalid.nix");
        // Invalid Nix: chained assignment (`{ inputs = outputs = }`) is malformed syntax.
        // is_flake_file must treat parse errors as non-flakes.
        fs::write(&path, r#"{ inputs = outputs = }"#).unwrap();
        assert!(!is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_quoted_attrs() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("quoted-flake.nix");
        fs::write(
            &path,
            r#"{
            "inputs".nixpkgs.url = "github:NixOS/nixpkgs";
            "outputs" = { nixpkgs, ... }: { };
        }"#,
        )
        .unwrap();
        assert!(is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_recursive_attrset() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("rec-flake.nix");
        fs::write(
            &path,
            r#"rec {
            inputs.nixpkgs.url = "github:NixOS/nixpkgs";
            outputs = { nixpkgs, ... }: { };
        }"#,
        )
        .unwrap();
        assert!(is_flake_file(&path));
    }

    #[test]
    fn test_is_flake_file_with_interpolated_attr_skipped() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("interpolated-flake.nix");
        // File with an interpolated attribute name should still detect inputs/outputs
        fs::write(
            &path,
            r#"{
            inputs.nixpkgs.url = "github:NixOS/nixpkgs";
            "${"dynamic"}" = "value";
            outputs = { nixpkgs, ... }: { };
        }"#,
        )
        .unwrap();
        assert!(is_flake_file(&path));
    }
}
