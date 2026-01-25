use crate::error::Result;
use crate::nix::Nix;

use super::{info, success};

pub fn run() -> Result<()> {
    info("Running garbage collection...");
    Nix::gc()?;
    success("Garbage collection complete");
    Ok(())
}
