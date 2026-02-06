use crate::cli::InstallArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::{regenerate_flake, regenerate_flake_from_profile};
use crate::nix::Nix;
use crate::nixhub::{parse_package_spec, NixhubClient};
use crate::nixy_config::{nixy_json_exists, NixyConfig};
use crate::profile::get_flake_dir;
use crate::rollback::{self, RollbackContext};
use crate::state::{
    get_state_path, normalize_platforms, CustomPackage, PackageState, ResolvedNixpkgPackage,
};

use super::{info, success, warn};

pub fn run(config: &Config, args: InstallArgs) -> Result<()> {
    // Validate and normalize platform names early
    let platforms = if args.platform.is_empty() {
        None
    } else {
        Some(normalize_platforms(&args.platform).map_err(Error::Usage)?)
    };

    // Standard nixpkgs install (via Nixhub)
    let pkg_spec_str = args.package.ok_or_else(|| {
        Error::Usage(
            "Usage: nixy install <package>[@version] or nixy install <flake-ref>".to_string(),
        )
    })?;

    // Check if this looks like a flake reference (github:user/repo, path:./foo, etc.)
    // If so, route through install_from_registry instead of Nixhub
    if pkg_spec_str.contains(':') {
        let (flake_url, pkg) = if let Some((url, pkg_name)) = pkg_spec_str.split_once('#') {
            (url.to_string(), pkg_name.to_string())
        } else {
            // No fragment: derive the flake output/package name from the URL
            // (e.g., the repository name). The flake is expected to export a
            // package with this name, and nixy also uses it as the human-readable
            // package name, avoiding collisions with any internal "default" attr.
            let name = derive_package_name_from_url(&pkg_spec_str);
            (pkg_spec_str.clone(), name)
        };
        return install_from_flake_url(config, &flake_url, &pkg, platforms);
    }

    // Parse package spec (e.g., "nodejs@20" or "ripgrep")
    let pkg_spec = parse_package_spec(&pkg_spec_str);

    // Use NixyConfig if available (new format), otherwise fall back to legacy
    if nixy_json_exists(config) {
        return install_with_nixy_config(
            config,
            &pkg_spec.name,
            pkg_spec.version.as_deref(),
            platforms,
        );
    }

    // Legacy: Get flake directory and use PackageState
    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);

    // Load state
    let mut state = PackageState::load(&state_path)?;

    // Check if package is already installed
    if state.has_package(&pkg_spec.name) {
        success(&format!("Package '{}' is already installed", pkg_spec.name));
        return Ok(());
    }

    // Resolve package via Nixhub
    let version_display = pkg_spec.version.as_deref().unwrap_or("latest");
    info(&format!(
        "Resolving {}@{} via Nixhub...",
        pkg_spec.name, version_display
    ));

    let client = NixhubClient::new();
    let resolved = client.resolve_for_current_system(
        &pkg_spec.name,
        pkg_spec.version.as_deref().unwrap_or("latest"),
    )?;

    info(&format!(
        "Found {} version {} (commit {})",
        resolved.name,
        resolved.version,
        &resolved.commit_hash[..8.min(resolved.commit_hash.len())]
    ));

    // Save original state for rollback
    let original_state = state.clone();

    // Add resolved package to state
    state.add_resolved_package(ResolvedNixpkgPackage {
        name: resolved.name.clone(),
        version_spec: pkg_spec.version.clone(),
        resolved_version: resolved.version.clone(),
        attribute_path: resolved.attribute_path.clone(),
        commit_hash: resolved.commit_hash.clone(),
        platforms: platforms.clone(),
    });
    state.save(&state_path)?;

    // Regenerate flake.nix (rollback state if this fails)
    if let Err(e) = regenerate_flake(&flake_dir, &state) {
        original_state.save(&state_path)?;
        warn("Failed to regenerate flake.nix. Reverted changes.");
        return Err(e);
    }

    // Set up rollback context for Ctrl+C handling
    rollback::set_context(RollbackContext::legacy(
        flake_dir.clone(),
        state_path.clone(),
        original_state.clone(),
    ));

    info(&format!(
        "Installing {}@{}...",
        resolved.name, resolved.version
    ));
    if let Err(e) = super::sync::run(config) {
        // Clear rollback context since we're handling the error here
        rollback::clear_context();
        // Sync failed, revert state and flake
        original_state.save(&state_path)?;
        let _ = regenerate_flake(&flake_dir, &original_state);
        warn("Sync failed. Reverted changes.");
        return Err(e);
    }

    // Clear rollback context on success
    rollback::clear_context();

    Ok(())
}

