pub mod parser;
pub mod template;

use std::path::Path;

/// Check if a file is a flake (has inputs and outputs)
pub fn is_flake_file(path: &Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let has_inputs = content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("inputs") && trimmed.contains('=')
    });

    let has_outputs = content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("outputs") && trimmed.contains('=')
    });

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
