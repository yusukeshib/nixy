//! Migration from legacy per-profile packages.json to centralized nixy.json.
//!
//! This module handles the automatic migration of existing nixy configurations
//! from the old per-profile structure to the new centralized configuration.
//!
//! ## Old Structure (pre-0.3.0)
//!
//! ```text
//! ~/.config/nixy/
//! ├── active                    # Active profile name
//! └── profiles/
//!     ├── default/
//!     │   ├── flake.nix         # Generated
//!     │   ├── flake.lock        # Managed by nix
//!     │   ├── packages.json     # Package state
//!     │   └── packages/         # Local packages
//!     └── work/
//!         └── ...
//! ```
//!
//! ## New Structure (0.3.0+)
//!
//! ```text
//! ~/.config/nixy/
//! ├── nixy.json                 # Single source of truth (ALL profiles)
//! └── packages/                 # Global local packages directory
//!
//! ~/.local/state/nixy/
//! ├── env -> ...                # Symlink to current profile's build
//! └── profiles/
//!     ├── default/
//!     │   ├── flake.nix         # Generated from nixy.json
//!     │   └── flake.lock        # Managed by nix
//!     └── work/
//!         └── ...
//! ```

use std::fs;
use std::path::Path;

use crate::config::{Config, DEFAULT_PROFILE};
use crate::error::Result;
use crate::nixy_config::{NixyConfig, ProfileConfig, NIXY_CONFIG_VERSION};
use crate::state::PackageState;

/// Check if migration is needed.
///
/// Migration is needed when:
/// 1. nixy.json doesn't exist, AND
/// 2. Legacy profiles directory exists with at least one profile, OR
/// 3. Legacy active file exists
pub fn needs_migration(config: &Config) -> bool {
    // If nixy.json already exists, no migration needed
    if config.nixy_json.exists() {
        return false;
    }

    // Check for legacy profiles directory
    if config.profiles_dir.exists() && config.profiles_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&config.profiles_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    // Found at least one profile directory
                    return true;
                }
            }
        }
    }

    // Check for legacy active file
    if config.active_file.exists() {
        return true;
    }

    // Check for legacy flake.nix in config dir (very old format)
    if config.legacy_flake.exists() {
        return true;
    }

    false
}

/// Migrate from legacy per-profile packages.json to centralized nixy.json.
///
/// This function:
/// 1. Reads all existing profile directories from ~/.config/nixy/profiles/
/// 2. Loads packages.json from each profile
/// 3. Creates a unified nixy.json with all profile data
/// 4. Copies flake.nix and flake.lock to the new state directory
/// 5. Merges local packages from all profiles to the global packages directory
/// 6. Preserves the active profile setting
pub fn migrate_to_nixy_json(config: &Config) -> Result<NixyConfig> {
    let mut nixy_config = NixyConfig {
        version: NIXY_CONFIG_VERSION,
        active_profile: DEFAULT_PROFILE.to_string(),
        profiles: std::collections::HashMap::new(),
    };

    // Read active profile from legacy file
    if config.active_file.exists() {
        if let Ok(active) = fs::read_to_string(&config.active_file) {
            let active = active.trim().to_string();
            if !active.is_empty() {
                nixy_config.active_profile = active;
            }
        }
    }

    // Migrate each profile
    if config.profiles_dir.exists() {
        if let Ok(entries) = fs::read_dir(&config.profiles_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        let profile_config = migrate_profile(&path)?;
                        nixy_config
                            .profiles
                            .insert(name.to_string(), profile_config);

                        // Copy flake.nix and flake.lock to state directory
                        let state_profile_dir = config.profiles_state_dir.join(name);
                        fs::create_dir_all(&state_profile_dir)?;

                        let legacy_flake = path.join("flake.nix");
                        if legacy_flake.exists() {
                            fs::copy(&legacy_flake, state_profile_dir.join("flake.nix"))?;
                        }

                        let legacy_lock = path.join("flake.lock");
                        if legacy_lock.exists() {
                            fs::copy(&legacy_lock, state_profile_dir.join("flake.lock"))?;
                        }

                        // Merge local packages to global directory
                        let legacy_packages_dir = path.join("packages");
                        if legacy_packages_dir.exists() {
                            merge_local_packages(
                                &legacy_packages_dir,
                                &config.global_packages_dir,
                            )?;
                        }
                    }
                }
            }
        }
    }

    // Handle very old format (flake.nix directly in config dir)
    if config.legacy_flake.exists() && !nixy_config.profiles.contains_key(DEFAULT_PROFILE) {
        nixy_config
            .profiles
            .insert(DEFAULT_PROFILE.to_string(), ProfileConfig::default());

        // Copy legacy flake to state directory
        let state_profile_dir = config.profiles_state_dir.join(DEFAULT_PROFILE);
        fs::create_dir_all(&state_profile_dir)?;
        fs::copy(&config.legacy_flake, state_profile_dir.join("flake.nix"))?;

        let legacy_lock = config.config_dir.join("flake.lock");
        if legacy_lock.exists() {
            fs::copy(&legacy_lock, state_profile_dir.join("flake.lock"))?;
        }

        // Merge local packages from config dir
        let legacy_packages_dir = config.config_dir.join("packages");
        if legacy_packages_dir.exists() {
            merge_local_packages(&legacy_packages_dir, &config.global_packages_dir)?;
        }
    }

    // Ensure default profile exists
    if !nixy_config.profiles.contains_key(DEFAULT_PROFILE) {
        nixy_config
            .profiles
            .insert(DEFAULT_PROFILE.to_string(), ProfileConfig::default());
    }

    // Ensure active profile exists in profiles
    if !nixy_config
        .profiles
        .contains_key(&nixy_config.active_profile)
    {
        nixy_config.active_profile = DEFAULT_PROFILE.to_string();
    }

    Ok(nixy_config)
}

