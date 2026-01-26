use std::fs;
use std::process::Command;

use crate::cli::UninstallArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::generate_flake;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

use super::{info, warn};

pub fn run(config: &Config, args: UninstallArgs) -> Result<()> {
    let package = &args.package;
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");
    let state_path = get_state_path(&flake_dir);

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
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
