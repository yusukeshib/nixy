//! Rollback context for handling Ctrl+C interrupts.
//!
//! This module provides a mechanism to restore the original state of packages.json
//! and flake.nix when the user interrupts an operation with Ctrl+C.
//!
//! # Thread Safety
//!
//! The rollback context uses a Mutex which may become poisoned if a panic occurs
//! while holding the lock. In such cases, `set_context` and `clear_context` will
//! silently fail, which is acceptable since a panic indicates a more serious issue.
//! The signal handler uses `take_context` which also handles poisoned mutexes gracefully.
//!
//! # Race Conditions
//!
//! An atomic flag `COMPLETED` is used to prevent rollback from occurring if Ctrl+C
//! is pressed after the operation has successfully completed but before the context
//! is cleared.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::flake::template::regenerate_flake;
use crate::state::PackageState;

/// Flag indicating operation completed successfully (prevents late rollback)
static COMPLETED: AtomicBool = AtomicBool::new(false);

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
    if let Err(e) = ctrlc::set_handler(move || {
        // Check if operation already completed successfully
        if COMPLETED.load(Ordering::SeqCst) {
            std::process::exit(130);
        }

        // Attempt rollback
        if let Some(ctx) = take_context() {
            eprintln!("\nInterrupted. Rolling back changes...");
            perform_rollback(&ctx);
        }

        std::process::exit(130); // 128 + SIGINT (2)
    }) {
        eprintln!(
            "Warning: Failed to set Ctrl+C handler: {}. Rollback on interrupt will be unavailable.",
            e
        );
    }
}

/// Set the rollback context before a potentially long operation.
///
/// Note: Silently fails if the mutex is poisoned (which indicates a prior panic).
pub fn set_context(ctx: RollbackContext) {
    COMPLETED.store(false, Ordering::SeqCst);
    if let Ok(mut guard) = ROLLBACK_CONTEXT.lock() {
        *guard = Some(ctx);
    }
}

/// Clear the rollback context (call after successful operation).
///
/// Note: Silently fails if the mutex is poisoned (which indicates a prior panic).
pub fn clear_context() {
    // Mark operation as completed to prevent late rollback
    COMPLETED.store(true, Ordering::SeqCst);
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
