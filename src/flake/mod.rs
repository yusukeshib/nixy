//! Flake.nix handling for nixy.
//!
//! This module provides functionality for parsing and generating Nix flake files.
//!
//! Submodules:
//! - `parser`: AST-based parsing of Nix files using the `rnix` library
//! - `template`: Generation of `flake.nix` content from package state

pub mod parser;
pub mod template;

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
