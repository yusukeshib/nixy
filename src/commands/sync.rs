use std::fs;

use crate::config::Config;
use crate::error::Result;
use crate::flake::template::regenerate_flake;
use crate::nix::Nix;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

use super::{info, success};

pub fn run(config: &Config) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    // Auto-regenerate flake.nix if missing
    if !flake_path.exists() {
        let state_path = get_state_path(&flake_dir);
        let state = PackageState::load(&state_path)?;
        info("Regenerating flake.nix from packages.json...");
        regenerate_flake(&flake_dir, &state)?;
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

    Nix::build(&flake_dir, "default", &config.env_link)?;

    success("Sync complete");
    Ok(())
}
