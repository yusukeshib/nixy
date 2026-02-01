//! Profile management for nixy.
//!
//! Profiles allow users to maintain separate package environments. Each profile
//! has its own `flake.nix` and `flake.lock` in the state directory.
//!
//! Profile structure (new format with nixy.json):
//! ```text
//! ~/.config/nixy/
//! ├── nixy.json           # Single source of truth (ALL profiles)
//! └── packages/           # Global local packages directory
//!
//! ~/.local/state/nixy/
//! ├── env -> ...          # Symlink to current profile's build
//! └── profiles/
//!     ├── default/
//!     │   ├── flake.nix   # Generated from nixy.json
//!     │   └── flake.lock  # Managed by nix
//!     └── work/
//!         └── ...
//! ```
//!
//! Legacy profile structure (for migration):
//! ```text
//! ~/.config/nixy/
//! ├── active              # Contains the name of the active profile
//! └── profiles/
//!     ├── default/        # The default profile
//!     │   ├── flake.nix
//!     │   ├── flake.lock
//!     │   ├── packages.json
//!     │   └── packages/   # Optional local packages
//!     └── work/           # Another profile
//!         └── ...
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::config::{Config, DEFAULT_PROFILE};
use crate::error::{Error, Result};
use crate::nixy_config::{nixy_json_exists, NixyConfig};

/// Regex for validating profile names (alphanumeric, dashes, underscores only)
static PROFILE_NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_-]+$").expect("Invalid regex pattern"));

/// Profile management
pub struct Profile {
    /// State directory for this profile (~/.local/state/nixy/profiles/<name>)
    pub state_dir: PathBuf,
    /// Path to flake.nix (in state directory)
    pub flake_path: PathBuf,
    /// Legacy directory path (~/.config/nixy/profiles/<name>) - for migration
    pub legacy_dir: PathBuf,
}

impl Profile {
    /// Create a Profile instance from a name and config
    pub fn new(name: &str, config: &Config) -> Self {
        let state_dir = config.profiles_state_dir.join(name);
        let legacy_dir = config.profiles_dir.join(name);
        Self {
            flake_path: state_dir.join("flake.nix"),
            state_dir,
            legacy_dir,
        }
    }

    /// Check if profile exists (either in nixy.json or legacy directory)
    pub fn exists(&self) -> bool {
        self.state_dir.exists() || self.legacy_dir.exists()
    }

    /// Check if profile exists in nixy.json
    pub fn exists_in_config(name: &str, config: &Config) -> bool {
        if !nixy_json_exists(config) {
            return false;
        }

        match NixyConfig::load(config) {
            Ok(nixy_config) => nixy_config.profile_exists(name),
            Err(err) => {
                eprintln!(
                    "Warning: failed to load nixy.json while checking for profile '{}': {}",
                    name, err
                );
                false
            }
        }
    }

    /// Create the profile state directory
    pub fn create(&self) -> Result<()> {
        fs::create_dir_all(&self.state_dir)?;
        Ok(())
    }

    /// Delete the profile state directory (and legacy directory if exists)
    pub fn delete(&self) -> Result<()> {
        if self.state_dir.exists() {
            fs::remove_dir_all(&self.state_dir)?;
        }
        if self.legacy_dir.exists() {
            fs::remove_dir_all(&self.legacy_dir)?;
        }
        Ok(())
    }
}

/// Get the active profile name
pub fn get_active_profile(config: &Config) -> String {
    // Try to read from nixy.json first (new format)
    if nixy_json_exists(config) {
        match NixyConfig::load(config) {
            Ok(nixy_config) => {
                return nixy_config.active_profile.clone();
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to load nixy.json for active profile: {}",
                    e
                );
                // Fall through to legacy active file / default profile.
            }
        }
    }

    // Fall back to legacy active file
    if config.active_file.exists() {
        fs::read_to_string(&config.active_file)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| DEFAULT_PROFILE.to_string())
    } else {
        DEFAULT_PROFILE.to_string()
    }
}

/// Set the active profile
pub fn set_active_profile(config: &Config, name: &str) -> Result<()> {
    // Update nixy.json if it exists (new format)
    if nixy_json_exists(config) {
        let mut nixy_config = NixyConfig::load(config)?;
        nixy_config.set_active_profile(name)?;
        nixy_config.save(config)?;
    } else {
        // Fall back to legacy active file
        fs::create_dir_all(&config.config_dir)?;
        fs::write(&config.active_file, name)?;
    }
    Ok(())
}

/// Validate profile name (alphanumeric, dashes, underscores only)
pub fn validate_profile_name(name: &str) -> Result<()> {
    if !PROFILE_NAME_REGEX.is_match(name) {
        return Err(Error::InvalidProfileName(name.to_string()));
    }
    Ok(())
}

