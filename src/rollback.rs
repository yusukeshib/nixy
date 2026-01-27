//! Rollback context for handling Ctrl+C interrupts.
//!
//! This module provides a mechanism to restore the original state of packages.json
//! and flake.nix when the user interrupts an operation with Ctrl+C.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::flake::template::regenerate_flake;
use crate::state::PackageState;

/// Global flag indicating if Ctrl+C was pressed
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Global rollback context
static ROLLBACK_CONTEXT: Mutex<Option<RollbackContext>> = Mutex::new(None);

/// Context needed to rollback changes on interrupt
#[derive(Clone)]
pub struct RollbackContext {
    pub flake_dir: PathBuf,
    pub state_path: PathBuf,
    pub original_state: PackageState,
    /// Optional path to a file that was copied (for --file installs)
    pub copied_file: Option<PathBuf>,
    /// Optional path to a directory that was created (for local flake installs)
    pub created_dir: Option<PathBuf>,
}

/// Initialize the Ctrl+C handler. Should be called once at startup.
pub fn init_signal_handler() {
    let _ = ctrlc::set_handler(move || {
        INTERRUPTED.store(true, Ordering::SeqCst);

        // Attempt rollback
        if let Some(ctx) = take_context() {
            eprintln!("\nInterrupted. Rolling back changes...");
            perform_rollback(&ctx);
        }

        std::process::exit(130); // 128 + SIGINT (2)
    });
}

/// Set the rollback context before a potentially long operation
pub fn set_context(ctx: RollbackContext) {
    if let Ok(mut guard) = ROLLBACK_CONTEXT.lock() {
        *guard = Some(ctx);
    }
}

/// Clear the rollback context (call after successful operation)
pub fn clear_context() {
    if let Ok(mut guard) = ROLLBACK_CONTEXT.lock() {
        *guard = None;
    }
}

/// Take the rollback context (for use in signal handler)
fn take_context() -> Option<RollbackContext> {
    ROLLBACK_CONTEXT
        .lock()
        .ok()
        .and_then(|mut guard| guard.take())
}

/// Perform the actual rollback
fn perform_rollback(ctx: &RollbackContext) {
    // Restore original state
    if let Err(e) = ctx.original_state.save(&ctx.state_path) {
        eprintln!("Warning: Failed to restore packages.json: {}", e);
    }

    // Remove copied file if any
    if let Some(ref path) = ctx.copied_file {
        let _ = std::fs::remove_file(path);
    }

    // Remove created directory if any
    if let Some(ref path) = ctx.created_dir {
        let _ = std::fs::remove_dir_all(path);
    }

    // Regenerate flake.nix from original state
    if let Err(e) = regenerate_flake(&ctx.flake_dir, &ctx.original_state) {
        eprintln!("Warning: Failed to restore flake.nix: {}", e);
    }

    eprintln!("Rollback complete.");
}
