use std::fs;

use crate::cli::UpdateArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::{regenerate_flake, regenerate_flake_from_profile};
use crate::nix::Nix;
use crate::nixhub::NixhubClient;
use crate::nixy_config::{nixy_json_exists, NixyConfig, ProfileConfig};
use crate::profile::get_flake_dir;
use crate::rollback::{self, RollbackContext};
use crate::state::{get_state_path, CustomPackage, PackageState, ResolvedNixpkgPackage};

use super::{info, success, warn};

pub fn run(config: &Config, args: UpdateArgs) -> Result<()> {
    let inputs = args.inputs;

    // Require either specific targets or --all to update everything
    if inputs.is_empty() && !args.all {
        return Err(Error::Usage(
            "Specify packages/inputs to update, or pass --all to update everything.\n\nExamples:\n  nixy update <package>\n  nixy update --all"
                .to_string(),
        ));
    }

    // Use NixyConfig if available (new format)
    if nixy_json_exists(config) {
        return upgrade_with_nixy_config(config, inputs);
    }

    // Legacy format
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");
    let state_path = get_state_path(&flake_dir);
    let lock_file = flake_dir.join("flake.lock");

    // Load state
    let mut state = PackageState::load(&state_path)?;

    // Auto-regenerate flake.nix if missing
    if !flake_path.exists() {
        info("Regenerating flake.nix from packages.json...");
        regenerate_flake(&flake_dir, &state)?;
    }

    if !inputs.is_empty() {
        // Check if inputs are package names or flake input names
        let resolved_names: Vec<&str> = state
            .resolved_packages
            .iter()
            .map(|p| p.name.as_str())
            .collect();

        let (packages_to_upgrade, flake_inputs_to_update): (Vec<&String>, Vec<&String>) = inputs
            .iter()
            .partition(|input| resolved_names.contains(&input.as_str()));

        // Upgrade resolved packages
        if !packages_to_upgrade.is_empty() {
            upgrade_resolved_packages(&mut state, &packages_to_upgrade)?;
            state.save(&state_path)?;
            regenerate_flake(&flake_dir, &state)?;
        }

        // Update flake inputs (for legacy packages or explicit input names)
        if !flake_inputs_to_update.is_empty() {
            if !lock_file.exists() {
                return Err(Error::NoFlakeLock);
            }

            let available = Nix::get_flake_inputs(&lock_file)?;
            let classified = classify_update_targets(
                &flake_inputs_to_update,
                &available,
                &state.packages,
                &state.custom_packages,
            );

            // Genuine legacy packages cannot be upgraded individually
            if !classified.legacy.is_empty() {
                warn_legacy_packages(&classified.legacy);
                return Ok(());
            }

            if !classified.invalid.is_empty() {
                return Err(Error::InvalidFlakeInputs(
                    classified.invalid.join(", "),
                    available.join(" "),
                ));
            }

            info(&format!(
                "Updating inputs: {}...",
                classified.inputs_to_update.join(", ")
            ));
            Nix::flake_update(&flake_dir, &classified.inputs_to_update)?;
        }
    } else {
        // --all: upgrade all resolved packages
        if !state.resolved_packages.is_empty() {
            let all_names: Vec<String> = state
                .resolved_packages
                .iter()
                .map(|p| p.name.clone())
                .collect();
            let all_refs: Vec<&String> = all_names.iter().collect();
            upgrade_resolved_packages(&mut state, &all_refs)?;
            state.save(&state_path)?;
            regenerate_flake(&flake_dir, &state)?;
        }

        // Also update all flake inputs (for legacy packages)
        info("Updating all flake inputs...");
        Nix::flake_update_all(&flake_dir)?;
    }

    info("Rebuilding environment...");

    // Ensure parent directory exists
    if let Some(parent) = config.env_link.parent() {
        fs::create_dir_all(parent)?;
    }

    Nix::build(&flake_dir, "default", &config.env_link)?;

    if !inputs.is_empty() {
        success(&format!("Updated: {}", inputs.join(", ")));
    } else {
        success("All packages updated");
    }

    Ok(())
}

