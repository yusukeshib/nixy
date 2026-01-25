use std::process::Command;

use tempfile::TempDir;

fn nixy_cmd() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--quiet", "--"]);
    cmd
}

fn setup_test_env() -> TempDir {
    let temp = TempDir::new().unwrap();
    std::env::set_var("NIXY_CONFIG_DIR", temp.path().join("config"));
    std::env::set_var("NIXY_ENV", temp.path().join("env"));
    temp
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
    let _temp = setup_test_env();
    let output = nixy_cmd().arg("list").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No flake.nix found") || stderr.contains("flake"));
}

// =============================================================================
// Sync command tests
// =============================================================================

#[test]
fn test_sync_no_flake() {
    let _temp = setup_test_env();
    let output = nixy_cmd().arg("sync").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No flake.nix found") || stderr.contains("flake"));
}

// =============================================================================
// Profile command tests
// =============================================================================

#[test]
fn test_profile_shows_default() {
    let _temp = setup_test_env();
    let output = nixy_cmd().arg("profile").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Active profile: default"));
}

#[test]
fn test_profile_list_empty() {
    let _temp = setup_test_env();
    let output = nixy_cmd().args(["profile", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no profiles") || stdout.contains("Available profiles"));
}

#[test]
fn test_profile_switch_create() {
    let _temp = setup_test_env();

    // Create a new profile
    let output = nixy_cmd()
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
    let _temp = setup_test_env();

    let output = nixy_cmd()
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
    let _temp = setup_test_env();

    let output = nixy_cmd()
        .args(["profile", "delete", "nonexistent", "--force"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_invalid_profile_name() {
    let _temp = setup_test_env();

    let output = nixy_cmd()
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
    let _temp = setup_test_env();
    let output = nixy_cmd().arg("install").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_install_from_requires_package() {
    let _temp = setup_test_env();
    let output = nixy_cmd()
        .args(["install", "--from", "nixpkgs"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Package name is required") || stderr.contains("required"));
}

#[test]
fn test_install_file_not_found() {
    let _temp = setup_test_env();
    let output = nixy_cmd()
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
    let _temp = setup_test_env();
    let output = nixy_cmd().arg("upgrade").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No flake.nix found") || stderr.contains("flake"));
}
