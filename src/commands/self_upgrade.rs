use crate::config::VERSION;
use crate::error::Result;

use super::info;

pub fn run(_force: bool) -> Result<()> {
    info(&format!("Current version: {}", VERSION));
    info("Self-upgrade is not available for the Rust version.");
    info("Please update using your package manager or by rebuilding from source.");
    Ok(())
}
