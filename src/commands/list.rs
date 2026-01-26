use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::editor::extract_packages_from_flake;
use crate::profile::get_flake_path;

use std::fs;

use super::info;

pub fn run(config: &Config) -> Result<()> {
    let flake_path = get_flake_path(config);

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    info(&format!("Packages in {}:", flake_path.display()));

    // Read and parse the flake.nix file directly instead of using nix eval
    // This works even when flake.lock doesn't exist yet
    let content = fs::read_to_string(&flake_path)?;
    let packages = extract_packages_from_flake(&content);

    if packages.is_empty() {
        println!("  (none)");
    } else {
        for pkg in packages {
            println!("  {}", pkg);
        }
    }

    Ok(())
}
