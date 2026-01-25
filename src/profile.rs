use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::config::{Config, DEFAULT_PROFILE};
use crate::error::{Error, Result};

/// Profile management
pub struct Profile {
    pub name: String,
    pub dir: PathBuf,
    pub flake_path: PathBuf,
    pub packages_dir: PathBuf,
}

impl Profile {
    /// Create a Profile instance from a name and config
    pub fn new(name: &str, config: &Config) -> Self {
        let dir = config.profiles_dir.join(name);
        Self {
            name: name.to_string(),
            flake_path: dir.join("flake.nix"),
            packages_dir: dir.join("packages"),
            dir,
        }
    }

    /// Check if profile exists
    pub fn exists(&self) -> bool {
        self.dir.exists()
    }

    /// Create the profile directory
    pub fn create(&self) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        Ok(())
    }

    /// Delete the profile directory
    pub fn delete(&self) -> Result<()> {
        if self.dir.exists() {
            fs::remove_dir_all(&self.dir)?;
        }
        Ok(())
    }

    /// Get the flake directory (resolves symlinks)
    pub fn get_flake_dir(&self) -> Result<PathBuf> {
        if self.flake_path.is_symlink() {
            let target = fs::read_link(&self.flake_path)?;
            let resolved = if target.is_absolute() {
                target
            } else {
                self.flake_path.parent().unwrap().join(&target)
            };
            Ok(resolved.parent().unwrap().to_path_buf())
        } else {
            Ok(self.dir.clone())
        }
    }
}

/// Get the active profile name
pub fn get_active_profile(config: &Config) -> String {
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
    fs::create_dir_all(&config.config_dir)?;
    fs::write(&config.active_file, name)?;
    Ok(())
}

/// Validate profile name (alphanumeric, dashes, underscores only)
pub fn validate_profile_name(name: &str) -> Result<()> {
    let re = Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap();
    if !re.is_match(name) {
        return Err(Error::InvalidProfileName(name.to_string()));
    }
    Ok(())
}

/// List all profiles
pub fn list_profiles(config: &Config) -> Result<Vec<String>> {
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

    // Check profile directory first
    if profile.flake_path.exists() {
        return profile.flake_path;
    }

    // Legacy fallback: only for default profile
    if active == DEFAULT_PROFILE && config.legacy_flake.exists() {
        return config.legacy_flake.clone();
    }

    // Return expected path even if doesn't exist
    profile.flake_path
}

/// Get the flake directory for the active profile
pub fn get_flake_dir(config: &Config) -> Result<PathBuf> {
    let flake_path = get_flake_path(config);

    if flake_path.is_symlink() {
        let target = fs::read_link(&flake_path)?;
        let resolved = if target.is_absolute() {
            target
        } else {
            flake_path.parent().unwrap().join(&target)
        };
        // Normalize the path
        let parent = resolved.parent().unwrap();
        if parent.exists() {
            Ok(fs::canonicalize(parent)?)
        } else {
            Ok(parent.to_path_buf())
        }
    } else {
        Ok(flake_path.parent().unwrap().to_path_buf())
    }
}

/// Check if there's a legacy flake that needs migration
pub fn has_legacy_flake(config: &Config) -> bool {
    config.legacy_flake.exists() && !config.profiles_dir.join(DEFAULT_PROFILE).exists()
}

/// Migrate legacy flake to default profile
pub fn migrate_legacy_flake(config: &Config) -> Result<()> {
    let profile = Profile::new(DEFAULT_PROFILE, config);
    profile.create()?;

    // Copy flake.nix
    fs::copy(&config.legacy_flake, &profile.flake_path)?;

    // Copy flake.lock if exists
    let legacy_lock = config.config_dir.join("flake.lock");
    if legacy_lock.exists() {
        fs::copy(&legacy_lock, profile.dir.join("flake.lock"))?;
    }

    // Copy packages directory if exists
    let legacy_packages = config.config_dir.join("packages");
    if legacy_packages.exists() {
        copy_dir_recursive(&legacy_packages, &profile.packages_dir)?;
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
