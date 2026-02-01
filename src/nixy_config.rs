//! Centralized configuration for nixy.
//!
//! This module provides the unified `nixy.json` configuration file that serves as the
//! single source of truth for all profile data. It replaces the per-profile `packages.json`
//! files with a centralized approach.
//!
//! ## Configuration Structure
//!
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

use std::collections::BTreeMap;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::config::{Config, DEFAULT_PROFILE};
use crate::error::{Error, Result};
use crate::state::{CustomPackage, ResolvedNixpkgPackage};

/// Current version of the nixy.json format
pub const NIXY_CONFIG_VERSION: u32 = 3;

/// Configuration for a single profile
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    /// Legacy: simple package names (kept for backwards compatibility)
    #[serde(default)]
    pub packages: Vec<String>,
    /// Packages resolved via Nixhub with specific versions and commits
    #[serde(default)]
    pub resolved_packages: Vec<ResolvedNixpkgPackage>,
    /// Packages from custom flake URLs
    #[serde(default)]
    pub custom_packages: Vec<CustomPackage>,
}

impl ProfileConfig {
    /// Add a standard nixpkgs package (legacy method for backwards compatibility)
    #[allow(dead_code)]
    pub fn add_package(&mut self, name: &str) {
        if !self.packages.contains(&name.to_string()) {
            self.packages.push(name.to_string());
            self.packages.sort();
        }
    }

    /// Add a resolved nixpkgs package (new method with version info)
    pub fn add_resolved_package(&mut self, pkg: ResolvedNixpkgPackage) {
        // Remove from legacy packages if present
        self.packages.retain(|p| p != &pkg.name);
        // Remove any existing resolved package with the same name
        self.resolved_packages.retain(|p| p.name != pkg.name);
        self.resolved_packages.push(pkg);
        self.resolved_packages.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Get a resolved package by name
    #[allow(dead_code)]
    pub fn get_resolved_package(&self, name: &str) -> Option<&ResolvedNixpkgPackage> {
        self.resolved_packages.iter().find(|p| p.name == name)
    }

    /// Add a custom package from a registry
    pub fn add_custom_package(&mut self, pkg: CustomPackage) {
        // Remove any existing package with the same name
        self.custom_packages.retain(|p| p.name != pkg.name);
        self.custom_packages.push(pkg);
        self.custom_packages.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Remove a package by name (checks legacy, resolved, and custom)
    pub fn remove_package(&mut self, name: &str) -> bool {
        let removed_legacy = self.packages.iter().position(|p| p == name).map(|i| {
            self.packages.remove(i);
            true
        });

        let removed_resolved = self
            .resolved_packages
            .iter()
            .position(|p| p.name == name)
            .map(|i| {
                self.resolved_packages.remove(i);
                true
            });

        let removed_custom = self
            .custom_packages
            .iter()
            .position(|p| p.name == name)
            .map(|i| {
                self.custom_packages.remove(i);
                true
            });

        removed_legacy.unwrap_or(false)
            || removed_resolved.unwrap_or(false)
            || removed_custom.unwrap_or(false)
    }

    /// Check if a package is installed (legacy, resolved, or custom)
    pub fn has_package(&self, name: &str) -> bool {
        self.packages.contains(&name.to_string())
            || self.resolved_packages.iter().any(|p| p.name == name)
            || self.custom_packages.iter().any(|p| p.name == name)
    }

    /// Check if a package is a legacy (non-resolved) package
    #[allow(dead_code)]
    pub fn is_legacy_package(&self, name: &str) -> bool {
        self.packages.contains(&name.to_string())
    }
}

/// The main nixy.json configuration file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NixyConfig {
    /// Configuration file version
    pub version: u32,
    /// Name of the active profile
    pub active_profile: String,
    /// All profile configurations
    pub profiles: BTreeMap<String, ProfileConfig>,
}

impl Default for NixyConfig {
    fn default() -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert(DEFAULT_PROFILE.to_string(), ProfileConfig::default());
        Self {
            version: NIXY_CONFIG_VERSION,
            active_profile: DEFAULT_PROFILE.to_string(),
            profiles,
        }
    }
}

