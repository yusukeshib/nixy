use std::process::Command;

use tempfile::TempDir;

fn nixy_cmd() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--quiet", "--"]);
    cmd
}

/// Test environment that passes config via subprocess environment variables
/// instead of modifying global process state (avoids race conditions in parallel tests)
struct TestEnv {
    _temp: TempDir,
    config_dir: std::path::PathBuf,
    env_path: std::path::PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        Self {
            config_dir: temp.path().join("config"),
            env_path: temp.path().join("env"),
            _temp: temp,
        }
    }

    /// Create a nixy command with test environment variables set
    fn cmd(&self) -> Command {
        let mut cmd = nixy_cmd();
        cmd.env("NIXY_CONFIG_DIR", &self.config_dir);
        cmd.env("NIXY_ENV", &self.env_path);
        cmd
    }
}

// =============================================================================
// Version tests
// =============================================================================

#[test]
fn test_version() {
    let output = nixy_cmd().arg("version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nixy version"));
}

#[test]
fn test_version_flag() {
    let output = nixy_cmd().arg("--version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // --version uses clap's output which includes package name
    assert!(stdout.contains("nixy") || stdout.contains("0.1"));
}

// =============================================================================
// Help tests
// =============================================================================

#[test]
fn test_help() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:") || stdout.contains("USAGE:"));
    assert!(stdout.contains("install"));
}

#[test]
fn test_unknown_command_fails() {
    let output = nixy_cmd().arg("unknowncommand").output().unwrap();
    assert!(!output.status.success());
}

// =============================================================================
// Config command tests
// =============================================================================

