use std::fs;
use std::process::Command;

use crate::cli::UninstallArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::generate_flake;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

use super::info;

pub fn run(config: &Config, args: UninstallArgs) -> Result<()> {
    let package = &args.package;
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");
    let state_path = get_state_path(&flake_dir);

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    // Load state
    let mut state = PackageState::load(&state_path)?;

    info(&format!("Uninstalling {}...", package));

    // Remove local package file or flake directory if exists
    let pkg_dir = flake_dir.join("packages");
    let local_pkg_file = pkg_dir.join(format!("{}.nix", package));
    let local_flake_dir = pkg_dir.join(package);

    if local_pkg_file.exists() {
        info(&format!(
            "Removing local package definition: {}",
            local_pkg_file.display()
        ));
        fs::remove_file(&local_pkg_file)?;
        git_rm(&flake_dir, &format!("packages/{}.nix", package));
    } else if local_flake_dir.exists() && local_flake_dir.join("flake.nix").exists() {
        info(&format!(
            "Removing local flake: {}",
            local_flake_dir.display()
        ));
        fs::remove_dir_all(&local_flake_dir)?;
        git_rm_recursive(&flake_dir, &format!("packages/{}", package));
    }

    // Remove package from state
    state.remove_package(package);
    state.save(&state_path)?;

    // Regenerate flake.nix
    let content = generate_flake(&state, Some(&flake_dir));
    fs::write(&flake_path, content)?;
    super::success(&format!("Removed {} from flake.nix", package));

    info("Rebuilding environment...");
    super::sync::run(config)?;

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
