use std::fs;

use crate::cli::UpgradeArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::regenerate_flake;
use crate::nix::Nix;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

use super::{info, success};

pub fn run(config: &Config, args: UpgradeArgs) -> Result<()> {
    let inputs = args.inputs;
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");
    let lock_file = flake_dir.join("flake.lock");

    // Auto-regenerate flake.nix if missing
    if !flake_path.exists() {
        let state_path = get_state_path(&flake_dir);
        let state = PackageState::load(&state_path)?;
        info("Regenerating flake.nix from packages.json...");
        regenerate_flake(&flake_dir, &state)?;
    }

    if !inputs.is_empty() {
        // Validate specific inputs
        if !lock_file.exists() {
            return Err(Error::NoFlakeLock);
        }

        let available = Nix::get_flake_inputs(&lock_file)?;
        let mut invalid = Vec::new();

        for input in &inputs {
            if !available.contains(input) {
                invalid.push(input.clone());
            }
        }

        if !invalid.is_empty() {
            return Err(Error::InvalidFlakeInputs(
                invalid.join(", "),
                available.join(" "),
            ));
        }

        info(&format!("Updating inputs: {}...", inputs.join(", ")));
        Nix::flake_update(&flake_dir, &inputs)?;
    } else {
        info("Updating all inputs...");
        Nix::flake_update_all(&flake_dir)?;
    }

    info("Rebuilding environment...");

    // Ensure parent directory exists
    if let Some(parent) = config.env_link.parent() {
        fs::create_dir_all(parent)?;
    }

    Nix::build(&flake_dir, "default", &config.env_link)?;

    if !inputs.is_empty() {
        success(&format!("Upgraded: {}", inputs.join(", ")));
    } else {
        success("All inputs upgraded");
    }

    Ok(())
}
