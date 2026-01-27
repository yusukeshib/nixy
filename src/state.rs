//! Package state management for nixy.
//!
//! This module handles the persistent state of installed packages, stored in `packages.json`.
//! It tracks both standard nixpkgs packages and custom packages from external flakes.
//!
//! The state file uses atomic writes (write to temp file, then rename) to prevent
//! corruption if a write is interrupted.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Custom package installed from a flake registry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomPackage {
    pub name: String,
    pub input_name: String,
    pub input_url: String,
    pub package_output: String, // e.g., "packages" or "legacyPackages"
    #[serde(default)]
    pub source_name: Option<String>, // The actual package name in the source flake (for aliases)
}

impl CustomPackage {
    /// Get the source package name (falls back to name if not set)
    pub fn source_package_name(&self) -> &str {
        self.source_name.as_deref().unwrap_or(&self.name)
    }
}

/// State file for tracking installed packages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageState {
    pub version: u32,
    pub packages: Vec<String>,
    #[serde(default)]
    pub custom_packages: Vec<CustomPackage>,
}

impl Default for PackageState {
    fn default() -> Self {
        Self {
            version: 1,
            packages: Vec::new(),
            custom_packages: Vec::new(),
        }
    }
}

impl PackageState {
    /// Load state from a file, or return default if file doesn't exist
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(|e| Error::StateFile(e.to_string()))
    }

    /// Save state to a file atomically
    pub fn save(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content =
            serde_json::to_string_pretty(self).map_err(|e| Error::StateFile(e.to_string()))?;

        // Write to a temporary file first, then atomically rename it into place
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &content)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Add a standard nixpkgs package
    pub fn add_package(&mut self, name: &str) {
        if !self.packages.contains(&name.to_string()) {
            self.packages.push(name.to_string());
            self.packages.sort();
        }
    }

    /// Add a custom package from a registry
    pub fn add_custom_package(&mut self, pkg: CustomPackage) {
        // Remove any existing package with the same name
        self.custom_packages.retain(|p| p.name != pkg.name);
        self.custom_packages.push(pkg);
        self.custom_packages.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Remove a package by name (checks both standard and custom)
    pub fn remove_package(&mut self, name: &str) -> bool {
        let removed_standard = self.packages.iter().position(|p| p == name).map(|i| {
            self.packages.remove(i);
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

        removed_standard.unwrap_or(false) || removed_custom.unwrap_or(false)
    }

    /// Check if a package is installed (either standard or custom)
    pub fn has_package(&self, name: &str) -> bool {
        self.packages.contains(&name.to_string())
            || self.custom_packages.iter().any(|p| p.name == name)
    }

    /// Get all package names (standard + custom)
    #[allow(dead_code)]
    pub fn all_package_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.packages.clone();
        names.extend(self.custom_packages.iter().map(|p| p.name.clone()));
        names.sort();
        names
    }
}

/// Get the state file path for a profile
pub fn get_state_path(profile_dir: &Path) -> std::path::PathBuf {
    profile_dir.join("packages.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_state() {
        let state = PackageState::default();
        assert_eq!(state.version, 1);
        assert!(state.packages.is_empty());
        assert!(state.custom_packages.is_empty());
    }

    #[test]
    fn test_add_package() {
        let mut state = PackageState::default();
        state.add_package("ripgrep");
        state.add_package("fzf");

        assert_eq!(state.packages.len(), 2);
        assert!(state.has_package("ripgrep"));
        assert!(state.has_package("fzf"));
    }

    #[test]
    fn test_add_package_deduplication() {
        let mut state = PackageState::default();
        state.add_package("ripgrep");
        state.add_package("ripgrep");

        assert_eq!(state.packages.len(), 1);
    }

    #[test]
    fn test_add_package_sorted() {
        let mut state = PackageState::default();
        state.add_package("zsh");
        state.add_package("bat");
        state.add_package("fzf");

        assert_eq!(state.packages, vec!["bat", "fzf", "zsh"]);
    }

    #[test]
    fn test_add_custom_package() {
        let mut state = PackageState::default();
        let pkg = CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
        };
        state.add_custom_package(pkg.clone());

        assert_eq!(state.custom_packages.len(), 1);
        assert!(state.has_package("neovim"));
    }

    #[test]
    fn test_add_custom_package_replaces_existing() {
        let mut state = PackageState::default();
        let pkg1 = CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-old".to_string(),
            input_url: "github:old/overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
        };
        state.add_custom_package(pkg1);

        let pkg2 = CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-new".to_string(),
            input_url: "github:new/overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
        };
        state.add_custom_package(pkg2);

        assert_eq!(state.custom_packages.len(), 1);
        assert_eq!(state.custom_packages[0].input_name, "neovim-new");
    }

    #[test]
    fn test_remove_package() {
        let mut state = PackageState::default();
        state.add_package("ripgrep");
        state.add_package("fzf");

        assert!(state.remove_package("ripgrep"));
        assert!(!state.has_package("ripgrep"));
        assert!(state.has_package("fzf"));
    }

    #[test]
    fn test_remove_custom_package() {
        let mut state = PackageState::default();
        let pkg = CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
        };
        state.add_custom_package(pkg);

        assert!(state.remove_package("neovim"));
        assert!(!state.has_package("neovim"));
    }

    #[test]
    fn test_remove_nonexistent_package() {
        let mut state = PackageState::default();
        assert!(!state.remove_package("nonexistent"));
    }

    #[test]
    fn test_all_package_names() {
        let mut state = PackageState::default();
        state.add_package("ripgrep");
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
        });

        let names = state.all_package_names();
        assert_eq!(names, vec!["neovim", "ripgrep"]);
    }

    #[test]
    fn test_save_and_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("packages.json");

        let mut state = PackageState::default();
        state.add_package("ripgrep");
        state.add_custom_package(CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-nightly".to_string(),
            input_url: "github:nix-community/neovim-nightly-overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
        });

        state.save(&path).unwrap();

        let loaded = PackageState::load(&path).unwrap();
        assert_eq!(loaded.packages, state.packages);
        assert_eq!(loaded.custom_packages.len(), state.custom_packages.len());
        assert_eq!(
            loaded.custom_packages[0].name,
            state.custom_packages[0].name
        );
    }

    #[test]
    fn test_load_nonexistent_returns_default() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nonexistent.json");

        let state = PackageState::load(&path).unwrap();
        assert!(state.packages.is_empty());
        assert!(state.custom_packages.is_empty());
    }
}
