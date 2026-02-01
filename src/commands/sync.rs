use std::fs;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::{regenerate_flake, regenerate_flake_from_profile};
use crate::nix::Nix;
use crate::nixy_config::{nixy_json_exists, NixyConfig};
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

use super::{info, success};

pub fn run(config: &Config) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    // When using nixy.json, always regenerate flake.nix to ensure it reflects
    // the current state (nixy.json is the source of truth)
    if nixy_json_exists(config) {
        let nixy_config = NixyConfig::load(config)?;
        let active_profile_name = nixy_config.active_profile.clone();
        let profile = nixy_config
            .get_active_profile()
            .ok_or(Error::ProfileNotFound(active_profile_name))?;
        // Always pass global_packages_dir from config - even if it doesn't exist yet,
        // it will be created when local packages are installed
        let global_packages_dir = Some(config.global_packages_dir.as_path());
        regenerate_flake_from_profile(&flake_dir, profile, global_packages_dir)?;
    } else if !flake_path.exists() {
        // Legacy mode: regenerate only if flake.nix is missing
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
