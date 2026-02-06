//! Rollback context for handling Ctrl+C interrupts.
//!
//! This module provides a mechanism to restore the original state of packages.json/nixy.json
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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::flake::template::{regenerate_flake, regenerate_flake_from_profile};
use crate::nixy_config::NixyConfig;
use crate::state::PackageState;

/// Flag indicating operation completed successfully (prevents late rollback)
static COMPLETED: AtomicBool = AtomicBool::new(false);

/// Global rollback context
static ROLLBACK_CONTEXT: Mutex<Option<RollbackContext>> = Mutex::new(None);

/// Original state to restore on rollback (legacy or new format)
#[derive(Clone)]
pub enum OriginalState {
    /// Legacy packages.json format
    Legacy {
        state_path: PathBuf,
        state: PackageState,
    },
    /// New nixy.json format
    NixyConfig {
        nixy_json_path: PathBuf,
        config: NixyConfig,
        global_packages_dir: Option<PathBuf>,
    },
}

/// Context needed to rollback changes on interrupt
#[derive(Clone)]
pub struct RollbackContext {
    pub flake_dir: PathBuf,
    pub original_state: OriginalState,
}

impl RollbackContext {
    /// Create a new legacy rollback context
    pub fn legacy(flake_dir: PathBuf, state_path: PathBuf, original_state: PackageState) -> Self {
        Self {
            flake_dir,
            original_state: OriginalState::Legacy {
                state_path,
                state: original_state,
            },
        }
    }

    /// Create a new nixy.json rollback context
    pub fn nixy_config(
        flake_dir: PathBuf,
        nixy_json_path: PathBuf,
        config: NixyConfig,
        global_packages_dir: Option<&Path>,
    ) -> Self {
        Self {
            flake_dir,
            original_state: OriginalState::NixyConfig {
                nixy_json_path,
                config,
                global_packages_dir: global_packages_dir.map(|p| p.to_path_buf()),
            },
        }
    }
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
    // First clear the rollback context so that once COMPLETED is true,
    // there is definitely no context left to roll back.
    if let Ok(mut guard) = ROLLBACK_CONTEXT.lock() {
        *guard = None;
    }
    // Mark operation as completed to prevent late rollback
    COMPLETED.store(true, Ordering::SeqCst);
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
    // Restore original state based on format
    match &ctx.original_state {
        OriginalState::Legacy { state_path, state } => {
            // Restore packages.json
            if let Err(e) = state.save(state_path) {
                eprintln!("Warning: Failed to restore packages.json: {}", e);
            }

            // Regenerate flake.nix from original state
            if let Err(e) = regenerate_flake(&ctx.flake_dir, state) {
                eprintln!("Warning: Failed to restore flake.nix: {}", e);
            }
        }
        OriginalState::NixyConfig {
            nixy_json_path,
            config,
            global_packages_dir,
        } => {
            // Restore nixy.json by writing directly (avoid circular dependency with Config)
            let content = match serde_json::to_string_pretty(config) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Warning: Failed to serialize nixy.json for rollback: {}", e);
                    eprintln!("Rollback incomplete.");
                    return;
                }
            };

            // Write via temp file for atomicity
            let tmp_path = nixy_json_path.with_extension("json.tmp");
            if let Err(e) = std::fs::write(&tmp_path, &content) {
                eprintln!("Warning: Failed to write nixy.json temp file: {}", e);
            } else if let Err(e) = std::fs::rename(&tmp_path, nixy_json_path) {
                eprintln!("Warning: Failed to restore nixy.json: {}", e);
                let _ = std::fs::remove_file(&tmp_path);
            }

            // Regenerate flake.nix from original config
            if let Some(profile) = config.get_active_profile() {
                let gpd = global_packages_dir.as_deref();
                if let Err(e) = regenerate_flake_from_profile(&ctx.flake_dir, profile, gpd) {
                    eprintln!("Warning: Failed to restore flake.nix: {}", e);
                }
            }
        }
    }

    eprintln!("Rollback complete.");
}
