use std::fs;
use std::io::{self, IsTerminal};

use dialoguer::{Confirm, Select};

use crate::cli::ProfileArgs;
use crate::config::{Config, DEFAULT_PROFILE};
use crate::error::{Error, Result};
use crate::flake::template::regenerate_flake_from_profile;
use crate::nix::Nix;
use crate::nixy_config::{nixy_json_exists, NixyConfig, ProfileConfig};
use crate::profile::{
    get_active_profile, get_flake_dir, has_legacy_flake, list_profiles, migrate_legacy_flake,
    set_active_profile, validate_profile_name, Profile,
};

use super::{error, info, success, warn};

pub fn run(config: &Config, args: ProfileArgs) -> Result<()> {
    match (args.name, args.c, args.d) {
        (None, false, false) => interactive_select(config),
        (Some(name), false, false) => switch(config, &name, false),
        (Some(name), true, false) => switch(config, &name, true),
        (Some(name), false, true) => delete_interactive(config, &name),
        (None, _, _) => Err(Error::Usage(
            "Profile name required with -c or -d flag".to_string(),
        )),
        (Some(_), true, true) => Err(Error::Usage(
            "Options -c (create) and -d (delete) cannot be used together".to_string(),
        )),
    }
}

fn interactive_select(config: &Config) -> Result<()> {
    let active = get_active_profile(config);
    let profiles = list_profiles(config)?;

    // Check for legacy flake
    if profiles.is_empty() && has_legacy_flake(config) {
        info("Legacy flake detected at default location.");
        info("Run 'nixy profile default' to migrate to the new profile structure.");
        return Ok(());
    }

    if profiles.is_empty() {
        info("No profiles found.");
        info("Create a profile with: nixy profile <name> -c");
        return Ok(());
    }

    // If not a TTY, just list profiles
    if !io::stdin().is_terminal() {
        info("Available profiles:");
        for name in &profiles {
            if *name == active {
                println!("  * {} (active)", name);
            } else {
                println!("    {}", name);
            }
        }
        return Ok(());
    }

    // Build selection items with active marker
    let items: Vec<String> = profiles
        .iter()
        .map(|name| {
            if *name == active {
                format!("{} (active)", name)
            } else {
                name.clone()
            }
        })
        .collect();

    // Find index of active profile
    let active_index = profiles.iter().position(|n| *n == active).unwrap_or(0);

    let selection = Select::new()
        .with_prompt("Select profile")
        .items(&items)
        .default(active_index)
        .interact_opt()?;

    match selection {
        Some(idx) => {
            let selected = &profiles[idx];
            if *selected == active {
                info(&format!("Already on profile '{}'", selected));
                Ok(())
            } else {
                switch(config, selected, false)
            }
        }
        None => {
            // User pressed Esc
            Ok(())
        }
    }
}

fn switch(config: &Config, name: &str, create: bool) -> Result<()> {
    validate_profile_name(name)?;

    let profile = Profile::new(name, config);

    // Auto-migrate legacy flake for default profile (only if not using nixy.json yet)
    if !nixy_json_exists(config)
        && name == DEFAULT_PROFILE
        && !profile.exists()
        && has_legacy_flake(config)
    {
        info("Migrating legacy flake to default profile...");
        migrate_legacy_flake(config)?;
    }

    // Check if profile exists in nixy.json or as directory
    let profile_exists = if nixy_json_exists(config) {
        Profile::exists_in_config(name, config)
    } else {
        profile.exists()
    };

    // Create profile if -c flag is set and doesn't exist
    if !profile_exists {
        if create {
            info(&format!("Creating profile '{}'...", name));

            // Create profile in nixy.json (or create nixy.json if it doesn't exist)
            let mut nixy_config = NixyConfig::load(config)?;
            nixy_config.create_profile(name)?;
            nixy_config.save(config)?;

            // Create state directory and generate flake.nix
            profile.create()?;
            let profile_config = ProfileConfig::default();
            let global_packages_dir = if config.global_packages_dir.exists() {
                Some(config.global_packages_dir.as_path())
            } else {
                None
            };
            regenerate_flake_from_profile(
                &profile.state_dir,
                &profile_config,
                global_packages_dir,
            )?;
        } else {
            return Err(Error::Usage(format!(
                "Profile '{}' does not exist. Use -c to create it: nixy profile {} -c",
                name, name
            )));
        }
    }

    info(&format!("Switching to profile '{}'...", name));
    set_active_profile(config, name)?;

    // Build environment for the new profile
    if profile.flake_path.exists() {
        info(&format!("Building environment for profile '{}'...", name));

        if let Some(parent) = config.env_link.parent() {
            fs::create_dir_all(parent)?;
        }

        // Use get_flake_dir to resolve symlinks consistently with sync/upgrade
        let flake_dir = get_flake_dir(config)?;
        match Nix::build(&flake_dir, "default", &config.env_link) {
            Ok(_) => success(&format!("Switched to profile '{}'", name)),
            Err(e) => {
                warn("Profile switched but environment build failed. Run 'nixy sync' to rebuild.");
                error(&format!("{}", e));
                success(&format!("Switched to profile '{}'", name));
            }
        }
    } else {
        success(&format!(
            "Switched to profile '{}' (no packages installed)",
            name
        ));
    }

    Ok(())
}

fn delete_interactive(config: &Config, name: &str) -> Result<()> {
    validate_profile_name(name)?;

    let profile = Profile::new(name, config);

    // Check if profile exists
    let profile_exists = if nixy_json_exists(config) {
        Profile::exists_in_config(name, config)
    } else {
        profile.exists()
    };

    if !profile_exists {
        return Err(Error::ProfileNotFound(name.to_string()));
    }

    let active = get_active_profile(config);
    if name == active {
        return Err(Error::CannotDeleteActiveProfile);
    }

    // If not a TTY, require explicit confirmation
    if !io::stdin().is_terminal() {
        return Err(Error::Usage(
            "Cannot delete profile non-interactively. Use a terminal for confirmation.".to_string(),
        ));
    }

    warn(&format!(
        "This will delete profile '{}' and all its packages.",
        name
    ));

    let confirmed = Confirm::new()
        .with_prompt("Are you sure?")
        .default(false)
        .interact()?;

    if !confirmed {
        info("Deletion cancelled.");
        return Ok(());
    }

    info(&format!("Deleting profile '{}'...", name));

    // Delete state directory first to avoid inconsistent state
    // (if this fails, the config still references the profile, which is fine)
    profile.delete()?;

    // Delete from nixy.json if using new format
    if nixy_json_exists(config) {
        let mut nixy_config = NixyConfig::load(config)?;
        nixy_config.delete_profile(name)?;
        nixy_config.save(config)?;
    }

    success(&format!("Deleted profile '{}'", name));

    Ok(())
}
