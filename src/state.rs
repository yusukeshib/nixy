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

/// Package resolved via Nixhub API with specific nixpkgs commit
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedNixpkgPackage {
    /// Package name (e.g., "nodejs")
    pub name: String,
    /// Version spec as specified by user (e.g., "20" for semver range)
    /// None means latest, used for upgrade behavior
    #[serde(default)]
    pub version_spec: Option<String>,
    /// Resolved version (e.g., "20.11.0")
    pub resolved_version: String,
    /// Nix attribute path (e.g., "nodejs_20")
    pub attribute_path: String,
    /// nixpkgs commit hash
    pub commit_hash: String,
    /// Platform restrictions (e.g., ["x86_64-darwin", "aarch64-darwin"])
    /// None means all platforms
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platforms: Option<Vec<String>>,
}

/// Custom package installed from a flake registry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomPackage {
    pub name: String,
    pub input_name: String,
    pub input_url: String,
    pub package_output: String, // e.g., "packages" or "legacyPackages"
    #[serde(default)]
    pub source_name: Option<String>, // The actual package name in the source flake (for aliases)
    /// Platform restrictions (e.g., ["x86_64-darwin", "aarch64-darwin"])
    /// None means all platforms
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platforms: Option<Vec<String>>,
}

impl CustomPackage {
    /// Get the source package name (falls back to name if not set)
    pub fn source_package_name(&self) -> &str {
        self.source_name.as_deref().unwrap_or(&self.name)
    }
}

/// State file for tracking installed packages.
///
/// The state tracks three categories of packages:
/// - `packages`: Legacy format (v1) - simple package names like "hello", kept for backwards
///   compatibility. These are resolved via the default nixpkgs input.
/// - `resolved_packages`: Nixhub-resolved packages with pinned versions and nixpkgs commits.
///   Supports version syntax like "nodejs@20" with reproducible builds.
/// - `custom_packages`: Packages from custom flake URLs (e.g., github:owner/repo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageState {
    pub version: u32,
    /// Legacy: simple package names (version 1 format, kept for backwards compatibility)
    #[serde(default)]
    pub packages: Vec<String>,
    /// Packages resolved via Nixhub with specific versions and commits
    #[serde(default)]
    pub resolved_packages: Vec<ResolvedNixpkgPackage>,
    /// Packages from custom flake URLs
    #[serde(default)]
    pub custom_packages: Vec<CustomPackage>,
}

impl Default for PackageState {
    fn default() -> Self {
        Self {
            version: 2,
            packages: Vec::new(),
            resolved_packages: Vec::new(),
            custom_packages: Vec::new(),
        }
    }
}

impl PackageState {
    /// Load state from a file, or return default if file doesn't exist
    /// Automatically migrates from version 1 to version 2 format
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        let mut state: Self =
            serde_json::from_str(&content).map_err(|e| Error::StateFile(e.to_string()))?;

        // Migrate from version 1 to version 2 if needed
        if state.version < 2 {
            state.version = 2;
            // Note: Old packages in state.packages remain as-is for backwards compatibility
            // They will be migrated to resolved_packages when upgraded or re-added
        }

        Ok(state)
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

#[cfg(test)]
impl PackageState {
    /// Get all package names (legacy + resolved + custom) - test helper
    pub fn all_package_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.packages.clone();
        names.extend(self.resolved_packages.iter().map(|p| p.name.clone()));
        names.extend(self.custom_packages.iter().map(|p| p.name.clone()));
        names.sort();
        names.dedup();
        names
    }
}

/// Get the state file path for a profile
pub fn get_state_path(profile_dir: &Path) -> std::path::PathBuf {
    profile_dir.join("packages.json")
}

/// All valid Nix system platforms
pub const VALID_PLATFORMS: &[&str] = &[
    "x86_64-darwin",
    "aarch64-darwin",
    "x86_64-linux",
    "aarch64-linux",
];

/// Platform aliases that expand to multiple platforms
const PLATFORM_ALIASES: &[(&str, &[&str])] = &[
    ("darwin", &["x86_64-darwin", "aarch64-darwin"]),
    ("macos", &["x86_64-darwin", "aarch64-darwin"]),
    ("linux", &["x86_64-linux", "aarch64-linux"]),
];