/// Upgrade packages using the new nixy.json format
fn upgrade_with_nixy_config(config: &Config, inputs: Vec<String>) -> Result<()> {
    let mut nixy_config = NixyConfig::load(config)?;
    let active_profile = nixy_config.active_profile.clone();
    let flake_dir = get_flake_dir(config)?;
    let flake_path = flake_dir.join("flake.nix");
    let lock_file = flake_dir.join("flake.lock");

    // Save original config for rollback BEFORE any mutations
    let original_config = nixy_config.clone();

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

    let global_packages_dir = if config.global_packages_dir.exists() {
        Some(config.global_packages_dir.as_path())
    } else {
        None
    };

    // Track whether we modified the config (need rollback support)
    let mut config_modified = false;

    if !inputs.is_empty() {
        // Get resolved package names (scope the borrow)
        let resolved_names: Vec<String> = {
            let profile = nixy_config
                .get_active_profile()
                .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
            profile
                .resolved_packages
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };

        let (packages_to_upgrade, flake_inputs_to_update): (Vec<&String>, Vec<&String>) = inputs
            .iter()
            .partition(|input| resolved_names.iter().any(|n| n == *input));

        // Upgrade resolved packages
        if !packages_to_upgrade.is_empty() {
            {
                let profile = nixy_config
                    .get_active_profile_mut()
                    .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
                upgrade_resolved_packages_in_profile(profile, &packages_to_upgrade)?;
            }
            nixy_config.save(config)?;
            config_modified = true;
            let profile_for_flake = nixy_config.get_active_profile().unwrap();
            regenerate_flake_from_profile(&flake_dir, profile_for_flake, global_packages_dir)?;
        }

        // Update flake inputs
        if !flake_inputs_to_update.is_empty() {
            if !lock_file.exists() {
                return Err(Error::NoFlakeLock);
            }

            let available = Nix::get_flake_inputs(&lock_file)?;
            let classified = {
                let profile = nixy_config
                    .get_active_profile()
                    .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
                classify_update_targets(
                    &flake_inputs_to_update,
                    &available,
                    &profile.packages,
                    &profile.custom_packages,
                )
            };

            if !classified.legacy.is_empty() {
                warn_legacy_packages(&classified.legacy);
                return Ok(());
            }

            if !classified.invalid.is_empty() {
                return Err(Error::InvalidFlakeInputs(
                    classified.invalid.join(", "),
                    available.join(" "),
                ));
            }

            info(&format!(
                "Updating inputs: {}...",
                classified.inputs_to_update.join(", ")
            ));
            Nix::flake_update(&flake_dir, &classified.inputs_to_update)?;
        }
    } else {
        // --all: upgrade all resolved packages
        // Get package names first (scope the borrow)
        let all_names: Vec<String> = {
            let profile = nixy_config
                .get_active_profile()
                .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
            profile
                .resolved_packages
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };

        if !all_names.is_empty() {
            let all_refs: Vec<&String> = all_names.iter().collect();
            {
                let profile = nixy_config
                    .get_active_profile_mut()
                    .ok_or_else(|| Error::ProfileNotFound(active_profile.clone()))?;
                upgrade_resolved_packages_in_profile(profile, &all_refs)?;
            }
            nixy_config.save(config)?;
            config_modified = true;
            let profile_for_flake = nixy_config.get_active_profile().unwrap();
            regenerate_flake_from_profile(&flake_dir, profile_for_flake, global_packages_dir)?;
        }

        info("Updating all flake inputs...");
        Nix::flake_update_all(&flake_dir)?;
    }

    // Set up rollback context for Ctrl+C handling if we modified the config
    if config_modified {
        rollback::set_context(RollbackContext::nixy_config(
            flake_dir.clone(),
            config.nixy_json.clone(),
            original_config.clone(),
            global_packages_dir,
        ));
    }

    info("Rebuilding environment...");

    if let Some(parent) = config.env_link.parent() {
        fs::create_dir_all(parent)?;
    }

    if let Err(e) = Nix::build(&flake_dir, "default", &config.env_link) {
        // Clear rollback context since we're handling the error here
        rollback::clear_context();
        // Build failed, revert config if we modified it
        if config_modified {
            original_config.save(config)?;
            let original_profile = original_config.get_active_profile().unwrap();
            let _ =
                regenerate_flake_from_profile(&flake_dir, original_profile, global_packages_dir);
            warn("Build failed. Reverted nixy.json and flake.nix.");
        }
        return Err(e);
    }

    // Clear rollback context on success
    rollback::clear_context();

    if !inputs.is_empty() {
        success(&format!("Updated: {}", inputs.join(", ")));
    } else {
        success("All packages updated");
    }

    Ok(())
}

