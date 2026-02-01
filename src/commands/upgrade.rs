use std::fs;

use crate::cli::UpgradeArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::flake::template::regenerate_flake;
use crate::nix::Nix;
use crate::nixhub::NixhubClient;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState, ResolvedNixpkgPackage};

use super::{info, success, warn};

pub fn run(config: &Config, args: UpgradeArgs) -> Result<()> {
    let inputs = args.inputs;
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
            let mut invalid = Vec::new();
            let mut legacy_packages = Vec::new();

            for input in &flake_inputs_to_update {
                if !available.contains(*input) {
                    // Check if this is a legacy or custom package name
                    if state.packages.contains(*input)
                        || state.custom_packages.iter().any(|p| &p.name == *input)
                    {
                        legacy_packages.push((*input).clone());
                    } else {
                        invalid.push((*input).clone());
                    }
                }
            }

            // Provide clearer error for legacy packages
            if !legacy_packages.is_empty() {
                warn(
                    "Per-package upgrade is only supported for versioned packages (installed with @version).",
                );
                warn(&format!(
                    "Legacy packages ({}) are upgraded when you run 'nixy upgrade' without arguments.",
                    legacy_packages.join(", ")
                ));
                return Ok(());
            }

            if !invalid.is_empty() {
                return Err(Error::InvalidFlakeInputs(
                    invalid.join(", "),
                    available.join(" "),
                ));
            }

            let inputs_to_update: Vec<String> =
                flake_inputs_to_update.into_iter().cloned().collect();
            info(&format!(
                "Updating inputs: {}...",
                inputs_to_update.join(", ")
            ));
            Nix::flake_update(&flake_dir, &inputs_to_update)?;
        }
    } else {
        // No arguments: upgrade all resolved packages
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
        success(&format!("Upgraded: {}", inputs.join(", ")));
    } else {
        success("All packages upgraded");
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
