use std::fs;
use std::process::Command;

use crate::cli::UninstallArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::{generate_flake, regenerate_flake, regenerate_flake_from_profile};
use crate::nixy_config::{nixy_json_exists, NixyConfig};
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

use super::{info, warn};

pub fn run(config: &Config, args: UninstallArgs) -> Result<()> {
    let package = &args.package;

    // Use NixyConfig if available (new format)
    if nixy_json_exists(config) {
        return uninstall_with_nixy_config(config, package);
    }

    // Legacy format
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");
    let state_path = get_state_path(&flake_dir);

    // Auto-regenerate flake.nix if missing
    if !flake_path.exists() {
        let state = PackageState::load(&state_path)?;
        info("Regenerating flake.nix from packages.json...");
        regenerate_flake(&flake_dir, &state)?;
    }

    // Load state and save original for rollback
    let mut state = PackageState::load(&state_path)?;
    let original_state = state.clone();
    let original_flake = fs::read_to_string(&flake_path)?;

    info(&format!("Uninstalling {}...", package));

    // Remove local package file or flake directory if exists
    let pkg_dir = flake_dir.join("packages");
    let local_pkg_file = pkg_dir.join(format!("{}.nix", package));
    let local_flake_dir = pkg_dir.join(package);

    let mut removed_local = false;
    if local_pkg_file.exists() {
        info(&format!(
            "Removing local package definition: {}",
            local_pkg_file.display()
        ));
        fs::remove_file(&local_pkg_file)?;
        git_rm(&flake_dir, &format!("packages/{}.nix", package));
        removed_local = true;
    } else if local_flake_dir.exists() && local_flake_dir.join("flake.nix").exists() {
        info(&format!(
            "Removing local flake: {}",
            local_flake_dir.display()
        ));
        fs::remove_dir_all(&local_flake_dir)?;
        git_rm_recursive(&flake_dir, &format!("packages/{}", package));
        removed_local = true;
    }

    // Remove package from state (local packages are auto-discovered, not in state)
    let removed_from_state = state.remove_package(package);
    if !removed_local && !removed_from_state {
        return Err(Error::PackageNotFound(package.to_string()));
    }
    state.save(&state_path)?;

    // Regenerate flake.nix
    let content = generate_flake(&state, Some(&flake_dir));
    fs::write(&flake_path, content)?;
    super::success(&format!("Removed {} from flake.nix", package));

    info("Rebuilding environment...");
    if let Err(e) = super::sync::run(config) {
        // Sync failed, revert state and flake (note: local file deletions cannot be undone)
        original_state.save(&state_path)?;
        fs::write(&flake_path, original_flake)?;
        warn("Sync failed. Reverted state and flake.nix (local file deletions cannot be undone).");
        return Err(e);
    }

    Ok(())
}

/// Uninstall a package using the new nixy.json format
fn uninstall_with_nixy_config(config: &Config, package: &str) -> Result<()> {
    let mut nixy_config = NixyConfig::load(config)?;
    let active_profile = nixy_config.active_profile.clone();
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    // Auto-regenerate flake.nix if missing
    if !flake_path.exists() {
        if let Some(profile) = nixy_config.get_active_profile() {
            info("Regenerating flake.nix from nixy.json...");
            let global_packages_dir = if config.global_packages_dir.exists() {
                Some(config.global_packages_dir.as_path())
            } else {
                None
            };
            regenerate_flake_from_profile(&flake_dir, profile, global_packages_dir)?;
        }
    }

    // Save original for rollback
    let original_config = nixy_config.clone();
    let original_flake = if flake_path.exists() {
        Some(fs::read_to_string(&flake_path)?)
    } else {
        None
    };

    info(&format!("Uninstalling {}...", package));

    // Note: We do NOT delete global package definitions from packages/ directory.
    // The packages/ directory is shared across all profiles in the new format,
    // so uninstalling from one profile must not delete the global package definition.
    // Global cleanup, if desired, should be handled manually or by a dedicated command.
    let global_pkg_file = config.global_packages_dir.join(format!("{}.nix", package));
    let global_flake_dir = config.global_packages_dir.join(package);

    if global_pkg_file.exists()
        || (global_flake_dir.exists() && global_flake_dir.join("flake.nix").exists())
    {
        warn(
            "Note: Local package definition in packages/ was not removed (shared across profiles)",
        );
    }

    // Remove package from profile
    let profile = nixy_config
        .get_active_profile_mut()
        .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
    let removed_from_config = profile.remove_package(package);

    if !removed_from_config {
        return Err(Error::PackageNotFound(package.to_string()));
    }

    nixy_config.save(config)?;

    // Regenerate flake.nix
    let global_packages_dir = if config.global_packages_dir.exists() {
        Some(config.global_packages_dir.as_path())
    } else {
        None
    };
    let profile_for_flake = nixy_config.get_active_profile().unwrap();
    regenerate_flake_from_profile(&flake_dir, profile_for_flake, global_packages_dir)?;
    super::success(&format!("Removed {} from flake.nix", package));

    info("Rebuilding environment...");
    if let Err(e) = super::sync::run(config) {
        // Sync failed, revert
        original_config.save(config)?;
        if let Some(original) = original_flake {
            fs::write(&flake_path, original)?;
        }
        warn("Sync failed. Reverted nixy.json and flake.nix.");
        return Err(e);
    }

    Ok(())
}

/// Remove a file from git index
fn git_rm(dir: &std::path::Path, file: &str) {
    let is_git_repo = dir.join(".git").exists()
        || Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    if is_git_repo {
        let _ = Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rm", "--cached", file])
            .output();
    }
}

/// Remove a directory from git index recursively
fn git_rm_recursive(dir: &std::path::Path, path: &str) {
    let is_git_repo = dir.join(".git").exists()
        || Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    if is_git_repo {
        let _ = Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rm", "-r", "--cached", path])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_uninstall_updates_state() {
        let temp = TempDir::new().unwrap();
        let flake_dir = temp.path();
        let state_path = get_state_path(flake_dir);

        // Create initial state with packages
        let mut state = PackageState::default();
        state.add_package("hello");
        state.add_package("world");
        state.save(&state_path).unwrap();

        // Create flake.nix
        let content = generate_flake(&state, Some(flake_dir));
        fs::write(flake_dir.join("flake.nix"), content).unwrap();

        // Load and remove package
        let mut loaded_state = PackageState::load(&state_path).unwrap();
        loaded_state.remove_package("hello");
        loaded_state.save(&state_path).unwrap();

        // Verify state is updated
        let final_state = PackageState::load(&state_path).unwrap();
        assert!(!final_state.has_package("hello"));
        assert!(final_state.has_package("world"));
    }
}
