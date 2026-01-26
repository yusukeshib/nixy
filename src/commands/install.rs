use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cli::InstallArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::is_flake_file;
use crate::flake::parser::parse_local_package_attr;
use crate::flake::template::generate_flake;
use crate::nix::Nix;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, CustomPackage, PackageState};

use super::{info, success, warn};

pub fn run(config: &Config, args: InstallArgs) -> Result<()> {
    // Handle --file option
    if let Some(file) = args.file {
        return install_from_file(config, &file);
    }

    // Handle --from option
    if let Some(from) = args.from {
        let pkg = args
            .package
            .ok_or_else(|| Error::Usage("Package name is required with --from".to_string()))?;
        return install_from_registry(config, &from, &pkg);
    }

    // Standard nixpkgs install
    let pkg = args.package.ok_or_else(|| {
        Error::Usage(
            "Usage: nixy install <package> or nixy install --file <path> or nixy install --from <registry> <package>".to_string(),
        )
    })?;

    // Get flake directory
    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);

    // Load state
    let mut state = PackageState::load(&state_path)?;

    // Check if package is already installed
    if state.has_package(&pkg) {
        success(&format!("Package '{}' is already installed", pkg));
        return Ok(());
    }

    // Validate package exists in nixpkgs
    info(&format!("Validating package {}...", pkg));
    if !Nix::validate_package(&pkg)? {
        return Err(Error::PackageNotFound(pkg));
    }

    // Save original state for rollback
    let original_state = state.clone();

    // Add package to state
    state.add_package(&pkg);
    state.save(&state_path)?;

    // Regenerate flake.nix (rollback state if this fails)
    if let Err(e) = regenerate_flake(&flake_dir, &state) {
        original_state.save(&state_path)?;
        warn("Failed to regenerate flake.nix. Reverted changes.");
        return Err(e);
    }

    info(&format!("Installing {}...", pkg));
    if let Err(e) = super::sync::run(config) {
        // Sync failed, revert state and flake
        original_state.save(&state_path)?;
        let _ = regenerate_flake(&flake_dir, &original_state);
        warn("Sync failed. Reverted changes.");
        return Err(e);
    }

    Ok(())
}

/// Install from a flake registry or direct URL
fn install_from_registry(config: &Config, from_arg: &str, pkg: &str) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);

    // Load state
    let mut state = PackageState::load(&state_path)?;

    // Check if package is already installed
    if state.has_package(pkg) {
        success(&format!("Package '{}' is already installed", pkg));
        return Ok(());
    }

    let flake_url = if from_arg.contains(':') {
        // Direct flake URL
        info(&format!("Using flake URL: {}", from_arg));
        from_arg.to_string()
    } else {
        // Registry lookup
        info(&format!("Looking up '{}' in nix registry...", from_arg));
        let url = Nix::registry_lookup(from_arg)?
            .ok_or_else(|| Error::RegistryNotFound(from_arg.to_string()))?;
        info(&format!("Found: {}", url));
        url
    };

    // Derive input name
    let input_name = if !from_arg.contains(':') {
        // Use registry alias
        sanitize_input_name(from_arg)
    } else {
        // Derive from URL (owner-repo)
        derive_input_name_from_url(&flake_url)
    };

    // Validate the package exists
    info(&format!(
        "Validating package '{}' in {}...",
        pkg, input_name
    ));
    let pkg_output = Nix::validate_flake_package(&flake_url, pkg)?.ok_or_else(|| {
        let available = Nix::list_flake_packages(&flake_url, None)
            .unwrap_or_default()
            .into_iter()
            .take(10)
            .collect::<Vec<_>>()
            .join(" ");
        if available.is_empty() {
            Error::FlakePackageNotFound(pkg.to_string(), input_name.clone())
        } else {
            Error::Usage(format!(
                "Package '{}' not found in '{}'. Available packages: {}...",
                pkg, input_name, available
            ))
        }
    })?;

    // Save original state for rollback
    let original_state = state.clone();

    // Determine final input name and URL for this package
    let (final_input_name, final_url) = (input_name.clone(), flake_url.clone());

    // Add custom package to state
    state.add_custom_package(CustomPackage {
        name: pkg.to_string(),
        input_name: final_input_name.clone(),
        input_url: final_url,
        package_output: pkg_output,
        source_name: None,
    });
    state.save(&state_path)?;

    // Regenerate flake.nix (rollback state if this fails)
    if let Err(e) = regenerate_flake(&flake_dir, &state) {
        original_state.save(&state_path)?;
        warn("Failed to regenerate flake.nix. Reverted changes.");
        return Err(e);
    }

    info(&format!("Installing {} from {}...", pkg, final_input_name));
    if let Err(e) = super::sync::run(config) {
        // Sync failed, revert state and flake
        original_state.save(&state_path)?;
        let _ = regenerate_flake(&flake_dir, &original_state);
        warn("Sync failed. Reverted changes.");
        return Err(e);
    }

    Ok(())
}

