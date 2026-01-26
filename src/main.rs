mod cli;
mod commands;
mod config;
mod error;
mod flake;
mod migration;
mod nix;
mod profile;
mod state;

use clap::Parser;

use cli::{Cli, Commands};
use config::Config;
use error::Error;
use nix::Nix;
use profile::get_flake_dir;

fn main() {
    // Check dependencies
    if let Err(e) = Nix::check_installed() {
        commands::error(&e.to_string());
        std::process::exit(1);
    }

    let cli = Cli::parse();
    let config = Config::new();

    // Run migration if needed (one-time migration from marker-based to state-based)
    if let Err(e) = run_migration(&config) {
        commands::error(&format!("Migration failed: {}", e));
        std::process::exit(1);
    }

    let result = match cli.command {
        Commands::Install(args) => commands::install::run(&config, args),
        Commands::Uninstall(args) => commands::uninstall::run(&config, args),
        Commands::List => commands::list::run(&config),
        Commands::Search { query } => commands::search::run(&query),
        Commands::Upgrade(args) => commands::upgrade::run(&config, args),
        Commands::Sync(_) => commands::sync::run(&config),
        Commands::Gc => commands::gc::run(),
        Commands::Config { shell } => commands::config::run(&shell),
        Commands::Profile(args) => commands::profile::run(&config, args),
        Commands::SelfUpgrade(args) => commands::self_upgrade::run(args.force),
        Commands::Version => {
            commands::version::run();
            Ok(())
        }
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

/// Run migration if needed for the active profile
fn run_migration(config: &Config) -> error::Result<()> {
    // Get the active profile directory
    let flake_dir = match get_flake_dir(config) {
        Ok(dir) => dir,
        Err(_) => return Ok(()), // No profile yet, nothing to migrate
    };

    // Check if migration is needed
    if migration::needs_migration(&flake_dir) {
        commands::info("Migrating to new state-based format...");
        migration::migrate(&flake_dir)?;
        commands::success("Migration complete. Package state is now stored in packages.json");
    }

    Ok(())
}
