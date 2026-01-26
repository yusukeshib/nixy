pub mod config;
pub mod install;
pub mod list;
pub mod profile;
pub mod search;
pub mod self_upgrade;
pub mod sync;
pub mod uninstall;
pub mod upgrade;
pub mod version;

use colored::Colorize;

/// Print info message
pub fn info(msg: &str) {
    println!("{} {}", "==>".blue(), msg);
}

/// Print success message
pub fn success(msg: &str) {
    println!("{} {}", "==>".green(), msg);
}

/// Print warning message
pub fn warn(msg: &str) {
    eprintln!("{} {}", "Warning:".yellow(), msg);
}

/// Print error message
pub fn error(msg: &str) {
    eprintln!("{} {}", "Error:".red(), msg);
}