/// Normalize platform names, expanding aliases like "darwin" to full platform names.
/// Returns an error message if any platform is invalid.
pub fn normalize_platforms(platforms: &[String]) -> std::result::Result<Vec<String>, String> {
    let mut result = Vec::new();
    for p in platforms {
        let p_lower = p.to_lowercase();
        // Check if it's an alias
        if let Some((_, expanded)) = PLATFORM_ALIASES.iter().find(|(alias, _)| *alias == p_lower) {
            for exp in *expanded {
                if !result.contains(&exp.to_string()) {
                    result.push(exp.to_string());
                }
            }
        } else if VALID_PLATFORMS.contains(&p_lower.as_str()) {
            if !result.contains(&p_lower) {
                result.push(p_lower);
            }
        } else {
            return Err(format!(
                "Invalid platform '{}'. Valid platforms: darwin, macos, linux, {}",
                p,
                VALID_PLATFORMS.join(", ")
            ));
        }
    }
    result.sort();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_state() {
        let state = PackageState::default();
        assert_eq!(state.version, 2);
        assert!(state.packages.is_empty());
        assert!(state.resolved_packages.is_empty());
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
            platforms: None,
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
            platforms: None,
        };
        state.add_custom_package(pkg1);

        let pkg2 = CustomPackage {
            name: "neovim".to_string(),
            input_name: "neovim-new".to_string(),
            input_url: "github:new/overlay".to_string(),
            package_output: "packages".to_string(),
            source_name: None,
            platforms: None,
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
            platforms: None,
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
            platforms: None,
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
            platforms: None,
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
        assert!(state.resolved_packages.is_empty());
        assert!(state.custom_packages.is_empty());
    }

    #[test]
    fn test_add_resolved_package() {
        let mut state = PackageState::default();
        let pkg = ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123".to_string(),
            platforms: None,
        };
        state.add_resolved_package(pkg.clone());

        assert_eq!(state.resolved_packages.len(), 1);
        assert!(state.has_package("nodejs"));
        assert_eq!(
            state
                .get_resolved_package("nodejs")
                .unwrap()
                .resolved_version,
            "20.11.0"
        );
    }

    #[test]
    fn test_add_resolved_package_removes_legacy() {
        let mut state = PackageState::default();
        state.add_package("nodejs");
        assert!(state.packages.contains(&"nodejs".to_string()));

        let pkg = ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123".to_string(),
            platforms: None,
        };
        state.add_resolved_package(pkg);

        // Legacy should be removed
        assert!(!state.packages.contains(&"nodejs".to_string()));
        // But resolved should exist
        assert_eq!(state.resolved_packages.len(), 1);
        assert!(state.has_package("nodejs"));
    }

    #[test]
    fn test_remove_resolved_package() {
        let mut state = PackageState::default();
        let pkg = ResolvedNixpkgPackage {
            name: "nodejs".to_string(),
            version_spec: Some("20".to_string()),
            resolved_version: "20.11.0".to_string(),
            attribute_path: "nodejs_20".to_string(),
            commit_hash: "abc123".to_string(),
            platforms: None,
        };
        state.add_resolved_package(pkg);

        assert!(state.remove_package("nodejs"));
        assert!(!state.has_package("nodejs"));
        assert!(state.resolved_packages.is_empty());
    }

    #[test]
    fn test_is_legacy_package() {
        let mut state = PackageState::default();
        state.add_package("legacy-pkg");
        state.add_resolved_package(ResolvedNixpkgPackage {
            name: "resolved-pkg".to_string(),
            version_spec: None,
            resolved_version: "1.0.0".to_string(),
            attribute_path: "resolved-pkg".to_string(),
            commit_hash: "abc123".to_string(),
            platforms: None,
        });

        assert!(state.is_legacy_package("legacy-pkg"));
        assert!(!state.is_legacy_package("resolved-pkg"));
    }

    #[test]
    fn test_migration_from_v1() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("packages.json");

        // Write a v1 state file
        let v1_content = r#"{"version":1,"packages":["ripgrep","fzf"],"custom_packages":[]}"#;
        fs::write(&path, v1_content).unwrap();

        // Load should migrate to v2
        let state = PackageState::load(&path).unwrap();
        assert_eq!(state.version, 2);
        // Legacy packages should be preserved
        assert_eq!(state.packages, vec!["ripgrep", "fzf"]);
        assert!(state.resolved_packages.is_empty());
    }

    #[test]
    fn test_normalize_platforms_darwin_alias() {
        let result = normalize_platforms(&["darwin".to_string()]).unwrap();
        assert_eq!(result, vec!["aarch64-darwin", "x86_64-darwin"]);
    }

    #[test]
    fn test_normalize_platforms_macos_alias() {
        let result = normalize_platforms(&["macos".to_string()]).unwrap();
        assert_eq!(result, vec!["aarch64-darwin", "x86_64-darwin"]);
    }

    #[test]
    fn test_normalize_platforms_linux_alias() {
        let result = normalize_platforms(&["linux".to_string()]).unwrap();
        assert_eq!(result, vec!["aarch64-linux", "x86_64-linux"]);
    }

    #[test]
    fn test_normalize_platforms_full_name() {
        let result = normalize_platforms(&["x86_64-darwin".to_string()]).unwrap();
        assert_eq!(result, vec!["x86_64-darwin"]);
    }

    #[test]
    fn test_normalize_platforms_case_insensitive() {
        let result = normalize_platforms(&["Darwin".to_string()]).unwrap();
        assert_eq!(result, vec!["aarch64-darwin", "x86_64-darwin"]);

        let result = normalize_platforms(&["X86_64-LINUX".to_string()]).unwrap();
        assert_eq!(result, vec!["x86_64-linux"]);
    }

    #[test]
    fn test_normalize_platforms_dedup() {
        let result =
            normalize_platforms(&["darwin".to_string(), "x86_64-darwin".to_string()]).unwrap();
        assert_eq!(result, vec!["aarch64-darwin", "x86_64-darwin"]);
    }

    #[test]
    fn test_normalize_platforms_invalid() {
        let result = normalize_platforms(&["windows".to_string()]);
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Invalid platform"));
    }

    #[test]
    fn test_normalize_platforms_mixed() {
        let result =
            normalize_platforms(&["darwin".to_string(), "x86_64-linux".to_string()]).unwrap();
        assert_eq!(
            result,
            vec!["aarch64-darwin", "x86_64-darwin", "x86_64-linux"]
        );
    }
}