/// Migrate a single profile from its directory.
fn migrate_profile(profile_dir: &Path) -> Result<ProfileConfig> {
    let state_path = profile_dir.join("packages.json");

    if state_path.exists() {
        let state = PackageState::load(&state_path)?;
        Ok(ProfileConfig::from(&state))
    } else {
        Ok(ProfileConfig::default())
    }
}

/// Merge local packages from a legacy profile's packages directory to the global directory.
fn merge_local_packages(src_dir: &Path, dst_dir: &Path) -> Result<()> {
    if !src_dir.exists() {
        return Ok(());
    }

    fs::create_dir_all(dst_dir)?;

    if let Ok(entries) = fs::read_dir(src_dir) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let file_name = entry.file_name();
            let dst_path = dst_dir.join(&file_name);

            // Skip if destination already exists (don't overwrite)
            if dst_path.exists() {
                continue;
            }

            if src_path.is_dir() {
                copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
    }

    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Run the migration process.
///
/// This is called from main.rs before any command is executed.
/// It checks if migration is needed and performs it automatically.
pub fn run_migration_if_needed(config: &Config) -> Result<()> {
    if !needs_migration(config) {
        return Ok(());
    }

    crate::commands::info("Migrating to new nixy.json configuration format...");

    let nixy_config = migrate_to_nixy_json(config)?;
    nixy_config.save(config)?;

    crate::commands::success("Migration complete! Your configuration has been updated.");
    crate::commands::info(&format!(
        "Configuration is now stored in: {}",
        config.nixy_json.display()
    ));
    crate::commands::info(&format!(
        "Generated files are now in: {}",
        config.profiles_state_dir.display()
    ));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CustomPackage, ResolvedNixpkgPackage};
    use tempfile::TempDir;

    fn test_config(temp: &TempDir) -> Config {
        Config {
            config_dir: temp.path().join("config"),
            nixy_json: temp.path().join("config/nixy.json"),
            global_packages_dir: temp.path().join("config/packages"),
            state_dir: temp.path().join("state"),
            profiles_state_dir: temp.path().join("state/profiles"),
            profiles_dir: temp.path().join("config/profiles"),
            active_file: temp.path().join("config/active"),
            env_link: temp.path().join("state/env"),
            legacy_flake: temp.path().join("config/flake.nix"),
        }
    }

    #[test]
    fn test_needs_migration_false_when_nixy_json_exists() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create nixy.json
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.nixy_json, "{}").unwrap();

        assert!(!needs_migration(&config));
    }

    #[test]
    fn test_needs_migration_false_when_nothing_exists() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        assert!(!needs_migration(&config));
    }

    #[test]
    fn test_needs_migration_true_when_legacy_profile_exists() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create legacy profile directory
        fs::create_dir_all(config.profiles_dir.join("default")).unwrap();

        assert!(needs_migration(&config));
    }

    #[test]
    fn test_needs_migration_true_when_active_file_exists() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create legacy active file
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.active_file, "default").unwrap();

        assert!(needs_migration(&config));
    }

    #[test]
    fn test_needs_migration_true_when_legacy_flake_exists() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create legacy flake.nix
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.legacy_flake, "{}").unwrap();

        assert!(needs_migration(&config));
    }

    #[test]
    fn test_migrate_empty_profile() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create empty legacy profile
        let profile_dir = config.profiles_dir.join("default");
        fs::create_dir_all(&profile_dir).unwrap();

        let nixy_config = migrate_to_nixy_json(&config).unwrap();

        assert!(nixy_config.profiles.contains_key("default"));
        assert_eq!(nixy_config.active_profile, "default");
    }

    #[test]
    fn test_migrate_profile_with_packages() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create legacy profile with packages.json
        let profile_dir = config.profiles_dir.join("default");
        fs::create_dir_all(&profile_dir).unwrap();

        let state = PackageState {
            version: 2,
            packages: vec!["hello".to_string()],
            resolved_packages: vec![ResolvedNixpkgPackage {
                name: "nodejs".to_string(),
                version_spec: Some("20".to_string()),
                resolved_version: "20.11.0".to_string(),
                attribute_path: "nodejs_20".to_string(),
                commit_hash: "abc123".to_string(),
                platforms: None,
            }],
            custom_packages: vec![CustomPackage {
                name: "neovim".to_string(),
                input_name: "neovim-nightly".to_string(),
                input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
                package_output: "packages".to_string(),
                source_name: None,
                platforms: None,
            }],
        };
        state.save(&profile_dir.join("packages.json")).unwrap();

        let nixy_config = migrate_to_nixy_json(&config).unwrap();
        let profile = nixy_config.profiles.get("default").unwrap();

        assert!(profile.has_package("hello"));
        assert!(profile.has_package("nodejs"));
        assert!(profile.has_package("neovim"));
    }

    #[test]
    fn test_migrate_preserves_active_profile() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create two profiles
        fs::create_dir_all(config.profiles_dir.join("default")).unwrap();
        fs::create_dir_all(config.profiles_dir.join("work")).unwrap();

        // Set work as active
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.active_file, "work").unwrap();

        let nixy_config = migrate_to_nixy_json(&config).unwrap();

        assert_eq!(nixy_config.active_profile, "work");
    }

    #[test]
    fn test_migrate_copies_flake_files() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create legacy profile with flake files
        let profile_dir = config.profiles_dir.join("default");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(profile_dir.join("flake.nix"), "{ }").unwrap();
        fs::write(profile_dir.join("flake.lock"), "{}").unwrap();

        let _ = migrate_to_nixy_json(&config).unwrap();

        // Check files were copied to state directory
        let state_profile_dir = config.profiles_state_dir.join("default");
        assert!(state_profile_dir.join("flake.nix").exists());
        assert!(state_profile_dir.join("flake.lock").exists());
    }

    #[test]
    fn test_migrate_merges_local_packages() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create legacy profile with local packages
        let profile_dir = config.profiles_dir.join("default");
        let packages_dir = profile_dir.join("packages");
        fs::create_dir_all(&packages_dir).unwrap();
        fs::write(packages_dir.join("my-pkg.nix"), "{ }").unwrap();

        let _ = migrate_to_nixy_json(&config).unwrap();

        // Check packages were copied to global directory
        assert!(config.global_packages_dir.join("my-pkg.nix").exists());
    }

    #[test]
    fn test_migrate_handles_multiple_profiles() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create multiple profiles
        for name in &["default", "work", "personal"] {
            let profile_dir = config.profiles_dir.join(name);
            fs::create_dir_all(&profile_dir).unwrap();

            let mut state = PackageState::default();
            state.packages.push(format!("{}-pkg", name));
            state.save(&profile_dir.join("packages.json")).unwrap();
        }

        let nixy_config = migrate_to_nixy_json(&config).unwrap();

        assert!(nixy_config.profiles.contains_key("default"));
        assert!(nixy_config.profiles.contains_key("work"));
        assert!(nixy_config.profiles.contains_key("personal"));

        assert!(nixy_config
            .profiles
            .get("default")
            .unwrap()
            .has_package("default-pkg"));
        assert!(nixy_config
            .profiles
            .get("work")
            .unwrap()
            .has_package("work-pkg"));
        assert!(nixy_config
            .profiles
            .get("personal")
            .unwrap()
            .has_package("personal-pkg"));
    }

    #[test]
    fn test_migrate_legacy_flake_only() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create very old format (flake.nix in config dir)
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.legacy_flake, "{ }").unwrap();

        let nixy_config = migrate_to_nixy_json(&config).unwrap();

        // Should create default profile
        assert!(nixy_config.profiles.contains_key("default"));

        // Should copy flake to state directory
        let state_profile_dir = config.profiles_state_dir.join("default");
        assert!(state_profile_dir.join("flake.nix").exists());
    }
}
