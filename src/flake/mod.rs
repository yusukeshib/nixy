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

    // Flake must be an attribute set at the top level
    let attrset = match expr {
        rnix::ast::Expr::AttrSet(a) => a,
        _ => return false,
    };

    let mut has_inputs = false;
    let mut has_outputs = false;

    for entry in attrset.attrpath_values() {
        if let Some(attrpath) = entry.attrpath() {
            // Get first component of the path
            if let Some(rnix::ast::Attr::Ident(ident)) = attrpath.attrs().next() {
                if let Some(token) = ident.ident_token() {
                    match token.text() {
                        "inputs" => has_inputs = true,
                        "outputs" => has_outputs = true,
                        _ => {}
                    }
                }
            }
        }
    }

    has_inputs && has_outputs
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
        fs::write(&path, r#"{ inputs = outputs = }"#).unwrap();
        assert!(!is_flake_file(&path));
    }
}
