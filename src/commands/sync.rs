use std::fs;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::{generate_flake, PreservedContent};
use crate::nix::Nix;
use crate::profile::get_flake_dir;

use super::{info, success};

pub fn run(config: &Config) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    info(&format!(
        "Syncing packages with {}...",
        flake_path.display()
    ));

    // Check if flake has buildEnv default output (upgrade from old nixy version)
    if !Nix::has_default_output(&flake_dir) {
        info("Upgrading flake.nix to buildEnv format...");

        let packages = Nix::eval_packages(&flake_dir)?;
        let preserved = PreservedContent::from_file(&flake_path);
        let new_content = generate_flake(&packages, Some(&flake_dir), Some(&preserved));
        fs::write(&flake_path, new_content)?;
    }

    // Build environment and create symlink
    info("Building nixy environment...");

    // Ensure parent directory exists
    if let Some(parent) = config.env_link.parent() {
        fs::create_dir_all(parent)?;
    }

    Nix::build(&flake_dir, "default", &config.env_link)?;

    success("Sync complete");
    Ok(())
}
