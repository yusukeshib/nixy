//! Dynamic completion helper.
//!
//! Prints newline-separated completion candidates consumed by the shell
//! completion scripts (`src/completions/nixy.zsh`, `nixy.bash`). This keeps the
//! candidate lists (installed packages, profiles) always in sync with the real
//! config instead of being hard-coded in the shell scripts.
//!
//! All lookups swallow errors and fall back to an empty list: completion must
//! never fail or print diagnostics, even when the config is missing or invalid.

use crate::config::Config;
use crate::error::Result;
use crate::flake::parser::collect_local_packages;
use crate::nixy_config::{nixy_json_exists, NixyConfig};
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

pub fn run(config: &Config, kind: &str) -> Result<()> {
    let candidates = match kind {
        "installed" => installed_package_names(config),
        "profiles" => profile_names(config),
        // Unknown kinds print nothing so future shell scripts degrade gracefully.
        _ => Vec::new(),
    };

    for name in candidates {
        println!("{}", name);
    }

    Ok(())
}

/// Names of all installed packages in the active profile (or legacy state).
fn installed_package_names(config: &Config) -> Vec<String> {
    let mut names = Vec::new();

    if nixy_json_exists(config) {
        if let Ok(nixy_config) = NixyConfig::load(config) {
            if let Some(profile) = nixy_config.get_active_profile() {
                names.extend(profile.packages.iter().cloned());
                names.extend(profile.resolved_packages.iter().map(|p| p.name.clone()));
                names.extend(profile.custom_packages.iter().map(|p| p.name.clone()));
            }
        }

        // Local packages from the global packages/ directory.
        if config.global_packages_dir.exists() {
            let (local_packages, local_flakes) =
                collect_local_packages(&config.global_packages_dir);
            names.extend(local_packages.into_iter().map(|p| p.name));
            names.extend(local_flakes.into_iter().map(|f| f.name));
        }
    } else if let Ok(flake_dir) = get_flake_dir(config) {
        if let Ok(state) = PackageState::load(&get_state_path(&flake_dir)) {
            names.extend(state.packages.iter().cloned());
            names.extend(state.resolved_packages.iter().map(|p| p.name.clone()));
            names.extend(state.custom_packages.iter().map(|p| p.name.clone()));
        }
    }

    names.sort();
    names.dedup();
    names
}

/// Names of all configured profiles.
fn profile_names(config: &Config) -> Vec<String> {
    NixyConfig::load(config)
        .map(|c| c.list_profiles())
        .unwrap_or_default()
}