/// Install a package using the new nixy.json format
fn install_with_nixy_config(
    config: &Config,
    name: &str,
    version: Option<&str>,
    platforms: Option<Vec<String>>,
) -> Result<()> {
    let mut nixy_config = NixyConfig::load(config)?;
    let active_profile = nixy_config.active_profile.clone();

    // Check if package is already installed (scope the borrow)
    {
        let profile = nixy_config
            .get_active_profile()
            .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
        if profile.has_package(name) {
            success(&format!("Package '{}' is already installed", name));
            return Ok(());
        }
    }

    // Resolve package via Nixhub
    let version_display = version.unwrap_or("latest");
    info(&format!(
        "Resolving {}@{} via Nixhub...",
        name, version_display
    ));

    let client = NixhubClient::new();
    let resolved = client.resolve_for_current_system(name, version.unwrap_or("latest"))?;

    info(&format!(
        "Found {} version {} (commit {})",
        resolved.name,
        resolved.version,
        &resolved.commit_hash[..8.min(resolved.commit_hash.len())]
    ));

    // Save original config for rollback BEFORE mutating
    let original_config = nixy_config.clone();

    // Add resolved package to profile
    {
        let profile = nixy_config
            .get_active_profile_mut()
            .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
        profile.add_resolved_package(ResolvedNixpkgPackage {
            name: resolved.name.clone(),
            version_spec: version.map(String::from),
            resolved_version: resolved.version.clone(),
            attribute_path: resolved.attribute_path.clone(),
            commit_hash: resolved.commit_hash.clone(),
            platforms: platforms.clone(),
        });
    }
    nixy_config.save(config)?;

    // Regenerate flake.nix
    let flake_dir = get_flake_dir(config)?;
    let global_packages_dir = if config.global_packages_dir.exists() {
        Some(config.global_packages_dir.as_path())
    } else {
        None
    };
    let profile_for_flake = nixy_config.get_active_profile().unwrap();
    if let Err(e) =
        regenerate_flake_from_profile(&flake_dir, profile_for_flake, global_packages_dir)
    {
        original_config.save(config)?;
        warn("Failed to regenerate flake.nix. Reverted changes.");
        return Err(e);
    }

    // Set up rollback context for Ctrl+C handling
    rollback::set_context(RollbackContext::nixy_config(
        flake_dir.clone(),
        config.nixy_json.clone(),
        original_config.clone(),
        global_packages_dir,
    ));

    info(&format!(
        "Installing {}@{}...",
        resolved.name, resolved.version
    ));
    if let Err(e) = super::sync::run(config) {
        // Clear rollback context since we're handling the error here
        rollback::clear_context();
        // Sync failed, revert config
        original_config.save(config)?;
        let original_profile = original_config.get_active_profile().unwrap();
        let _ = regenerate_flake_from_profile(&flake_dir, original_profile, global_packages_dir);
        warn("Sync failed. Reverted changes.");
        return Err(e);
    }

    // Clear rollback context on success
    rollback::clear_context();

    Ok(())
}

