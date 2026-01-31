//! Show path to package source file in Nix store.

use std::path::PathBuf;

use crate::cli::FileArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::nix::Nix;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

/// Run the file command to show the source path for a package
pub fn run(config: &Config, args: FileArgs) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);
    let state = PackageState::load(&state_path)?;

    let path = if let Some(custom) = state
        .custom_packages
        .iter()
        .find(|p| p.name == args.package)
    {
        // Custom package: prefetch the flake and return flake.nix path
        let store_path = Nix::flake_prefetch(&custom.input_url)?;
        store_path.join("flake.nix")
    } else if let Some(resolved) = state
        .resolved_packages
        .iter()
        .find(|p| p.name == args.package)
    {
        // Resolved nixpkgs package: get source path via meta.position
        let system = Nix::current_system()?;
        Nix::get_package_source_path(&resolved.commit_hash, &resolved.attribute_path, &system)?
    } else if state.packages.contains(&args.package) {
        // Legacy nixpkgs package: use nixos-unstable
        let system = Nix::current_system()?;
        Nix::get_package_source_path("nixos-unstable", &args.package, &system)?
    } else if let Some(local_path) = find_local_package(&flake_dir, &args.package) {
        // Local package in the packages/ directory
        local_path
    } else {
        return Err(Error::PackageNotInstalled(args.package));
    };

    println!("{}", path.display());
    Ok(())
}

/// Find a local package by its pname/name in the packages/ directory.
/// Returns the source file path if found.
fn find_local_package(flake_dir: &std::path::Path, package_name: &str) -> Option<PathBuf> {
    let packages_dir = flake_dir.join("packages");
    if !packages_dir.exists() {
        return None;
    }

    // Scan for .nix files and subdirectories with flake.nix
    if let Ok(entries) = std::fs::read_dir(&packages_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Check for local flake - use directory name as package name
                let flake_file = path.join("flake.nix");
                if flake_file.exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name == package_name {
                            return Some(flake_file);
                        }
                    }
                }
            } else if path.extension().is_some_and(|e| e == "nix") {
                // Parse the .nix file to get its pname/name
                if let Some(name) = parse_package_name(&path) {
                    if name == package_name {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}

/// Parse a .nix file to extract its pname or name attribute.
fn parse_package_name(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;

    // Try pname first, then name (consistent with collect_local_packages)
    crate::flake::parser::parse_local_package_attr(&content, "pname")
        .or_else(|| crate::flake::parser::parse_local_package_attr(&content, "name"))
}
