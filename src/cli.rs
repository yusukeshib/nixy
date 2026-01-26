use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "nixy",
    about = "Homebrew-style wrapper for Nix using flake.nix"
)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install a package from nixpkgs
    #[command(alias = "add")]
    Install(InstallArgs),

    /// Uninstall a package
    #[command(alias = "remove")]
    Uninstall(UninstallArgs),

    /// List packages in flake.nix
    #[command(alias = "ls")]
    List,

    /// Search for packages
    Search {
        /// Search query
        query: String,
    },

    /// Upgrade all inputs or specific ones
    Upgrade(UpgradeArgs),

    /// Build environment from flake.nix and create symlink
    Sync(SyncArgs),

    /// Output shell config (for eval in rc files)
    Config {
        /// Shell type (bash, zsh, fish)
        shell: String,
    },

    /// Profile management commands
    Profile(ProfileArgs),

    /// Upgrade nixy to the latest version
    SelfUpgrade(SelfUpgradeArgs),

    /// Show nixy version
    Version,
}

#[derive(Args)]
pub struct InstallArgs {
    /// Package name to install
    pub package: Option<String>,

    /// Install from a flake (registry name or URL)
    #[arg(long)]
    pub from: Option<String>,

    /// Install from local nix file
    #[arg(long, short)]
    pub file: Option<PathBuf>,
}

#[derive(Args)]
pub struct UpgradeArgs {
    /// Specific inputs to upgrade (if empty, upgrades all)
    pub inputs: Vec<String>,
}

#[derive(Args)]
pub struct SyncArgs {}

#[derive(Args)]
pub struct UninstallArgs {
    /// Package name to uninstall
    pub package: String,
}

#[derive(Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: Option<ProfileCommands>,
}

#[derive(Subcommand)]
pub enum ProfileCommands {
    /// Switch to a different profile
    #[command(alias = "use")]
    Switch {
        /// Profile name
        name: String,

        /// Create the profile if it doesn't exist
        #[arg(short)]
        c: bool,
    },

    /// List all profiles
    #[command(alias = "ls")]
    List,

    /// Delete a profile
    #[command(alias = "rm")]
    Delete {
        /// Profile name to delete
        name: String,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Args)]
pub struct SelfUpgradeArgs {
    /// Force reinstall even if already at latest version
    #[arg(long, short)]
    pub force: bool,
}