impl NixyConfig {
    /// Load nixy.json from the config directory
    pub fn load(config: &Config) -> Result<Self> {
        let path = &config.nixy_json;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        let mut nixy_config: Self =
            serde_json::from_str(&content).map_err(|e| Error::StateFile(e.to_string()))?;

        // Migrate from older versions if needed
        if nixy_config.version < NIXY_CONFIG_VERSION {
            nixy_config.version = NIXY_CONFIG_VERSION;
        }

        // Normalize config: ensure default profile exists and active_profile is valid
        nixy_config.normalize();

        Ok(nixy_config)
    }

    /// Normalize the config to ensure invariants are maintained:
    /// - Default profile always exists
    /// - active_profile always points to an existing profile
    fn normalize(&mut self) {
        // Ensure default profile exists
        if !self.profiles.contains_key(DEFAULT_PROFILE) {
            self.profiles
                .insert(DEFAULT_PROFILE.to_string(), ProfileConfig::default());
        }

        // Ensure active_profile points to an existing profile
        if !self.profiles.contains_key(&self.active_profile) {
            self.active_profile = DEFAULT_PROFILE.to_string();
        }
    }

    /// Save nixy.json to the config directory atomically
    pub fn save(&self, config: &Config) -> Result<()> {
        let path = &config.nixy_json;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content =
            serde_json::to_string_pretty(self).map_err(|e| Error::StateFile(e.to_string()))?;

        // Write to a temporary file first, then atomically rename it into place
        let tmp_path = path.with_extension("json.tmp");
        if let Err(e) = fs::write(&tmp_path, &content) {
            // Clean up temp file on write failure (if it was partially created)
            let _ = fs::remove_file(&tmp_path);
            return Err(e.into());
        }
        if let Err(e) = fs::rename(&tmp_path, path) {
            // Clean up temp file on rename failure
            let _ = fs::remove_file(&tmp_path);
            return Err(e.into());
        }
        Ok(())
    }

    /// Get the active profile configuration
    pub fn get_active_profile(&self) -> Option<&ProfileConfig> {
        self.profiles.get(&self.active_profile)
    }

    /// Get the active profile configuration (mutable)
    pub fn get_active_profile_mut(&mut self) -> Option<&mut ProfileConfig> {
        let name = self.active_profile.clone();
        self.profiles.get_mut(&name)
    }

    /// Set the active profile
    pub fn set_active_profile(&mut self, name: &str) -> Result<()> {
        if !self.profiles.contains_key(name) {
            return Err(Error::ProfileNotFound(name.to_string()));
        }
        self.active_profile = name.to_string();
        Ok(())
    }

    /// Create a new profile
    pub fn create_profile(&mut self, name: &str) -> Result<()> {
        if self.profiles.contains_key(name) {
            // Profile already exists, not an error
            return Ok(());
        }
        self.profiles
            .insert(name.to_string(), ProfileConfig::default());
        Ok(())
    }

    /// Delete a profile
    pub fn delete_profile(&mut self, name: &str) -> Result<()> {
        if name == self.active_profile {
            return Err(Error::CannotDeleteActiveProfile);
        }
        if self.profiles.remove(name).is_none() {
            return Err(Error::ProfileNotFound(name.to_string()));
        }
        Ok(())
    }

    /// List all profile names
    pub fn list_profiles(&self) -> Vec<String> {
        let mut names: Vec<String> = self.profiles.keys().cloned().collect();
        names.sort();
        names
    }

    /// Check if a profile exists
    pub fn profile_exists(&self, name: &str) -> bool {
        self.profiles.contains_key(name)
    }
}

/// Check if nixy.json exists
pub fn nixy_json_exists(config: &Config) -> bool {
    config.nixy_json.exists()
}

/// Convert ProfileConfig to PackageState for compatibility with existing code
impl From<&ProfileConfig> for crate::state::PackageState {
    fn from(profile: &ProfileConfig) -> Self {
        Self {
            version: 2,
            packages: profile.packages.clone(),
            resolved_packages: profile.resolved_packages.clone(),
            custom_packages: profile.custom_packages.clone(),
        }
    }
}

