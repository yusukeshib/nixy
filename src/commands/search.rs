use crate::error::Result;
use crate::nix::Nix;

use super::info;

pub fn run(query: &str) -> Result<()> {
    info(&format!("Searching for {}...", query));
    Nix::search(query)?;
    Ok(())
}