/// List all profiles
pub fn list_profiles(config: &Config) -> Result<Vec<String>> {
    // Read from nixy.json if it exists (new format)
    if nixy_json_exists(config) {
        let nixy_config = NixyConfig::load(config)?;
        return Ok(nixy_config.list_profiles());
    }

    // Fall back to legacy profiles directory
    let mut profiles = Vec::new();

    if config.profiles_dir.exists() {
        for entry in fs::read_dir(&config.profiles_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    profiles.push(name.to_string());
                }
            }
        }
    }

    profiles.sort();
    Ok(profiles)
}

/// Get the flake.nix path for the active profile
pub fn get_flake_path(config: &Config) -> PathBuf {
    let active = get_active_profile(config);
    let profile = Profile::new(&active, config);

    // Check new state directory first
    if profile.flake_path.exists() {
        return profile.flake_path;
    }

    // Check legacy profile directory
    let legacy_flake = profile.legacy_dir.join("flake.nix");
    if legacy_flake.exists() {
        return legacy_flake;
    }

    // Legacy fallback: only for default profile (very old format)
    if active == DEFAULT_PROFILE && config.legacy_flake.exists() {
        return config.legacy_flake.clone();
    }

    // Return expected path in state directory even if doesn't exist
    profile.flake_path
}

/// Get the flake directory for the active profile
///
/// In the new format, this returns the state directory for the profile.
/// This function also ensures the directory exists.
pub fn get_flake_dir(config: &Config) -> Result<PathBuf> {
    let active = get_active_profile(config);
    let profile = Profile::new(&active, config);

    // If using new format (nixy.json exists), return state directory
    if nixy_json_exists(config) {
        // Ensure the state directory exists
        fs::create_dir_all(&profile.state_dir)?;
        return Ok(profile.state_dir);
    }

    // Legacy: get flake path and determine directory
    let flake_path = get_flake_path(config);

    if flake_path.is_symlink() {
        let target = fs::read_link(&flake_path)?;
        let resolved = if target.is_absolute() {
            target
        } else {
            match flake_path.parent() {
                Some(parent) => parent.join(&target),
                None => target,
            }
        };
        // Normalize the path
        let parent = match resolved.parent() {
            Some(p) => p.to_path_buf(),
            None => resolved.clone(),
        };
        if parent.exists() {
            Ok(fs::canonicalize(&parent)?)
        } else {
            Ok(parent)
        }
    } else {
        let dir = flake_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(dir)
    }
}

/// Check if there's a legacy flake that needs migration (very old format)
pub fn has_legacy_flake(config: &Config) -> bool {
    // If nixy.json exists, no legacy migration needed
    if nixy_json_exists(config) {
        return false;
    }

    // Check for very old format: flake.nix directly in config dir
    config.legacy_flake.exists() && !config.profiles_dir.join(DEFAULT_PROFILE).exists()
}

/// Migrate legacy flake to default profile (very old format)
///
/// Note: This function is kept for backwards compatibility but the main migration
/// is now handled by migration::run_migration_if_needed()
pub fn migrate_legacy_flake(config: &Config) -> Result<()> {
    let profile = Profile::new(DEFAULT_PROFILE, config);
    profile.create()?;

    // Copy flake.nix to state directory
    fs::copy(&config.legacy_flake, &profile.flake_path)?;

    // Copy flake.lock if exists
    let legacy_lock = config.config_dir.join("flake.lock");
    if legacy_lock.exists() {
        fs::copy(&legacy_lock, profile.state_dir.join("flake.lock"))?;
    }

    // Copy packages directory to global packages dir if exists
    let legacy_packages = config.config_dir.join("packages");
    if legacy_packages.exists() && !config.global_packages_dir.exists() {
        copy_dir_recursive(&legacy_packages, &config.global_packages_dir)?;
    }

    Ok(())
}

