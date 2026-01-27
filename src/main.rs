mod cli;
mod commands;
mod config;
mod error;
mod flake;
mod nix;
mod profile;
mod state;

use clap::Parser;

use cli::{Cli, Commands};
use config::Config;
use error::Error;
use nix::Nix;

fn main() {
    // Check dependencies
    if let Err(e) = Nix::check_installed() {
        commands::error(&e.to_string());
        std::process::exit(1);
    }

    let cli = Cli::parse();
    let config = Config::new();

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