/// Convert PackageState to ProfileConfig for migration
impl From<&crate::state::PackageState> for ProfileConfig {
    fn from(state: &crate::state::PackageState) -> Self {
        Self {
            packages: state.packages.clone(),
            resolved_packages: state.resolved_packages.clone(),
            custom_packages: state.custom_packages.clone(),
        }
    }
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
    fn test_default_config() {
        let config = NixyConfig::default();
        assert_eq!(config.version, NIXY_CONFIG_VERSION);
        assert_eq!(config.active_profile, "default");
        assert!(config.profiles.contains_key("default"));
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);
        let nixy_config = NixyConfig::load(&config).unwrap();
        assert_eq!(nixy_config.active_profile, "default");
    }

    #[test]
    fn test_save_and_load() {
        let temp = TempDir::new().unwrap();
        let config = test_config(&temp);

        let mut nixy_config = NixyConfig::default();
        nixy_config.create_profile("work").unwrap();
        nixy_config.set_active_profile("work").unwrap();

        if let Some(profile) = nixy_config.profiles.get_mut("work") {
            profile.add_resolved_package(ResolvedNixpkgPackage {
                name: "hello".to_string(),
                version_spec: None,
                resolved_version: "2.12.1".to_string(),
                attribute_path: "hello".to_string(),
                commit_hash: "abc123".to_string(),
                platforms: None,
            });
        }

        nixy_config.save(&config).unwrap();

        let loaded = NixyConfig::load(&config).unwrap();
        assert_eq!(loaded.active_profile, "work");
        assert!(loaded.profiles.contains_key("work"));
        let work_profile = loaded.profiles.get("work").unwrap();
        assert!(work_profile.has_package("hello"));
    }

    #[test]
    fn test_create_profile() {
        let mut config = NixyConfig::default();
        config.create_profile("work").unwrap();
        assert!(config.profile_exists("work"));
        assert!(config.profile_exists("default"));
    }

    #[test]
    fn test_delete_profile() {
        let mut config = NixyConfig::default();
        config.create_profile("work").unwrap();
        config.delete_profile("work").unwrap();
        assert!(!config.profile_exists("work"));
    }

    #[test]
    fn test_delete_active_profile_fails() {
        let mut config = NixyConfig::default();
        let result = config.delete_profile("default");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_active_profile() {
        let mut config = NixyConfig::default();
        config.create_profile("work").unwrap();
        config.set_active_profile("work").unwrap();
        assert_eq!(config.active_profile, "work");
    }

    #[test]
    fn test_set_active_profile_nonexistent_fails() {
        let mut config = NixyConfig::default();
        let result = config.set_active_profile("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_profiles() {
        let mut config = NixyConfig::default();
        config.create_profile("work").unwrap();
        config.create_profile("personal").unwrap();
        let profiles = config.list_profiles();
        assert_eq!(profiles, vec!["default", "personal", "work"]);
    }

    #[test]
    fn test_profile_config_add_package() {
        let mut profile = ProfileConfig::default();
        profile.add_package("hello");
        profile.add_package("world");
        assert!(profile.has_package("hello"));
        assert!(profile.has_package("world"));
    }

    #[test]
    fn test_profile_config_add_resolved_package() {
        let mut profile = ProfileConfig::default();
        profile.add_resolved_package(ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123".to_string(),
            platforms: None,
        });
        assert!(profile.has_package("nodejs"));
        assert_eq!(
            profile
                .get_resolved_package("nodejs")
                .unwrap()
                .resolved_version,
            "20.11.0"
        );
    }

    #[test]
    fn test_profile_config_add_custom_package() {
        let mut profile = ProfileConfig::default();
        profile.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
            platforms: None,
        });
        assert!(profile.has_package("neovim"));
    }

    #[test]
    fn test_profile_config_remove_package() {
        let mut profile = ProfileConfig::default();
        profile.add_package("hello");
        assert!(profile.remove_package("hello"));
        assert!(!profile.has_package("hello"));
    }

    #[test]
    fn test_profile_config_to_package_state() {
        let mut profile = ProfileConfig::default();
        profile.add_package("hello");
        profile.add_resolved_package(ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123".to_string(),
            platforms: None,
        });

        let state: crate::state::PackageState = (&profile).into();
        assert!(state.packages.contains(&"hello".to_string()));
        assert!(state.resolved_packages.iter().any(|p| p.name == "nodejs"));
    }
}
