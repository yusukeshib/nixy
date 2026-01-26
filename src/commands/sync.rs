use std::fs;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::nix::Nix;
use crate::profile::get_flake_dir;

use super::{info, success};

pub fn run(config: &Config, allow_unfree: bool) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    info(&format!(
        "Syncing packages with {}...",
        flake_path.display()
    ));

    // Build environment and create symlink
    info("Building nixy environment...");

    // Ensure parent directory exists
    if let Some(parent) = config.env_link.parent() {
        fs::create_dir_all(parent)?;
    }

    Nix::build(&flake_dir, "default", &config.env_link, allow_unfree)?;

    success("Sync complete");
    Ok(())
}