/// Upgrade resolved packages by re-resolving them via Nixhub
fn upgrade_resolved_packages(state: &mut PackageState, package_names: &[&String]) -> Result<()> {
    let client = NixhubClient::new();

    for name in package_names {
        if let Some(existing) = state.resolved_packages.iter().find(|p| &p.name == *name) {
            // Determine version to resolve
            let version = existing.version_spec.as_deref().unwrap_or("latest");
            info(&format!("Resolving {}@{}...", name, version));

            match client.resolve_for_current_system(name, version) {
                Ok(resolved) => {
                    if resolved.version != existing.resolved_version
                        || resolved.commit_hash != existing.commit_hash
                    {
                        info(&format!(
                            "  {} -> {} (commit {})",
                            existing.resolved_version,
                            resolved.version,
                            &resolved.commit_hash[..8.min(resolved.commit_hash.len())]
                        ));

                        // Update the package, preserving platform restrictions
                        state.add_resolved_package(ResolvedNixpkgPackage {
                            name: resolved.name,
                            version_spec: existing.version_spec.clone(),
                            resolved_version: resolved.version,
                            attribute_path: resolved.attribute_path,
                            commit_hash: resolved.commit_hash,
                            platforms: existing.platforms.clone(),
                        });
                    } else {
                        info(&format!("  {} is already at the latest version", name));
                    }
                }
                Err(e) => {
                    warn(&format!("  Failed to resolve {}: {}", name, e));
                }
            }
        }
    }

    Ok(())
}

/// Upgrade resolved packages in a ProfileConfig
fn upgrade_resolved_packages_in_profile(
    profile: &mut ProfileConfig,
    package_names: &[&String],
) -> Result<()> {
    let client = NixhubClient::new();

    for name in package_names {
        if let Some(existing) = profile.resolved_packages.iter().find(|p| &p.name == *name) {
            let version = existing.version_spec.as_deref().unwrap_or("latest");
            info(&format!("Resolving {}@{}...", name, version));

            match client.resolve_for_current_system(name, version) {
                Ok(resolved) => {
                    if resolved.version != existing.resolved_version
                        || resolved.commit_hash != existing.commit_hash
                    {
                        info(&format!(
                            "  {} -> {} (commit {})",
                            existing.resolved_version,
                            resolved.version,
                            &resolved.commit_hash[..8.min(resolved.commit_hash.len())]
                        ));

                        profile.add_resolved_package(ResolvedNixpkgPackage {
                            name: resolved.name,
                            version_spec: existing.version_spec.clone(),
                            resolved_version: resolved.version,
                            attribute_path: resolved.attribute_path,
                            commit_hash: resolved.commit_hash,
                            platforms: existing.platforms.clone(),
                        });
                    } else {
                        info(&format!("  {} is already at the latest version", name));
                    }
                }
                Err(e) => {
                    warn(&format!("  Failed to resolve {}: {}", name, e));
                }
            }
        }
    }

    Ok(())
}

/// Result of classifying user-supplied `nixy update` targets.
struct ClassifiedTargets {
    /// Real flake input names to pass to `nix flake update`.
    inputs_to_update: Vec<String>,
    /// Genuine unversioned legacy (v1) packages that cannot be updated alone.
    legacy: Vec<String>,
    /// Targets that match no flake input, custom package, or legacy package.
    invalid: Vec<String>,
}

