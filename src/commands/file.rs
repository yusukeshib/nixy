//! Show path to package source file in Nix store.

use crate::cli::FileArgs;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::nix::Nix;
use crate::profile::get_flake_dir;
use crate::state::{get_state_path, PackageState};

/// Run the file command to show the source path for a package
pub fn run(config: &Config, args: FileArgs) -> Result<()> {
    let flake_dir = get_flake_dir(config)?;
    let state_path = get_state_path(&flake_dir);
    let state = PackageState::load(&state_path)?;

    let path = if let Some(custom) = state
        .custom_packages
        .iter()
        .find(|p| p.name == args.package)
    {
        // Custom package: prefetch the flake and return flake.nix path
        let store_path = Nix::flake_prefetch(&custom.input_url)?;
        store_path.join("flake.nix")
    } else if let Some(resolved) = state
        .resolved_packages
        .iter()
        .find(|p| p.name == args.package)
    {
        // Resolved nixpkgs package: get source path via meta.position
        let system = Nix::current_system()?;
        Nix::get_package_source_path(&resolved.commit_hash, &resolved.attribute_path, &system)?
    } else if state.packages.contains(&args.package) {
        // Legacy nixpkgs package: use nixos-unstable
        let system = Nix::current_system()?;
        Nix::get_package_source_path("nixos-unstable", &args.package, &system)?
    } else {
        // Check for local packages in the packages/ directory
        let local_nix = flake_dir
            .join("packages")
            .join(format!("{}.nix", args.package));
        let local_flake = flake_dir
            .join("packages")
            .join(&args.package)
            .join("flake.nix");

        if local_nix.is_file() {
            local_nix
        } else if local_flake.is_file() {
            local_flake
        } else {
            return Err(Error::PackageNotInstalled(args.package));
        }
    };

    println!("{}", path.display());
    Ok(())
}
