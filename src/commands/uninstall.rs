use std::fs;
use std::process::Command;

use regex::Regex;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::editor::remove_from_section;
use crate::flake::is_nixy_managed;
use crate::profile::get_flake_dir;

use super::info;

pub fn run(config: &Config, package: &str) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    if !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

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

    // Remove from flake.nix
    remove_package_from_flake(config, package)?;

    info("Rebuilding environment...");
    super::sync::run(config)?;

    Ok(())
}

/// Remove a package from flake.nix
fn remove_package_from_flake(config: &Config, pkg: &str) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    if !is_nixy_managed(&flake_path) {
        return Err(Error::NotNixyManaged);
    }

    let content = fs::read_to_string(&flake_path)?;

    // Remove from packages section
    let pkg_pattern = Regex::new(&format!(
        r"^\s*{} = pkgs\.{};",
        regex::escape(pkg),
        regex::escape(pkg)
    ))?;
    let content = remove_from_section(
        &content,
        "# [nixy:packages]",
        "# [/nixy:packages]",
        &pkg_pattern,
    );

    // Remove from env-paths section
    let path_pattern = Regex::new(&format!(r"^\s*{}$", regex::escape(pkg)))?;
    let content = remove_from_section(
        &content,
        "# [nixy:env-paths]",
        "# [/nixy:env-paths]",
        &path_pattern,
    );

    fs::write(&flake_path, content)?;
    super::success(&format!("Removed {} from flake.nix", pkg));

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