/// Classify update targets (which may be flake input names OR package names)
/// into the actual flake inputs to update, genuine legacy packages, and
/// invalid targets.
///
/// A custom flake package is referenced by its package `name` (e.g. `pi-nix`)
/// but its flake input is named differently (e.g. `github-lukasl-dev-pi-nix`).
/// We map those names to their `input_name` so they can be updated individually.
fn classify_update_targets(
    targets: &[&String],
    available_inputs: &[String],
    legacy_packages: &[String],
    custom_packages: &[CustomPackage],
) -> ClassifiedTargets {
    let mut inputs_to_update = Vec::new();
    let mut legacy = Vec::new();
    let mut invalid = Vec::new();

    for target in targets {
        if available_inputs.contains(*target) {
            // Already a real flake input name.
            inputs_to_update.push((*target).clone());
        } else if let Some(pkg) = custom_packages.iter().find(|p| &p.name == *target) {
            // Custom flake package referenced by package name: map it to its
            // dedicated flake input so it can be updated individually.
            if available_inputs.contains(&pkg.input_name) {
                inputs_to_update.push(pkg.input_name.clone());
            } else {
                invalid.push((*target).clone());
            }
        } else if legacy_packages.contains(*target) {
            // Genuine unversioned legacy (v1) package: shares the default nixpkgs
            // input, so it has no dedicated input to update on its own.
            legacy.push((*target).clone());
        } else {
            invalid.push((*target).clone());
        }
    }

    ClassifiedTargets {
        inputs_to_update,
        legacy,
        invalid,
    }
}

/// Emit the warning shown when genuine legacy packages are targeted individually.
fn warn_legacy_packages(legacy: &[String]) {
    warn("Per-package upgrade is not supported for unversioned legacy packages (installed without @version).");
    warn(&format!(
        "These packages ({}) share the default nixpkgs input and are upgraded when you run 'nixy update --all'.",
        legacy.join(", ")
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(name: &str, input_name: &str) -> CustomPackage {
        CustomPackage {
            name: name.to_string(),
            input_name: input_name.to_string(),
            input_url: format!("github:owner/{}", name),
            package_output: "packages".to_string(),
            source_name: None,
            platforms: None,
        }
    }

    #[test]
    fn custom_package_name_maps_to_its_flake_input() {
        // Regression: `nixy update pi-nix` must map the package name to its
        // flake input (github-lukasl-dev-pi-nix) instead of being mislabeled
        // as a legacy package.
        let available = vec![
            "nixpkgs".to_string(),
            "github-lukasl-dev-pi-nix".to_string(),
        ];
        let custom_packages = vec![custom("pi-nix", "github-lukasl-dev-pi-nix")];
        let pi_nix = "pi-nix".to_string();
        let targets = vec![&pi_nix];

        let result = classify_update_targets(&targets, &available, &[], &custom_packages);

        assert_eq!(result.inputs_to_update, vec!["github-lukasl-dev-pi-nix"]);
        assert!(result.legacy.is_empty());
        assert!(result.invalid.is_empty());
    }

    #[test]
    fn direct_flake_input_name_is_passed_through() {
        let available = vec!["nixpkgs".to_string()];
        let nixpkgs = "nixpkgs".to_string();
        let targets = vec![&nixpkgs];

        let result = classify_update_targets(&targets, &available, &[], &[]);

        assert_eq!(result.inputs_to_update, vec!["nixpkgs"]);
        assert!(result.legacy.is_empty());
        assert!(result.invalid.is_empty());
    }

    #[test]
    fn genuine_legacy_package_is_reported_as_legacy() {
        let available = vec!["nixpkgs".to_string()];
        let legacy_packages = vec!["hello".to_string()];
        let hello = "hello".to_string();
        let targets = vec![&hello];

        let result = classify_update_targets(&targets, &available, &legacy_packages, &[]);

        assert!(result.inputs_to_update.is_empty());
        assert_eq!(result.legacy, vec!["hello"]);
        assert!(result.invalid.is_empty());
    }

    #[test]
    fn custom_package_without_locked_input_is_invalid() {
        // Custom package exists but its input is not yet in flake.lock.
        let available = vec!["nixpkgs".to_string()];
        let custom_packages = vec![custom("pi-nix", "github-lukasl-dev-pi-nix")];
        let pi_nix = "pi-nix".to_string();
        let targets = vec![&pi_nix];

        let result = classify_update_targets(&targets, &available, &[], &custom_packages);

        assert!(result.inputs_to_update.is_empty());
        assert!(result.legacy.is_empty());
        assert_eq!(result.invalid, vec!["pi-nix"]);
    }

    #[test]
    fn unknown_target_is_invalid() {
        let available = vec!["nixpkgs".to_string()];
        let bogus = "does-not-exist".to_string();
        let targets = vec![&bogus];

        let result = classify_update_targets(&targets, &available, &[], &[]);

        assert!(result.inputs_to_update.is_empty());
        assert!(result.legacy.is_empty());
        assert_eq!(result.invalid, vec!["does-not-exist"]);
    }
}
