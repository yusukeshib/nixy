use crate::config::Config;
use crate::error::Result;
use crate::flake::parser::collect_local_packages;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

use super::info;

pub fn run(config: &Config) -> Result<()> {
    info("Installed packages:");

    // Get the flake directory
    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);

    // Load state from packages.json
    let state = PackageState::load(&state_path)?;

    // Get all packages from state
    let mut packages = state.all_package_names();

    // Also include local packages from packages/ directory
    let packages_dir = flake_dir.join("packages");
    if packages_dir.exists() {
        let (local_packages, local_flakes) = collect_local_packages(&packages_dir);
        for pkg in local_packages {
            if !packages.contains(&pkg.name) {
                packages.push(pkg.name);
            }
        }
        for flake in local_flakes {
            if !packages.contains(&flake.name) {
                packages.push(flake.name);
            }
        }
    }

    packages.sort();

    if packages.is_empty() {
        println!("  (none)");
    } else {
        for pkg in packages {
            println!("  {}", pkg);
        }
    }

    Ok(())
}