/// Install from a flake URL (e.g., github:user/repo)
fn install_from_flake_url(
    config: &Config,
    flake_url: &str,
    pkg: &str,
    platforms: Option<Vec<String>>,
) -> Result<()> {
    // Use NixyConfig if available (new format)
    if nixy_json_exists(config) {
        return install_from_flake_url_with_nixy_config(config, flake_url, pkg, platforms);
    }

    // Legacy format
    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);

    // Load state
    let mut state = PackageState::load(&state_path)?;

    // Check if package is already installed
    if state.has_package(pkg) {
        success(&format!("Package '{}' is already installed", pkg));
        return Ok(());
    }

    info(&format!("Using flake URL: {}", flake_url));
    let input_name = derive_input_name_from_url(flake_url);

    // Validate the package exists
    info(&format!(
        "Validating package '{}' in {}...",
        pkg, input_name
    ));
    let pkg_output = Nix::validate_flake_package(flake_url, pkg)?.ok_or_else(|| {
        let available = Nix::list_flake_packages(flake_url, None)
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

    // Add custom package to state
    state.add_custom_package(CustomPackage {
        name: pkg.to_string(),
        input_name: input_name.clone(),
        input_url: flake_url.to_string(),
        package_output: pkg_output,
        source_name: None,
        platforms,
    });
    state.save(&state_path)?;

    // Regenerate flake.nix (rollback state if this fails)
    if let Err(e) = regenerate_flake(&flake_dir, &state) {
        original_state.save(&state_path)?;
        warn("Failed to regenerate flake.nix. Reverted changes.");
        return Err(e);
    }

    // Set up rollback context for Ctrl+C handling
    rollback::set_context(RollbackContext::legacy(
        flake_dir.clone(),
        state_path.clone(),
        original_state.clone(),
    ));

    info(&format!("Installing {} from {}...", pkg, input_name));
    if let Err(e) = super::sync::run(config) {
        // Clear rollback context since we're handling the error here
        rollback::clear_context();
        // Sync failed, revert state and flake
        original_state.save(&state_path)?;
        let _ = regenerate_flake(&flake_dir, &original_state);
        warn("Sync failed. Reverted changes.");
        return Err(e);
    }

    // Clear rollback context on success
    rollback::clear_context();

    Ok(())
}

/// Install from a flake URL using the new nixy.json format
fn install_from_flake_url_with_nixy_config(
    config: &Config,
    flake_url: &str,
    pkg: &str,
    platforms: Option<Vec<String>>,
) -> Result<()> {
    let mut nixy_config = NixyConfig::load(config)?;
    let active_profile = nixy_config.active_profile.clone();

    // Check if package is already installed (scope the borrow)
    {
        let profile = nixy_config
            .get_active_profile()
            .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
        if profile.has_package(pkg) {
            success(&format!("Package '{}' is already installed", pkg));
            return Ok(());
        }
    }

    info(&format!("Using flake URL: {}", flake_url));
    let input_name = derive_input_name_from_url(flake_url);

    info(&format!(
        "Validating package '{}' in {}...",
        pkg, input_name
    ));
    let pkg_output = Nix::validate_flake_package(flake_url, pkg)?.ok_or_else(|| {
        let available = Nix::list_flake_packages(flake_url, None)
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

    // Save original config for rollback BEFORE mutating
    let original_config = nixy_config.clone();

    // Add custom package to profile
    {
        let profile = nixy_config
            .get_active_profile_mut()
            .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
        profile.add_custom_package(CustomPackage {
            name: pkg.to_string(),
            input_name: input_name.clone(),
            input_url: flake_url.to_string(),
            package_output: pkg_output,
            source_name: None,
            platforms,
        });
    }
    nixy_config.save(config)?;

    // Regenerate flake.nix
    let flake_dir = get_flake_dir(config)?;
    let global_packages_dir = if config.global_packages_dir.exists() {
        Some(config.global_packages_dir.as_path())
    } else {
        None
    };
    let profile_for_flake = nixy_config.get_active_profile().unwrap();
    if let Err(e) =
        regenerate_flake_from_profile(&flake_dir, profile_for_flake, global_packages_dir)
    {
        original_config.save(config)?;
        warn("Failed to regenerate flake.nix. Reverted changes.");
        return Err(e);
    }

    // Set up rollback context for Ctrl+C handling
    rollback::set_context(RollbackContext::nixy_config(
        flake_dir.clone(),
        config.nixy_json.clone(),
        original_config.clone(),
        global_packages_dir,
    ));

    info(&format!("Installing {} from {}...", pkg, input_name));
    if let Err(e) = super::sync::run(config) {
        // Clear rollback context since we're handling the error here
        rollback::clear_context();
        original_config.save(config)?;
        let original_profile = original_config.get_active_profile().unwrap();
        let _ = regenerate_flake_from_profile(&flake_dir, original_profile, global_packages_dir);
        warn("Sync failed. Reverted changes.");
        return Err(e);
    }

    // Clear rollback context on success
    rollback::clear_context();

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

/// Derive a package name from a flake URL (uses the last path component, e.g., repo name)
/// For "github:user/repo" → "repo", for "path:./foo/bar" → "bar"
fn derive_package_name_from_url(url: &str) -> String {
    // Strip the scheme (everything before and including ':')
    let path = url.split_once(':').map(|(_, p)| p).unwrap_or(url);
    // Take the last path component
    let name = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("default")
        .trim_end_matches(".git");
    if name.is_empty() {
        "default".to_string()
    } else {
        sanitize_input_name(name)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
    fn test_flake_reference_detection() {
        // Strings containing ':' should be detected as flake references
        assert!("github:user/repo".contains(':'));
        assert!("gitlab:user/repo".contains(':'));
        assert!("path:/some/path".contains(':'));
        assert!("git+https://example.com/repo".contains(':'));

        // Plain package names should NOT be detected
        assert!(!"hello".contains(':'));
        assert!(!"nodejs".contains(':'));
        assert!(!"ripgrep".contains(':'));
        // Version specs with @ should NOT be detected
        assert!(!"nodejs@20".contains(':'));
    }

    #[test]
    fn test_flake_reference_split() {
        // With fragment: should extract package name
        let spec = "github:user/repo#some-pkg";
        let (url, pkg) = if let Some((u, p)) = spec.split_once('#') {
            (u.to_string(), p.to_string())
        } else {
            (spec.to_string(), derive_package_name_from_url(spec))
        };
        assert_eq!(url, "github:user/repo");
        assert_eq!(pkg, "some-pkg");

        // Without fragment: should derive package name from URL
        let spec = "github:user/repo";
        let (url, pkg) = if let Some((u, p)) = spec.split_once('#') {
            (u.to_string(), p.to_string())
        } else {
            (spec.to_string(), derive_package_name_from_url(spec))
        };
        assert_eq!(url, "github:user/repo");
        assert_eq!(pkg, "repo");
    }

    #[test]
    fn test_derive_package_name_from_url() {
        assert_eq!(derive_package_name_from_url("github:user/repo"), "repo");
        assert_eq!(derive_package_name_from_url("github:user/repo.git"), "repo");
        assert_eq!(
            derive_package_name_from_url("gitlab:org/project"),
            "project"
        );
        assert_eq!(derive_package_name_from_url("path:./foo/bar"), "bar");
        assert_eq!(derive_package_name_from_url("path:./single"), "single");
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
