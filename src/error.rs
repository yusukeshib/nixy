use thiserror::Error;

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

    #[error("Existing flake.nix is not managed by nixy")]
    NotNixyManaged,

    #[error("No flake.nix found at {0}. Run 'nixy install <package>' to create one.")]
    NoFlakeFound(String),

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

    #[error("flake.nix has modifications outside nixy markers. Use --force to proceed.")]
    CustomModifications,

    #[error("Unknown shell: {0}. Supported: bash, zsh, fish")]
    UnknownShell(String),

    #[error("{0}")]
    Usage(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