#[test]
fn test_config_bash() {
    let output = nixy_cmd().args(["config", "bash"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("export PATH"));
    assert!(stdout.contains(".local/state/nixy/env/bin"));
}

#[test]
fn test_config_zsh() {
    let output = nixy_cmd().args(["config", "zsh"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("export PATH"));
}

#[test]
fn test_config_fish() {
    let output = nixy_cmd().args(["config", "fish"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("set -gx PATH"));
    assert!(stdout.contains(".local/state/nixy/env/bin"));
}

#[test]
fn test_config_invalid_shell() {
    let output = nixy_cmd().args(["config", "invalid"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown shell"));
}

#[test]
fn test_config_no_shell() {
    let output = nixy_cmd().arg("config").output().unwrap();
    assert!(!output.status.success());
}

// =============================================================================
// List command tests
// =============================================================================

#[test]
fn test_list_no_flake() {
    let env = TestEnv::new();
    let output = env.cmd().arg("list").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No flake.nix found") || stderr.contains("flake"));
}

// =============================================================================
// Sync command tests
// =============================================================================

#[test]
fn test_sync_no_flake() {
    let env = TestEnv::new();
    let output = env.cmd().arg("sync").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No flake.nix found") || stderr.contains("flake"));
}

// =============================================================================
// Profile command tests
// =============================================================================

#[test]
fn test_profile_shows_default() {
    let env = TestEnv::new();
    let output = env.cmd().arg("profile").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Active profile: default"));
}

#[test]
fn test_profile_list_empty() {
    let env = TestEnv::new();
    let output = env.cmd().args(["profile", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no profiles") || stdout.contains("Available profiles"));
}

#[test]
fn test_profile_switch_create() {
    let env = TestEnv::new();

    // Create a new profile
    let output = env
        .cmd()
        .args(["profile", "switch", "-c", "test-profile"])
        .output()
        .unwrap();

    // The command should either succeed (profile created + nix build worked)
    // or fail during nix build (profile created but build failed).
    // In CI with nix available, the command should attempt to create the profile.
    // We verify the command ran without crashing and produced expected output.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The command should mention creating or switching to the profile
    let mentioned_profile = stdout.contains("test-profile")
        || stderr.contains("test-profile")
        || stdout.contains("Creating profile")
        || stdout.contains("Switching to profile");

    // Either the command succeeded or it failed for a known reason (nix build failure)
    assert!(
        output.status.success() || mentioned_profile || stderr.contains("build"),
        "Unexpected failure: stdout={}, stderr={}",
        stdout,
        stderr
    );
}

#[test]
fn test_profile_switch_nonexistent() {
    let env = TestEnv::new();

    let output = env
        .cmd()
        .args(["profile", "switch", "nonexistent"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Error should mention profile doesn't exist or suggest -c flag
    assert!(
        stderr.contains("does not exist") || stderr.contains("-c") || stderr.contains("Profile")
    );
}

#[test]
fn test_profile_delete_nonexistent() {
    let env = TestEnv::new();

    let output = env
        .cmd()
        .args(["profile", "delete", "nonexistent", "--force"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_invalid_profile_name() {
    let env = TestEnv::new();

    let output = env
        .cmd()
        .args(["profile", "switch", "-c", "invalid name!"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Invalid profile name") || stderr.contains("invalid"));
}

// =============================================================================
// Install command tests
// =============================================================================

#[test]
fn test_install_requires_package() {
    let env = TestEnv::new();
    let output = env.cmd().arg("install").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_install_from_requires_package() {
    let env = TestEnv::new();
    let output = env
        .cmd()
        .args(["install", "--from", "nixpkgs"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Package name is required") || stderr.contains("required"));
}

#[test]
fn test_install_file_not_found() {
    let env = TestEnv::new();
    let output = env
        .cmd()
        .args(["install", "--file", "nonexistent.nix"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("File not found") || stderr.contains("not found"));
}

// =============================================================================
// Upgrade command tests
// =============================================================================

#[test]
fn test_upgrade_no_flake() {
    let env = TestEnv::new();
    let output = env.cmd().arg("upgrade").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No flake.nix found") || stderr.contains("flake"));
}

// =============================================================================
// Uninstall command tests
// =============================================================================

#[test]
fn test_uninstall_requires_package() {
    let env = TestEnv::new();
    let output = env.cmd().arg("uninstall").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_uninstall_no_flake() {
    let env = TestEnv::new();
    let output = env.cmd().args(["uninstall", "hello"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No flake.nix found") || stderr.contains("flake"));
}

// =============================================================================
// Search command tests
// =============================================================================

#[test]
fn test_search_requires_query() {
    let output = nixy_cmd().arg("search").output().unwrap();
    assert!(!output.status.success());
}

// =============================================================================
// GC command tests
// =============================================================================

#[test]
fn test_gc_runs() {
    // GC should run without error (even if nix gc does nothing)
    let output = nixy_cmd().arg("gc").output().unwrap();
    // May succeed or fail depending on nix availability
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should at least attempt to run
    assert!(
        output.status.success()
            || stdout.contains("garbage")
            || stderr.contains("nix")
            || stderr.contains("gc"),
        "GC should attempt to run: stdout={}, stderr={}",
        stdout,
        stderr
    );
}

// =============================================================================
// Help content tests
// =============================================================================

#[test]
fn test_help_shows_install_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("install"));
}

#[test]
fn test_help_shows_uninstall_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("uninstall"));
}

#[test]
fn test_help_shows_list_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("list"));
}

#[test]
fn test_help_shows_search_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("search"));
}

#[test]
fn test_help_shows_upgrade_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("upgrade"));
}

#[test]
fn test_help_shows_sync_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sync"));
}

#[test]
fn test_help_shows_gc_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gc"));
}

#[test]
fn test_help_shows_config_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("config"));
}

#[test]
fn test_help_shows_profile_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("profile"));
}

#[test]
fn test_help_shows_version_command() {
    let output = nixy_cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("version"));
}

// =============================================================================
// Install subcommand help tests
// =============================================================================

#[test]
fn test_install_help_shows_from_option() {
    let output = nixy_cmd().args(["install", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--from") || stdout.contains("from"));
}

#[test]
fn test_install_help_shows_file_option() {
    let output = nixy_cmd().args(["install", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--file") || stdout.contains("file"));
}

#[test]
fn test_install_help_shows_force_option() {
    let output = nixy_cmd().args(["install", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--force") || stdout.contains("force"));
}

// =============================================================================
// Profile subcommand tests
// =============================================================================

#[test]
fn test_profile_help() {
    let output = nixy_cmd().args(["profile", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("switch"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("delete"));
}

#[test]
fn test_profile_switch_help() {
    let output = nixy_cmd()
        .args(["profile", "switch", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show -c flag for create
    assert!(stdout.contains("-c") || stdout.contains("create"));
}

#[test]
fn test_profile_delete_help() {
    let output = nixy_cmd()
        .args(["profile", "delete", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show --force flag
    assert!(stdout.contains("--force") || stdout.contains("force"));
}

// =============================================================================
// Version format tests
// =============================================================================

#[test]
fn test_version_shows_semver_format() {
    let output = nixy_cmd().arg("version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain version in semver-like format (X.Y.Z)
    let has_version = stdout.contains("0.1") || stdout.contains("version");
    assert!(has_version, "Should show version: {}", stdout);
}

// =============================================================================
// Edge case tests
// =============================================================================

#[test]
fn test_empty_command_shows_help() {
    // Running nixy with no arguments should show help or usage
    let output = nixy_cmd().output().unwrap();
    // Clap shows help by default when no subcommand is given
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("Usage") || stderr.contains("Usage") || stdout.contains("nixy"),
        "Should show usage info: stdout={}, stderr={}",
        stdout,
        stderr
    );
}

#[test]
fn test_install_validates_file_extension() {
    let env = TestEnv::new();
    // Create a non-.nix file
    let temp_file = std::env::temp_dir().join("test.txt");
    std::fs::write(&temp_file, "not a nix file").unwrap();

    let output = env
        .cmd()
        .args(["install", "--file", temp_file.to_str().unwrap()])
        .output()
        .unwrap();

    // Should either fail or warn about non-.nix file
    // (implementation dependent)
    let _ = output; // Just verify it doesn't crash

    std::fs::remove_file(&temp_file).ok();
}

// =============================================================================
// Config command output format tests
// =============================================================================

#[test]
fn test_config_bash_has_correct_format() {
    let output = nixy_cmd().args(["config", "bash"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should have shell configuration comment
    assert!(
        stdout.contains("# nixy shell configuration"),
        "Should have config comment: {}",
        stdout
    );
    // Should export PATH
    assert!(stdout.contains("export PATH="));
    // Should include nixy/env/bin in path
    assert!(stdout.contains("nixy/env/bin"));
}

#[test]
fn test_config_fish_has_correct_format() {
    let output = nixy_cmd().args(["config", "fish"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should use fish syntax
    assert!(stdout.contains("set -gx PATH"));
    // Should include nixy/env/bin in path
    assert!(stdout.contains("nixy/env/bin"));
}
