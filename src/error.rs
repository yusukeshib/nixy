//! Error types for nixy.
//!
//! This module defines all error types used throughout the application.
//! Errors are categorized into domain-specific variants (e.g., `PackageNotFound`,
//! `ProfileNotFound`) and generic variants (e.g., `Io`, `Regex`).
//!
//! The `Usage` variant is special - it's used for user-facing error messages
//! that don't need the "Error:" prefix.

use thiserror::Error;

/// All possible errors that can occur in nixy
#[derive(Error, Debug)]
pub enum Error {
    #[error("Package '{0}' not found in nixpkgs or is not a valid derivation")]
    PackageNotFound(String),

    #[error("Profile '{0}' does not exist")]
    ProfileNotFound(String),

    #[error("Cannot delete the active profile. Switch to another profile first.")]
    CannotDeleteActiveProfile,

    #[error("Invalid profile name '{0}'. Use only letters, numbers, dashes, and underscores.")]
    InvalidProfileName(String),

    #[error("Nix command failed: {0}")]
    NixCommand(String),

    #[error("Nix is not installed")]
    NixNotInstalled,

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Could not find 'name' or 'pname' attribute in {0}")]
    NoPackageName(String),

    #[error("Could not determine package name from filename: {0}")]
    InvalidFilename(String),

    #[error("Registry entry '{0}' not found. Use 'nix registry list' to see available entries.")]
    RegistryNotFound(String),

    #[error("Package '{0}' not found in '{1}'")]
    FlakePackageNotFound(String, String),

    #[error("Unknown input(s): {0}. Available inputs: {1}")]
    InvalidFlakeInputs(String, String),

    #[error("No flake.lock found. Run 'nixy sync' first.")]
    NoFlakeLock,

    #[error("Failed to parse flake.lock. The file may be corrupted.")]
    InvalidFlakeLock,

    #[error("Unknown shell: {0}. Supported: bash, zsh, fish")]
    UnknownShell(String),

    #[error("{0}")]
    Usage(String),

    #[error("Self-update error: {0}")]
    SelfUpdate(String),

    #[error("State file error: {0}")]
    StateFile(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Interactive prompt error: {0}")]
    Dialoguer(#[from] dialoguer::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
