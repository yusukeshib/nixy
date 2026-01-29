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
    /// Install a package from nixpkgs [alias: add]
    #[command(alias = "add")]
    Install(InstallArgs),

    /// Uninstall a package [alias: remove]
    #[command(alias = "remove")]
    Uninstall(UninstallArgs),

    /// List packages in flake.nix [alias: ls]
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
    /// Profile name
    pub name: Option<String>,

    /// Create the profile if it doesn't exist
    #[arg(short, conflicts_with = "d")]
    pub c: bool,

    /// Delete the specified profile
    #[arg(short, conflicts_with = "c")]
    pub d: bool,
}

#[derive(Args)]
pub struct SelfUpgradeArgs {
    /// Force reinstall even if already at latest version
    #[arg(long, short)]
    pub force: bool,
}
