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
        let config_dir = std::env::var("NIXY_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::config_dir()
                    .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
                    .unwrap_or_else(|| PathBuf::from(".config"))
                    .join("nixy")
            });

        let env_link = std::env::var("NIXY_ENV")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::state_dir()
                    .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))
                    .unwrap_or_else(|| PathBuf::from(".local/state"))
                    .join("nixy/env")
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

/// nixy version
pub const VERSION: &str = "0.1.0";

/// Nix experimental features flags
pub const NIX_FLAGS: &[&str] = &[
    "--extra-experimental-features",
    "nix-command",
    "--extra-experimental-features",
    "flakes",
];