/// Recursively copy a directory
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

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_validate_profile_name_valid() {
        assert!(validate_profile_name("default").is_ok());
        assert!(validate_profile_name("work").is_ok());
        assert!(validate_profile_name("my-profile").is_ok());
        assert!(validate_profile_name("profile_123").is_ok());
        assert!(validate_profile_name("Profile-Test_123").is_ok());
    }

    #[test]
    fn test_validate_profile_name_invalid() {
        assert!(validate_profile_name("invalid name").is_err());
        assert!(validate_profile_name("invalid!name").is_err());
        assert!(validate_profile_name("invalid@name").is_err());
        assert!(validate_profile_name("invalid/name").is_err());
        assert!(validate_profile_name("").is_err());
    }

    #[test]
    fn test_get_active_profile_default() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Without active file, should return default
        let active = get_active_profile(&config);
        assert_eq!(active, DEFAULT_PROFILE);
    }

    #[test]
    fn test_get_active_profile_custom() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create active file
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.active_file, "work").unwrap();

        let active = get_active_profile(&config);
        assert_eq!(active, "work");
    }

    #[test]
    fn test_set_active_profile() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        set_active_profile(&config, "work").unwrap();

        let content = fs::read_to_string(&config.active_file).unwrap();
        assert_eq!(content, "work");
    }

    #[test]
    fn test_profile_create_and_exists() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        let profile = Profile::new("test", &config);
        assert!(!profile.exists());

        profile.create().unwrap();
        assert!(profile.exists());
        assert!(profile.state_dir.exists());
    }

    #[test]
    fn test_profile_delete() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        let profile = Profile::new("test", &config);
        profile.create().unwrap();
        assert!(profile.exists());

        profile.delete().unwrap();
        assert!(!profile.exists());
    }

    #[test]
    fn test_list_profiles_empty() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        let profiles = list_profiles(&config).unwrap();
        assert!(profiles.is_empty());
    }

    #[test]
    fn test_list_profiles_multiple() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create some profiles in legacy format (legacy profiles_dir)
        fs::create_dir_all(config.profiles_dir.join("work")).unwrap();
        fs::create_dir_all(config.profiles_dir.join("personal")).unwrap();
        fs::create_dir_all(config.profiles_dir.join("default")).unwrap();

        let profiles = list_profiles(&config).unwrap();
        assert_eq!(profiles.len(), 3);
        // Should be sorted
        assert_eq!(profiles, vec!["default", "personal", "work"]);
    }

    #[test]
    fn test_list_profiles_multiple_nixy_json() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create nixy.json with multiple profiles
        let mut nixy_config = NixyConfig::load(&config).unwrap();
        nixy_config.create_profile("work").unwrap();
        nixy_config.create_profile("personal").unwrap();
        // default is created automatically
        nixy_config.save(&config).unwrap();

        let profiles = list_profiles(&config).unwrap();
        assert_eq!(profiles.len(), 3);
        // Should be sorted
        assert_eq!(profiles, vec!["default", "personal", "work"]);
    }

    #[test]
    fn test_has_legacy_flake() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // No legacy flake
        assert!(!has_legacy_flake(&config));

        // Create legacy flake
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.legacy_flake, "{}").unwrap();
        assert!(has_legacy_flake(&config));

        // Create default profile in legacy profiles_dir - should no longer be legacy
        fs::create_dir_all(config.profiles_dir.join(DEFAULT_PROFILE)).unwrap();
        assert!(!has_legacy_flake(&config));
    }

    #[test]
    fn test_get_flake_path_profile() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create profile with flake in state directory
        let profile = Profile::new(DEFAULT_PROFILE, &config);
        profile.create().unwrap();
        fs::write(&profile.flake_path, "{}").unwrap();

        let flake_path = get_flake_path(&config);
        assert_eq!(flake_path, profile.flake_path);
    }

    #[test]
    fn test_get_flake_path_legacy_profile() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create profile with flake in legacy directory
        let profile = Profile::new(DEFAULT_PROFILE, &config);
        fs::create_dir_all(&profile.legacy_dir).unwrap();
        let legacy_flake = profile.legacy_dir.join("flake.nix");
        fs::write(&legacy_flake, "{}").unwrap();

        let flake_path = get_flake_path(&config);
        assert_eq!(flake_path, legacy_flake);
    }

    #[test]
    fn test_get_flake_path_legacy_fallback() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create only legacy flake
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.legacy_flake, "{}").unwrap();

        let flake_path = get_flake_path(&config);
        assert_eq!(flake_path, config.legacy_flake);
    }

    #[test]
    fn test_migrate_legacy_flake() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        // Create legacy flake and lock
        fs::create_dir_all(&config.config_dir).unwrap();
        fs::write(&config.legacy_flake, "{ legacy = true; }").unwrap();
        fs::write(config.config_dir.join("flake.lock"), "{}").unwrap();

        // Create legacy packages directory
        let legacy_packages = config.config_dir.join("packages");
        fs::create_dir_all(&legacy_packages).unwrap();
        fs::write(legacy_packages.join("test.nix"), "{}").unwrap();

        // Migrate
        migrate_legacy_flake(&config).unwrap();

        // Check profile was created in state directory
        let profile = Profile::new(DEFAULT_PROFILE, &config);
        assert!(profile.flake_path.exists());
        assert!(profile.state_dir.join("flake.lock").exists());

        // Check packages were copied to global directory
        assert!(config.global_packages_dir.join("test.nix").exists());

        // Check content was copied
        let content = fs::read_to_string(&profile.flake_path).unwrap();
        assert!(content.contains("legacy = true"));
    }
}