/// Install from a local nix file
fn install_from_file(config: &Config, file: &Path) -> Result<()> {
    if !file.exists() {
        return Err(Error::FileNotFound(file.display().to_string()));
    }

    // Check if this is a flake file
    if is_flake_file(file) {
        return install_from_flake_file(config, file);
    }

    // Extract package name
    let content = fs::read_to_string(file)?;
    let pkg_name = parse_local_package_attr(&content, "pname")
        .or_else(|| parse_local_package_attr(&content, "name"))
        .ok_or_else(|| Error::NoPackageName(file.display().to_string()))?;

    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);

    // Load state
    let state = PackageState::load(&state_path)?;

    // Check if package is already installed
    if state.has_package(&pkg_name) {
        success(&format!("Package '{}' is already installed", pkg_name));
        return Ok(());
    }

    info(&format!(
        "Installing local package: {} from {}",
        pkg_name,
        file.display()
    ));

    // Save original state for rollback
    let original_state = state.clone();

    // Create packages directory
    let pkg_dir = flake_dir.join("packages");
    fs::create_dir_all(&pkg_dir)?;

    // Copy file to packages directory
    let dest = pkg_dir.join(format!("{}.nix", pkg_name));
    fs::copy(file, &dest)?;
    success(&format!("Copied package definition to {}", dest.display()));

    // Add to git if in a git repo
    git_add(&flake_dir, &format!("packages/{}.nix", pkg_name));

    // Regenerate flake.nix (local packages are auto-discovered)
    regenerate_flake(&flake_dir, &state)?;

    info(&format!("Installing {}...", pkg_name));
    if let Err(e) = super::sync::run(config) {
        // Sync failed, revert changes
        original_state.save(&state_path)?;
        let _ = fs::remove_file(&dest);
        regenerate_flake(&flake_dir, &original_state)?;
        warn("Sync failed. Reverted changes.");
        return Err(e);
    }

    Ok(())
}

/// Install from a local flake file
fn install_from_flake_file(config: &Config, file: &Path) -> Result<()> {
    // Extract package name from filename
    let pkg_name = file
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_input_name)
        .ok_or_else(|| Error::InvalidFilename(file.display().to_string()))?;

    if pkg_name.is_empty() {
        return Err(Error::InvalidFilename(file.display().to_string()));
    }

    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);

    // Load state
    let state = PackageState::load(&state_path)?;

    // Check if package is already installed
    if state.has_package(&pkg_name) {
        success(&format!("Package '{}' is already installed", pkg_name));
        return Ok(());
    }

    info(&format!(
        "Installing local flake: {} from {}",
        pkg_name,
        file.display()
    ));

    // Save original state for rollback
    let original_state = state.clone();

    // Create package directory
    let pkg_dir = flake_dir.join("packages").join(&pkg_name);
    fs::create_dir_all(&pkg_dir)?;

    // Copy file as flake.nix
    let dest = pkg_dir.join("flake.nix");
    fs::copy(file, &dest)?;
    success(&format!("Copied flake to {}", dest.display()));

    // Add to git if in a git repo
    git_add(&flake_dir, &format!("packages/{}/flake.nix", pkg_name));

    // Regenerate flake.nix (local flakes are auto-discovered)
    regenerate_flake(&flake_dir, &state)?;

    info(&format!("Installing {}...", pkg_name));
    if let Err(e) = super::sync::run(config) {
        // Sync failed, revert changes
        original_state.save(&state_path)?;
        let _ = fs::remove_dir_all(&pkg_dir);
        regenerate_flake(&flake_dir, &original_state)?;
        warn("Sync failed. Reverted changes.");
        return Err(e);
    }

    Ok(())
}

/// Regenerate flake.nix from state
fn regenerate_flake(flake_dir: &Path, state: &PackageState) -> Result<()> {
    let flake_path = flake_dir.join("flake.nix");
    fs::create_dir_all(flake_dir)?;
    let content = generate_flake(state, Some(flake_dir));
    fs::write(&flake_path, content)?;
    Ok(())
}

/// Sanitize a string for use as an input name
fn sanitize_input_name(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    sanitized.trim_matches('-').to_string()
}

/// Derive an input name from a flake URL
fn derive_input_name_from_url(url: &str) -> String {
    // Try to extract owner-repo from URL
    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() >= 2 {
        let owner = parts[parts.len() - 2];
        let repo = parts[parts.len() - 1].trim_end_matches(".git");
        sanitize_input_name(&format!("{}-{}", owner, repo))
    } else {
        "custom-flake".to_string()
    }
}

/// Add a file to git if in a git repo
fn git_add(dir: &Path, file: &str) {
    // Check if in a git repo
    let is_git_repo = dir.join(".git").exists()
        || Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    if is_git_repo {
        let _ = Command::new("git")
            .args(["-C", &dir.to_string_lossy(), "add", file])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_input_name() {
        assert_eq!(sanitize_input_name("nixpkgs"), "nixpkgs");
        assert_eq!(sanitize_input_name("foo-bar"), "foo-bar");
        assert_eq!(sanitize_input_name("foo_bar"), "foo-bar");
        assert_eq!(sanitize_input_name("foo/bar"), "foo-bar");
        assert_eq!(sanitize_input_name("--foo--"), "foo");
    }

    #[test]
    fn test_derive_input_name_from_url() {
        assert_eq!(
            derive_input_name_from_url("github:NixOS/nixpkgs"),
            "github-NixOS-nixpkgs"
        );
        assert_eq!(
            derive_input_name_from_url("github:user/repo.git"),
            "github-user-repo"
        );
    }

    #[test]
    fn test_regenerate_flake() {
        let temp = TempDir::new().unwrap();
        let flake_dir = temp.path();

        let mut state = PackageState::default();
        state.add_package("hello");

        regenerate_flake(flake_dir, &state).unwrap();

        let flake_path = flake_dir.join("flake.nix");
        assert!(flake_path.exists());

        let content = fs::read_to_string(&flake_path).unwrap();
        assert!(content.contains("hello = pkgs.hello;"));
    }
}
