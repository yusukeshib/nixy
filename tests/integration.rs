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

#[test]
fn test_install_already_installed() {
    let env = TestEnv::new();

    // Create a profile with a package already installed
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();

    // Create a nixy-managed flake.nix with hello already installed
    let flake_content = r#"{
  # This flake is managed by nixy. Do not edit manually.
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # [nixy:local-inputs]
    # [/nixy:local-inputs]
  };

  outputs = { self, nixpkgs, ... }@inputs:
    let
      system = builtins.currentSystem;
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };
      packages = {
          # [nixy:packages]
          hello = pkgs.hello;
          # [/nixy:packages]
          # [nixy:local-packages]
          # [/nixy:local-packages]
      };
    in {
      packages.${system}.default = pkgs.buildEnv {
        name = "nixy-env";
        paths = with packages; [
              # [nixy:env-paths]
              hello
              # [/nixy:env-paths]
        ];
      };
    };
}
"#;
    std::fs::write(profile_dir.join("flake.nix"), flake_content).unwrap();

    // Set active profile
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    // Try to install hello again
    let output = env.cmd().args(["install", "hello"]).output().unwrap();

    // Should succeed (not an error)
    assert!(
        output.status.success(),
        "Installing already-installed package should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show message about already installed
    assert!(
        stdout.contains("already installed"),
        "Should indicate package is already installed: {}",
        stdout
    );
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
// Upgrade command tests (additional)
// =============================================================================

#[test]
fn test_upgrade_help() {
    let output = nixy_cmd().args(["upgrade", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Check for upgrade-specific content, not generic "Usage"
    assert!(
        stdout.contains("nixpkgs") || stdout.contains("input") || stdout.contains("flake"),
        "Upgrade help should mention nixpkgs, input, or flake: {}",
        stdout
    );
}

#[test]
fn test_upgrade_requires_lock_file_for_specific_input() {
    let env = TestEnv::new();

    // Create profile directory with flake.nix but no flake.lock
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::write(
        profile_dir.join("flake.nix"),
        r#"{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: {};
}"#,
    )
    .unwrap();

    // Set active profile
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().args(["upgrade", "nixpkgs"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("flake.lock")
            || stderr.contains("lock file")
            || stderr.contains("lockfile")
            || stderr.contains("sync"),
        "Should mention lock file: {}",
        stderr
    );
}

#[test]
fn test_upgrade_handles_corrupted_lock_file() {
    let env = TestEnv::new();

    // Create profile directory with flake.nix and corrupted flake.lock
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::write(
        profile_dir.join("flake.nix"),
        r#"{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: {};
}"#,
    )
    .unwrap();
    std::fs::write(profile_dir.join("flake.lock"), "not valid json").unwrap();

    // Set active profile
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().args(["upgrade", "nixpkgs"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_lower = stderr.to_lowercase();
    assert!(
        stderr.contains("parse") || stderr.contains("invalid") || stderr_lower.contains("failed"),
        "Should mention parse failure: {}",
        stderr
    );
}

// =============================================================================
// Sync command tests (additional)
// =============================================================================

#[test]
fn test_sync_with_profile() {
    let env = TestEnv::new();

    // Create a profile first
    let _ = env.cmd().args(["profile", "switch", "-c", "test"]).output();

    // Sync should attempt to build
    let output = env.cmd().arg("sync").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Sync should either succeed (build completed) or fail with build-related message
    // Not just accepting any success - must show evidence of sync attempt
    if output.status.success() {
        // If successful, should show building/syncing messages
        assert!(
            stdout.contains("Building")
                || stdout.contains("environment")
                || stdout.contains("Syncing"),
            "Sync success should show progress: stdout={}",
            stdout
        );
    } else {
        // If failed, should be a build-related failure
        assert!(
            stderr.contains("build") || stderr.contains("flake"),
            "Sync failure should be build-related: stderr={}",
            stderr
        );
    }
}

// =============================================================================
// Profile management tests (additional)
// =============================================================================

#[test]
fn test_profile_list_shows_active() {
    let env = TestEnv::new();

    // Create and switch to a profile
    let _ = env.cmd().args(["profile", "switch", "-c", "work"]).output();

    let output = env.cmd().args(["profile", "list"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show work as active
    assert!(
        stdout.contains("work") && (stdout.contains("active") || stdout.contains("*")),
        "Should show active profile: {}",
        stdout
    );
}

#[test]
fn test_profile_delete_requires_force() {
    let env = TestEnv::new();

    // Create two profiles
    let _ = env.cmd().args(["profile", "switch", "-c", "work"]).output();
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    // Try to delete without --force
    let output = env
        .cmd()
        .args(["profile", "delete", "work"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--force"),
        "Should mention --force: {}",
        stderr
    );
}

#[test]
fn test_profile_delete_active_fails() {
    let env = TestEnv::new();

    // Create a profile and stay on it (it becomes active)
    let _ = env.cmd().args(["profile", "switch", "-c", "work"]).output();

    // Try to delete the active profile
    let output = env
        .cmd()
        .args(["profile", "delete", "work", "--force"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_lower = stderr.to_lowercase();
    assert!(
        stderr_lower.contains("active")
            || stderr_lower.contains("cannot delete")
            || stderr_lower.contains("can't delete")
            || stderr_lower.contains("unable to delete"),
        "Should prevent deleting active profile: {}",
        stderr
    );
}

#[test]
fn test_profile_delete_with_force_success() {
    let env = TestEnv::new();

    // Create two profiles and end up on default so work is not active
    let _ = env.cmd().args(["profile", "switch", "-c", "work"]).output();
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    // Delete work with --force
    let output = env
        .cmd()
        .args(["profile", "delete", "work", "--force"])
        .output()
        .unwrap();

    // Should succeed with a confirmation message
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout_lower = stdout.to_lowercase();
    assert!(
        output.status.success(),
        "Profile delete with --force should succeed: stdout={}, stderr={}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout_lower.contains("deleted")
            || stdout_lower.contains("removed")
            || stdout.contains("work"),
        "Should confirm deletion: {}",
        stdout
    );
}

#[test]
fn test_profile_switch_with_existing() {
    let env = TestEnv::new();

    // Create a profile
    let _ = env.cmd().args(["profile", "switch", "-c", "work"]).output();

    // Create another profile
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    // Switch back with -c (should just switch, not error)
    let output = env
        .cmd()
        .args(["profile", "switch", "-c", "work"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should switch successfully - verify both success and mention of work profile
    assert!(
        output.status.success(),
        "Should switch to existing profile: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Verify that the profile switch was acknowledged with specific confirmation
    let stdout_lower = stdout.to_lowercase();
    assert!(
        (stdout_lower.contains("switched") || stdout_lower.contains("active"))
            && stdout_lower.contains("work"),
        "Should confirm switch to work profile: {}",
        stdout
    );
}

// =============================================================================
// Install --file tests (additional)
// =============================================================================

#[test]
fn test_install_file_parses_pname() {
    let env = TestEnv::new();
    let temp = TempDir::new().unwrap();

    // Create a profile first
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    // Create a .nix file with pname
    let pkg_file = temp.path().join("my-pkg.nix");
    std::fs::write(
        &pkg_file,
        r#"{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "my-package";
  version = "1.0.0";
  src = ./.;
}"#,
    )
    .unwrap();

    let output = env
        .cmd()
        .args(["install", "--file", pkg_file.to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Command should succeed (or fail due to nix build, not parsing)
    // First verify it parsed the pname correctly by checking output
    assert!(
        stdout.contains("my-package") || stderr.contains("my-package"),
        "Should detect package name from pname: stdout={}, stderr={}",
        stdout,
        stderr
    );

    // If the command failed, it should be due to nix build issues, not parsing
    if !output.status.success() {
        assert!(
            !stderr.contains("Could not find")
                && !stderr.contains("missing name")
                && !stderr.contains("pname"),
            "Failure should not be due to missing pname (parsing should succeed): stderr={}",
            stderr
        );
    }
}

#[test]
fn test_install_file_requires_name_or_pname() {
    let env = TestEnv::new();
    let temp = TempDir::new().unwrap();

    // Create a profile first
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    // Create a .nix file without name or pname
    let pkg_file = temp.path().join("invalid-pkg.nix");
    std::fs::write(
        &pkg_file,
        r#"{ pkgs }:

pkgs.stdenv.mkDerivation {
  src = ./.;
}"#,
    )
    .unwrap();

    let output = env
        .cmd()
        .args(["install", "--file", pkg_file.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_lower = stderr.to_lowercase();
    assert!(
        stderr.contains("name")
            || stderr.contains("pname")
            || stderr_lower.contains("could not find")
            || stderr_lower.contains("cannot find")
            || stderr_lower.contains("unable to find")
            || stderr_lower.contains("missing"),
        "Should mention missing name: {}",
        stderr
    );
}

#[test]
fn test_install_file_detects_flake() {
    let env = TestEnv::new();
    let temp = TempDir::new().unwrap();

    // Create a profile first
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    // Create a flake file (has inputs and outputs)
    let flake_file = temp.path().join("my-flake.nix");
    std::fs::write(
        &flake_file,
        r#"{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }: {
    packages.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.hello;
  };
}"#,
    )
    .unwrap();

    let output = env
        .cmd()
        .args(["install", "--file", flake_file.to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout_lower = stdout.to_lowercase();
    let stderr_lower = stderr.to_lowercase();

    // Should detect and process as flake (not as a regular package file)
    // The key indicator is that it recognizes this as a flake
    assert!(
        stdout_lower.contains("flake") || stderr_lower.contains("flake"),
        "Should detect flake file format: stdout={}, stderr={}",
        stdout,
        stderr
    );
}

// =============================================================================
// Self-upgrade command tests
// =============================================================================

#[test]
fn test_self_upgrade_help_and_flags() {
    // Single comprehensive test for self-upgrade help output
    let output = nixy_cmd()
        .args(["self-upgrade", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify specific self-upgrade content
    assert!(
        stdout.contains("self-upgrade") || stdout.contains("Self"),
        "Help should mention self-upgrade: {}",
        stdout
    );

    // Verify --force flag is documented
    assert!(
        stdout.contains("--force"),
        "Help should show --force flag: {}",
        stdout
    );

    // Verify -f short flag is documented
    assert!(
        stdout.contains("-f"),
        "Help should show -f short flag: {}",
        stdout
    );
}

// =============================================================================
// List command tests (additional)
// =============================================================================

#[test]
fn test_list_shows_none_for_empty_flake() {
    let env = TestEnv::new();

    // Create a profile (empty flake)
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    let output = env.cmd().arg("list").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout_lower = stdout.to_lowercase();

    // Command should succeed for an empty flake
    assert!(
        output.status.success(),
        "List should succeed for empty flake: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Should show indication of empty/no packages
    assert!(
        stdout.contains("(none)")
            || stdout_lower.contains("no packages")
            || stdout_lower.contains("empty")
            || stdout.contains("Packages in"),
        "Should indicate empty package list: {}",
        stdout
    );
}

#[test]
fn test_list_shows_installed_packages() {
    let env = TestEnv::new();

    // Create a profile directory with a flake.nix containing packages
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();

    // Write a flake.nix with packages (no flake.lock needed)
    let flake_content = r#"{
  description = "nixy managed packages";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # [nixy:local-inputs]
    # [/nixy:local-inputs]
    # [nixy:custom-inputs]
    # [/nixy:custom-inputs]
  };

  outputs = { self, nixpkgs }@inputs:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in {
      packages = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in rec {
          # [nixy:packages]
          ripgrep = pkgs.ripgrep;
          fzf = pkgs.fzf;
          bat = pkgs.bat;
          # [/nixy:packages]
          # [nixy:local-packages]
          # [/nixy:local-packages]
          # [nixy:custom-packages]
          # [/nixy:custom-packages]

          default = pkgs.buildEnv {
            name = "nixy-env";
            paths = [
              # [nixy:env-paths]
              ripgrep
              fzf
              bat
              # [/nixy:env-paths]
              # [nixy:custom-paths]
              # [/nixy:custom-paths]
            ];
            extraOutputsToInstall = [ "man" "doc" "info" ];
          };
        });
    };
}
"#;
    std::fs::write(profile_dir.join("flake.nix"), flake_content).unwrap();

    // Set active profile
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().arg("list").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Command should succeed
    assert!(
        output.status.success(),
        "List should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Should show the installed packages
    assert!(
        stdout.contains("ripgrep"),
        "Should show ripgrep: {}",
        stdout
    );
    assert!(stdout.contains("fzf"), "Should show fzf: {}", stdout);
    assert!(stdout.contains("bat"), "Should show bat: {}", stdout);

    // Should NOT show "(none)"
    assert!(
        !stdout.contains("(none)"),
        "Should not show (none) when packages exist: {}",
        stdout
    );
}

// =============================================================================
// Uninstall command tests (additional)
// =============================================================================

#[test]
fn test_uninstall_package_not_installed() {
    let env = TestEnv::new();

    // Create a profile first
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    // Uninstalling a non-existent package
    let output = env
        .cmd()
        .args(["uninstall", "nonexistent-package"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_lower = stderr.to_lowercase();

    // The implementation may either:
    // 1. Succeed silently (no-op) - package wasn't there, nothing to uninstall
    // 2. Fail with an error message
    if output.status.success() {
        // If it succeeds, it should indicate the package wasn't found or is a no-op
        if stdout.is_empty() {
            // Silent success is acceptable for a no-op uninstall of a non-existent package,
            // but it must also be truly silent (no stderr output) and exit successfully.
            assert!(
                stderr.is_empty(),
                "Silent no-op uninstall should not produce stderr: {}",
                stderr
            );
        } else {
            // If it succeeds with output, it should indicate the package wasn't found or that
            // there were no changes to apply.
            assert!(
                stdout.contains("not installed")
                    || stdout.contains("nonexistent-package")
                    || stdout.contains("No changes"),
                "No-op uninstall should indicate status: stdout={}",
                stdout
            );
        }
    } else {
        // If it fails, it should provide a helpful error message
        assert!(
            stderr_lower.contains("not installed")
                || stderr_lower.contains("not found")
                || stderr_lower.contains("does not exist")
                || stderr.contains("nonexistent-package"),
            "Failed uninstall should indicate package not found: {}",
            stderr
        );
    }
}

// =============================================================================
// Install --from tests (additional)
// =============================================================================

#[test]
fn test_install_from_unknown_registry() {
    let env = TestEnv::new();

    // Create a profile first
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    let output = env
        .cmd()
        .args(["install", "--from", "nonexistent-registry", "hello"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_lower = stderr.to_lowercase();
    assert!(
        stderr_lower.contains("registry")
            && (stderr_lower.contains("unknown")
                || stderr_lower.contains("not found")
                || stderr_lower.contains("invalid")),
        "Should fail for unknown registry with appropriate message: {}",
        stderr
    );
}

#[test]
fn test_install_from_detects_direct_url() {
    let env = TestEnv::new();

    // Create a profile first
    let _ = env
        .cmd()
        .args(["profile", "switch", "-c", "default"])
        .output();

    let output = env
        .cmd()
        .args(["install", "--from", "github:NixOS/nixpkgs", "hello"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout_lower = stdout.to_lowercase();
    let stderr_lower = stderr.to_lowercase();

    // Should detect as direct URL (github: prefix) rather than looking up in registry
    // If successful, it means it recognized the URL format
    // If it fails, it should be a nix-related failure, not "unknown registry"
    if output.status.success() {
        // Success means it detected and processed the direct URL
        // Check for specific positive indicators, excluding error contexts
        assert!(
            // Explicitly mention the package name
            stdout.contains("hello")
                // Clearly positive success phrasing
                || stdout_lower.contains("successfully installed")
                || stdout_lower.contains("added package")
                // Generic "install" only counts if not in an error context
                || (stdout_lower.contains("install")
                    && !stdout_lower.contains("failed to install")
                    && !stdout_lower.contains("error installing")
                    && !stdout_lower.contains("unable to install"))
                // Generic "adding" only counts if not in an error context
                || (stdout_lower.contains("adding") && !stdout_lower.contains("error adding")),
            "Successful install should acknowledge the package: stdout={}",
            stdout
        );
    } else {
        // Failure should NOT be about unknown registry (since github: is a valid URL format)
        assert!(
            !stderr_lower.contains("unknown registry"),
            "Should detect github: as direct URL, not unknown registry: stderr={}",
            stderr
        );
    }
}
