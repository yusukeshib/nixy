use std::collections::HashSet;

use crate::config::Config;
use crate::error::Result;
use crate::flake::parser::collect_local_packages;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

use super::info;

/// Package entry with source information
struct PackageEntry {
    name: String,
    source: PackageSource,
    /// Platform restrictions (None means all platforms)
    platforms: Option<Vec<String>>,
}

/// Source of a package
enum PackageSource {
    /// Standard nixpkgs package (legacy, no version info)
    Nixpkgs,
    /// Resolved nixpkgs package with version
    NixpkgsVersioned { version: String },
    /// Custom package from an external flake
    Custom { url: String },
    /// Local package from packages/ directory
    Local,
}

impl PackageSource {
    fn display(&self) -> String {
        match self {
            PackageSource::Nixpkgs | PackageSource::NixpkgsVersioned { .. } => {
                "nixpkgs".to_string()
            }
            PackageSource::Custom { url } => url.clone(),
            PackageSource::Local => "local".to_string(),
        }
    }
}

/// Format package name with version if available
fn format_package_name(entry: &PackageEntry) -> String {
    match &entry.source {
        PackageSource::NixpkgsVersioned { version } => {
            format!("{}@{}", entry.name, version)
        }
        _ => entry.name.clone(),
    }
}

/// Format platform restrictions for display
fn format_platforms(platforms: &Option<Vec<String>>) -> String {
    match platforms {
        None => String::new(),
        Some(p) if p.is_empty() => String::new(),
        Some(p) => {
            // Simplify platform display: if all darwin or all linux, show the alias
            let all_darwin = p.iter().all(|s| s.ends_with("-darwin"));
            let all_linux = p.iter().all(|s| s.ends_with("-linux"));
            if all_darwin && p.len() == 2 {
                " [darwin]".to_string()
            } else if all_linux && p.len() == 2 {
                " [linux]".to_string()
            } else {
                format!(" [{}]", p.join(", "))
            }
        }
    }
}

pub fn run(config: &Config) -> Result<()> {
    info("Installed packages:");

    // Get the flake directory
    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);

    // Load state from packages.json
    let state = PackageState::load(&state_path)?;

    // Collect all packages with their sources
    let mut entries: Vec<PackageEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Add legacy nixpkgs packages (no version info)
    for name in &state.packages {
        entries.push(PackageEntry {
            name: name.clone(),
            source: PackageSource::Nixpkgs,
            platforms: None,
        });
        seen.insert(name.clone());
    }

    // Add resolved nixpkgs packages (with version info)
    for pkg in &state.resolved_packages {
        entries.push(PackageEntry {
            name: pkg.name.clone(),
            source: PackageSource::NixpkgsVersioned {
                version: pkg.resolved_version.clone(),
            },
            platforms: pkg.platforms.clone(),
        });
        seen.insert(pkg.name.clone());
    }

    // Add custom packages
    for pkg in &state.custom_packages {
        entries.push(PackageEntry {
            name: pkg.name.clone(),
            source: PackageSource::Custom {
                url: pkg.input_url.clone(),
            },
            platforms: pkg.platforms.clone(),
        });
        seen.insert(pkg.name.clone());
    }

    // Add local packages from packages/ directory
    let packages_dir = flake_dir.join("packages");
    if packages_dir.exists() {
        let (local_packages, local_flakes) = collect_local_packages(&packages_dir);
        for pkg in local_packages {
            if !seen.contains(&pkg.name) {
                entries.push(PackageEntry {
                    name: pkg.name.clone(),
                    source: PackageSource::Local,
                    platforms: None,
                });
                seen.insert(pkg.name);
            }
        }
        for flake in local_flakes {
            if !seen.contains(&flake.name) {
                entries.push(PackageEntry {
                    name: flake.name.clone(),
                    source: PackageSource::Local,
                    platforms: None,
                });
                seen.insert(flake.name);
            }
        }
    }

    // Sort by name
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    if entries.is_empty() {
        println!("  (none)");
    } else {
        // Calculate column width for alignment (using formatted name with version)
        let max_name_len = entries
            .iter()
            .map(|e| format_package_name(e).len())
            .max()
            .unwrap_or(0);

        for entry in entries {
            let formatted_name = format_package_name(&entry);
            let platform_str = format_platforms(&entry.platforms);
            println!(
                "  {:<width$}  ({}){}",
                formatted_name,
                entry.source.display(),
                platform_str,
                width = max_name_len
            );
        }
    }

    Ok(())
}
