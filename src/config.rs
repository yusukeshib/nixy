use std::path::PathBuf;

/// Application configuration paths
pub struct Config {
    /// Config directory (~/.config/nixy)
    pub config_dir: PathBuf,
    /// Profiles directory (~/.config/nixy/profiles)
    pub profiles_dir: PathBuf,
    /// Active profile file (~/.config/nixy/active)
    pub active_file: PathBuf,
    /// Environment symlink (~/.local/state/nixy/env)
    pub env_link: PathBuf,
    /// Legacy flake location (~/.config/nixy/flake.nix)
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

        // Use ~/.local/state/nixy/env to match bash script behavior
        let env_link = std::env::var("NIXY_ENV")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/state/nixy/env")
            });

        Self {
            profiles_dir: config_dir.join("profiles"),
            active_file: config_dir.join("active"),
            legacy_flake: config_dir.join("flake.nix"),
            config_dir,
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

    #[test]
    fn test_config_uses_dot_config_not_platform_specific() {
        // Ensure we use ~/.config/nixy, not platform-specific paths like
        // ~/Library/Application Support on macOS
        env::remove_var("NIXY_CONFIG_DIR");
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
        env::remove_var("NIXY_CONFIG_DIR");
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
    fn test_config_respects_env_vars() {
        env::set_var("NIXY_CONFIG_DIR", "/custom/config");
        env::set_var("NIXY_ENV", "/custom/env");

        let config = Config::new();

        assert_eq!(
            config.config_dir,
            PathBuf::from("/custom/config"),
            "Should respect NIXY_CONFIG_DIR"
        );
        assert_eq!(
            config.env_link,
            PathBuf::from("/custom/env"),
            "Should respect NIXY_ENV"
        );

        env::remove_var("NIXY_CONFIG_DIR");
        env::remove_var("NIXY_ENV");
    }
}
