use std::fs;

use crate::cli::{ProfileArgs, ProfileCommands};
use crate::config::{Config, DEFAULT_PROFILE};
use crate::error::{Error, Result};
use crate::flake::template::generate_flake;
use crate::nix::Nix;
use crate::profile::{
    get_active_profile, has_legacy_flake, list_profiles, migrate_legacy_flake, set_active_profile,
    validate_profile_name, Profile,
};

use super::{info, success, warn};

pub fn run(config: &Config, args: ProfileArgs) -> Result<()> {
    match args.command {
        Some(ProfileCommands::Switch { name, c: create }) => switch(config, &name, create),
        Some(ProfileCommands::List) => list(config),
        Some(ProfileCommands::Delete { name, force }) => delete(config, &name, force),
        None => {
            // Show current profile
            let active = get_active_profile(config);
            info(&format!("Active profile: {}", active));
            Ok(())
        }
    }
}

fn switch(config: &Config, name: &str, create: bool) -> Result<()> {
    validate_profile_name(name)?;

    let profile = Profile::new(name, config);

    // Auto-migrate legacy flake for default profile
    if name == DEFAULT_PROFILE && !profile.exists() && has_legacy_flake(config) {
        info("Migrating legacy flake to default profile...");
        migrate_legacy_flake(config)?;
    }

    // Create profile if -c flag is set and doesn't exist
    if !profile.exists() {
        if create {
            info(&format!("Creating profile '{}'...", name));
            profile.create()?;
            let content = generate_flake(&[], Some(&profile.dir), None);
            fs::write(&profile.flake_path, content)?;
        } else {
            return Err(Error::Usage(format!(
                "Profile '{}' does not exist. Use -c to create it: nixy profile switch -c {}",
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

        match Nix::build(&profile.dir, "default", &config.env_link) {
            Ok(_) => success(&format!("Switched to profile '{}'", name)),
            Err(_) => {
                warn("Profile switched but environment build failed. Run 'nixy sync' to rebuild.");
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

fn list(config: &Config) -> Result<()> {
    let active = get_active_profile(config);
    info("Available profiles:");

    let profiles = list_profiles(config)?;

    if profiles.is_empty() {
        // Check for legacy flake
        if has_legacy_flake(config) {
            println!("  * default (active, legacy location)");
            println!();
            info("Run 'nixy profile switch default' to migrate to the new profile structure.");
        } else {
            println!("  (no profiles)");
            println!();
            info("Create a profile with: nixy profile switch -c <name>");
        }
    } else {
        for name in profiles {
            if name == active {
                println!("  * {} (active)", name);
            } else {
                println!("    {}", name);
            }
        }
    }

    Ok(())
}

fn delete(config: &Config, name: &str, force: bool) -> Result<()> {
    validate_profile_name(name)?;

    let profile = Profile::new(name, config);

    if !profile.exists() {
        return Err(Error::ProfileNotFound(name.to_string()));
    }

    let active = get_active_profile(config);
    if name == active {
        return Err(Error::CannotDeleteActiveProfile);
    }

    if !force {
        warn(&format!(
            "This will delete profile '{}' and all its packages.",
            name
        ));
        return Err(Error::Usage("Use --force to confirm deletion.".to_string()));
    }

    info(&format!("Deleting profile '{}'...", name));
    profile.delete()?;
    success(&format!("Deleted profile '{}'", name));

    Ok(())
}
