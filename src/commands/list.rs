use crate::config::Config;
use crate::error::{Error, Result};
use crate::nix::Nix;
use crate::profile::get_flake_path;

use super::info;

pub fn run(config: &Config) -> Result<()> {
    let flake_path = get_flake_path(config);

    if !flake_path.exists() {
        return Err(Error::NoFlakeFound(flake_path.display().to_string()));
    }

    info(&format!("Packages in {}:", flake_path.display()));

    let flake_dir = flake_path.parent().unwrap();
    let packages = Nix::eval_packages(flake_dir)?;

    if packages.is_empty() {
        println!("  (none)");
    } else {
        for pkg in packages {
            println!("  {}", pkg);
        }
    }

    Ok(())
}
