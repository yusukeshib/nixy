mod cli;
mod commands;
mod config;
mod error;
mod flake;
mod migration;
mod nix;
mod nixhub;
mod nixy_config;
mod profile;
mod rollback;
mod state;

use clap::Parser;

use cli::{Cli, Commands};
use config::Config;
use error::Error;
use nix::Nix;

fn main() {
    // Initialize signal handler for Ctrl+C rollback
    rollback::init_signal_handler();

    // Check dependencies
    if let Err(e) = Nix::check_installed() {
        commands::error(&e.to_string());
        std::process::exit(1);
    }

    let cli = Cli::parse();
    let config = Config::new();

    // Commands that don't need config state (skip migration)
    let skip_migration = matches!(
        &cli.command,
        Commands::Config { .. } | Commands::Search { .. } | Commands::SelfUpgrade(_)
    );

    // Auto-migrate from legacy format if needed
    if !skip_migration {
        if let Err(e) = migration::run_migration_if_needed(&config) {
            commands::error(&e.to_string());
            std::process::exit(1);
        }
    }

    let result = match cli.command {
        Commands::Install(args) => commands::install::run(&config, args),
        Commands::Uninstall(args) => commands::uninstall::run(&config, args),
        Commands::List => commands::list::run(&config),
        Commands::Search { query } => commands::search::run(&query),
        Commands::Upgrade(args) => commands::upgrade::run(&config, args),
        Commands::Sync(_) => commands::sync::run(&config),
        Commands::Config { shell } => commands::config::run(&shell),
        Commands::Profile(args) => commands::profile::run(&config, args),
        Commands::SelfUpgrade(args) => commands::self_upgrade::run(args.force),
        Commands::File(args) => commands::file::run(&config, args),
    };

    if let Err(e) = result {
        match e {
            Error::Usage(msg) => {
                // Usage errors don't need "Error:" prefix
                eprintln!("{}", msg);
            }
            _ => {
                commands::error(&e.to_string());
            }
        }
        std::process::exit(1);
    }
}
