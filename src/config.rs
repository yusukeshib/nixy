//! Configuration and path management for nixy.
//!
//! This module defines the directory structure and configuration paths used by nixy.
//! It follows XDG-style conventions (`~/.config/nixy`, `~/.local/state/nixy`) for
//! cross-platform compatibility.
//!
//! ## Directory Structure
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
//!
//! The configuration can be overridden via environment variables:
//! - `NIXY_CONFIG_DIR`: Override the config directory
//! - `NIXY_STATE_DIR`: Override the state directory
//! - `NIXY_ENV`: Override the environment symlink location

use std::path::PathBuf;

/// Application configuration paths
pub struct Config {
    /// Config directory (~/.config/nixy)
    pub config_dir: PathBuf,
    /// Nixy JSON config file (~/.config/nixy/nixy.json) - NEW
    pub nixy_json: PathBuf,
    /// Global local packages directory (~/.config/nixy/packages) - NEW
    pub global_packages_dir: PathBuf,
    /// State directory (~/.local/state/nixy) - NEW (used in tests)
    #[allow(dead_code)]
    pub state_dir: PathBuf,
    /// Profiles state directory (~/.local/state/nixy/profiles) - NEW
    pub profiles_state_dir: PathBuf,
    /// Legacy profiles directory (~/.config/nixy/profiles) - for migration
    pub profiles_dir: PathBuf,
    /// Legacy active profile file (~/.config/nixy/active) - for migration
    pub active_file: PathBuf,
    /// Environment symlink (~/.local/state/nixy/env)
    pub env_link: PathBuf,
    /// Legacy flake location (~/.config/nixy/flake.nix) - for migration
    pub legacy_flake: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        // Use ~/.config/nixy to match bash script behavior (XDG-style, not platform-specific)
        let config_dir = std::env::var("NIXY_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config/nixy")
            });

        // State directory for generated files (flake.nix, flake.lock)
        let state_dir = std::env::var("NIXY_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/state/nixy")
            });

        // Use ~/.local/state/nixy/env to match bash script behavior
        let env_link = std::env::var("NIXY_ENV")
            .map(PathBuf::from)
            .unwrap_or_else(|_| state_dir.join("env"));

        Self {
            nixy_json: config_dir.join("nixy.json"),
            global_packages_dir: config_dir.join("packages"),
            profiles_state_dir: state_dir.join("profiles"),
            // Legacy paths for migration
            profiles_dir: config_dir.join("profiles"),
            active_file: config_dir.join("active"),
            legacy_flake: config_dir.join("flake.nix"),
            config_dir,
            state_dir,
            env_link,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// Default profile name
pub const DEFAULT_PROFILE: &str = "default";

/// Nix experimental features flags
pub const NIX_FLAGS: &[&str] = &[
    "--extra-experimental-features",
    "nix-command",
    "--extra-experimental-features",
    "flakes",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::{Mutex, MutexGuard};

    // Mutex to serialize tests that modify environment variables
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Guard that saves and restores environment variables on drop
    struct EnvGuard {
        _mutex_guard: MutexGuard<'static, ()>,
        saved_config_dir: Option<String>,
        saved_state_dir: Option<String>,
        saved_env: Option<String>,
    }

    impl EnvGuard {
        fn new() -> Self {
            let guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            Self {
                _mutex_guard: guard,
                saved_config_dir: env::var("NIXY_CONFIG_DIR").ok(),
                saved_state_dir: env::var("NIXY_STATE_DIR").ok(),
                saved_env: env::var("NIXY_ENV").ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // Restore original values
            match &self.saved_config_dir {
                Some(val) => env::set_var("NIXY_CONFIG_DIR", val),
                None => env::remove_var("NIXY_CONFIG_DIR"),
            }
            match &self.saved_state_dir {
                Some(val) => env::set_var("NIXY_STATE_DIR", val),
                None => env::remove_var("NIXY_STATE_DIR"),
            }
            match &self.saved_env {
                Some(val) => env::set_var("NIXY_ENV", val),
                None => env::remove_var("NIXY_ENV"),
            }
        }
    }

    #[test]
    fn test_config_uses_dot_config_not_platform_specific() {
        let _guard = EnvGuard::new();

        // Ensure we use ~/.config/nixy, not platform-specific paths like
        // ~/Library/Application Support on macOS
        env::remove_var("NIXY_CONFIG_DIR");
        env::remove_var("NIXY_STATE_DIR");
        env::remove_var("NIXY_ENV");

        let config = Config::new();
        let config_str = config.config_dir.to_string_lossy();

        // Should contain .config/nixy, not "Application Support" or other platform paths
        assert!(
            config_str.contains(".config/nixy"),
            "Config dir should be ~/.config/nixy, got: {}",
            config_str
        );
        assert!(
            !config_str.contains("Application Support"),
            "Should not use macOS Application Support dir"
        );
    }

    #[test]
    fn test_config_env_uses_local_state() {
        let _guard = EnvGuard::new();

        env::remove_var("NIXY_CONFIG_DIR");
        env::remove_var("NIXY_STATE_DIR");
        env::remove_var("NIXY_ENV");

        let config = Config::new();
        let env_str = config.env_link.to_string_lossy();

        assert!(
            env_str.contains(".local/state/nixy/env"),
            "Env link should be ~/.local/state/nixy/env, got: {}",
            env_str
        );
    }

    #[test]
    fn test_config_state_dir() {
        let _guard = EnvGuard::new();

        env::remove_var("NIXY_CONFIG_DIR");
        env::remove_var("NIXY_STATE_DIR");
        env::remove_var("NIXY_ENV");

        let config = Config::new();
        let state_str = config.state_dir.to_string_lossy();

        assert!(
            state_str.contains(".local/state/nixy"),
            "State dir should be ~/.local/state/nixy, got: {}",
            state_str
        );
    }

    #[test]
    fn test_config_nixy_json_path() {
        let _guard = EnvGuard::new();

        env::remove_var("NIXY_CONFIG_DIR");
        env::remove_var("NIXY_STATE_DIR");
        env::remove_var("NIXY_ENV");

        let config = Config::new();
        let nixy_json_str = config.nixy_json.to_string_lossy();

        assert!(
            nixy_json_str.contains(".config/nixy/nixy.json"),
            "nixy.json should be ~/.config/nixy/nixy.json, got: {}",
            nixy_json_str
        );
    }

    #[test]
    fn test_config_profiles_state_dir() {
        let _guard = EnvGuard::new();

        env::remove_var("NIXY_CONFIG_DIR");
        env::remove_var("NIXY_STATE_DIR");
        env::remove_var("NIXY_ENV");

        let config = Config::new();
        let profiles_state_str = config.profiles_state_dir.to_string_lossy();

        assert!(
            profiles_state_str.contains(".local/state/nixy/profiles"),
            "Profiles state dir should be ~/.local/state/nixy/profiles, got: {}",
            profiles_state_str
        );
    }

    #[test]
    fn test_config_respects_env_vars() {
        let _guard = EnvGuard::new();

        env::set_var("NIXY_CONFIG_DIR", "/custom/config");
        env::set_var("NIXY_STATE_DIR", "/custom/state");
        env::set_var("NIXY_ENV", "/custom/env");

        let config = Config::new();

        assert_eq!(
            config.config_dir,
            PathBuf::from("/custom/config"),
            "Should respect NIXY_CONFIG_DIR"
        );
        assert_eq!(
            config.state_dir,
            PathBuf::from("/custom/state"),
            "Should respect NIXY_STATE_DIR"
        );
        assert_eq!(
            config.env_link,
            PathBuf::from("/custom/env"),
            "Should respect NIXY_ENV"
        );
        assert_eq!(
            config.nixy_json,
            PathBuf::from("/custom/config/nixy.json"),
            "nixy.json should be under NIXY_CONFIG_DIR"
        );
        assert_eq!(
            config.profiles_state_dir,
            PathBuf::from("/custom/state/profiles"),
            "Profiles state dir should be under NIXY_STATE_DIR"
        );
    }
}
