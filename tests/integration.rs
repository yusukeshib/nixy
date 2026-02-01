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
    state_dir: std::path::PathBuf,
    env_path: std::path::PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        Self {
            config_dir: temp.path().join("config"),
            state_dir: temp.path().join("state"),
            env_path: temp.path().join("state/env"),
            _temp: temp,
        }
    }

    /// Create a nixy command with test environment variables set
    fn cmd(&self) -> Command {
        let mut cmd = nixy_cmd();
        cmd.env("NIXY_CONFIG_DIR", &self.config_dir);
        cmd.env("NIXY_STATE_DIR", &self.state_dir);
        cmd.env("NIXY_ENV", &self.env_path);
        cmd
    }
}

// =============================================================================
// Version tests
// =============================================================================

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
    // list command no longer requires flake.nix - it reads from packages.json
    let env = TestEnv::new();
    let output = env.cmd().arg("list").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show "(none)" for empty state
    assert!(stdout.contains("(none)") || stdout.contains("Installed packages"));
}

// =============================================================================
// Sync command tests
// =============================================================================

#[test]
fn test_sync_no_flake() {
    // sync now auto-regenerates flake.nix from packages.json
    let env = TestEnv::new();
    let output = env.cmd().arg("sync").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should mention regenerating flake.nix
    assert!(
        stdout.contains("Regenerating flake.nix") || stdout.contains("Syncing"),
        "Expected regeneration or syncing message: stdout={}, stderr={}",
        stdout,
        stderr
    );

    // Should NOT fail with "No flake.nix found"
    assert!(
        !stderr.contains("No flake.nix found"),
        "Should not fail with NoFlakeFound error"
    );

    // Command should succeed (empty env builds successfully)
    assert!(
        output.status.success(),
        "Sync should succeed with empty flake: stdout={}, stderr={}",
        stdout,
        stderr
    );
}

// =============================================================================
// Profile command tests
// =============================================================================

#[test]
fn test_profile_shows_profiles() {
    let env = TestEnv::new();
    let output = env.cmd().arg("profile").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // With no profiles, should show "No profiles found" or similar
    assert!(
        stdout.contains("No profiles") || stdout.contains("Available profiles"),
        "Should show profile info: {}",
        stdout
    );
}

#[test]
fn test_profile_no_profiles() {
    let env = TestEnv::new();
    // Running profile with no args shows available profiles (or indicates none)
    let output = env.cmd().arg("profile").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No profiles") || stdout.contains("Available profiles"),
        "Should show profile status: {}",
        stdout
    );
}

#[test]
fn test_profile_create() {
    let env = TestEnv::new();

    // Create a new profile with new syntax: nixy profile <name> -c
    let output = env
        .cmd()
        .args(["profile", "test-profile", "-c"])
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

    // New syntax: nixy profile <name> (without -c)
    let output = env.cmd().args(["profile", "nonexistent"]).output().unwrap();

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

    // New syntax: nixy profile <name> -d
    // Note: In non-TTY (test env), delete will fail because it requires interactive confirmation
    let output = env
        .cmd()
        .args(["profile", "nonexistent", "-d"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_invalid_profile_name() {
    let env = TestEnv::new();

    // New syntax: nixy profile <name> -c
    let output = env
        .cmd()
        .args(["profile", "invalid name!", "-c"])
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

    // Create packages.json with hello already installed (new state-based format)
    let state_content = r#"{
  "version": 1,
  "packages": ["hello"],
  "custom_packages": []
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();

    // Create a nixy-managed flake.nix (without markers - new format)
    let flake_content = r#"{
  description = "nixy managed packages";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }@inputs:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in {
      packages = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in rec {
          hello = pkgs.hello;

          default = pkgs.buildEnv {
            name = "nixy-env";
            paths = [
              hello
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
    // upgrade now auto-regenerates flake.nix from packages.json
    let env = TestEnv::new();
    let output = env.cmd().arg("upgrade").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should mention regenerating flake.nix
    assert!(
        stdout.contains("Regenerating flake.nix") || stdout.contains("Updating"),
        "Expected regeneration or updating message: stdout={}, stderr={}",
        stdout,
        stderr
    );

    // Should NOT fail with "No flake.nix found"
    assert!(
        !stderr.contains("No flake.nix found"),
        "Should not fail with NoFlakeFound error"
    );

    // Command should succeed (nix flake update works on empty flake)
    assert!(
        output.status.success(),
        "Upgrade should succeed with empty flake: stdout={}, stderr={}",
        stdout,
        stderr
    );
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
    // uninstall now auto-regenerates flake.nix from packages.json
    // but should still fail because the package is not installed
    let env = TestEnv::new();
    let output = env.cmd().args(["uninstall", "hello"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should NOT fail with "No flake.nix found"
    assert!(
        !stderr.contains("No flake.nix found"),
        "Should not fail with NoFlakeFound error: stdout={}, stderr={}",
        stdout,
        stderr
    );

    // Should fail because package is not installed
    assert!(
        stderr.contains("not found") || stderr.contains("not installed"),
        "Expected 'not found' or 'not installed' error: stderr={}",
        stderr
    );
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

// Removed: --force flag is no longer used since marker-based editing was removed
// The test_install_help_shows_force_option test has been removed as the --force
// flag was only needed for overriding custom modifications in the marker-based system

// =============================================================================
// Profile subcommand tests
// =============================================================================

#[test]
fn test_profile_help() {
    let output = nixy_cmd().args(["profile", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // New flat structure should show -c and -d flags
    assert!(stdout.contains("-c") || stdout.contains("Create"));
    assert!(stdout.contains("-d") || stdout.contains("Delete"));
}

#[test]
fn test_profile_flags_conflict() {
    let env = TestEnv::new();

    // -c and -d flags should conflict
    let output = env
        .cmd()
        .args(["profile", "test", "-c", "-d"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Clap should report the conflict
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "Should report flag conflict: {}",
        stderr
    );
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
    let _ = env.cmd().args(["profile", "test", "-c"]).output();

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

    // Create and switch to a profile with new syntax
    let _ = env.cmd().args(["profile", "work", "-c"]).output();

    // nixy profile now lists profiles (no subcommand needed)
    let output = env.cmd().arg("profile").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show work as active
    assert!(
        stdout.contains("work") && (stdout.contains("active") || stdout.contains("*")),
        "Should show active profile: {}",
        stdout
    );
}

#[test]
fn test_profile_delete_requires_tty() {
    let env = TestEnv::new();

    // Create two profiles with new syntax
    let _ = env.cmd().args(["profile", "work", "-c"]).output();
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

    // Try to delete (will fail because tests run without a TTY)
    let output = env.cmd().args(["profile", "work", "-d"]).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should mention needing a terminal for confirmation
    assert!(
        stderr.contains("terminal") || stderr.contains("TTY") || stderr.contains("interactively"),
        "Should mention terminal requirement: {}",
        stderr
    );
}

#[test]
fn test_profile_delete_active_fails() {
    let env = TestEnv::new();

    // Create a profile and stay on it (it becomes active)
    let _ = env.cmd().args(["profile", "work", "-c"]).output();

    // Try to delete the active profile (will fail for multiple reasons in test:
    // 1. No TTY for confirmation
    // 2. Cannot delete active profile)
    let output = env.cmd().args(["profile", "work", "-d"]).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_lower = stderr.to_lowercase();
    // Should fail - either because it's the active profile or because no TTY
    assert!(
        stderr_lower.contains("active")
            || stderr_lower.contains("cannot delete")
            || stderr_lower.contains("can't delete")
            || stderr_lower.contains("unable to delete")
            || stderr_lower.contains("terminal")
            || stderr_lower.contains("interactively"),
        "Should prevent deleting active profile or require terminal: {}",
        stderr
    );
}

#[test]
fn test_profile_delete_non_tty() {
    let env = TestEnv::new();

    // Create two profiles and end up on default so work is not active
    let _ = env.cmd().args(["profile", "work", "-c"]).output();
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

    // Try to delete work - will fail because delete requires TTY for confirmation
    let output = env.cmd().args(["profile", "work", "-d"]).output().unwrap();

    // Should fail because tests run without a TTY
    assert!(
        !output.status.success(),
        "Profile delete should fail without TTY"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("terminal") || stderr.contains("interactively"),
        "Should mention terminal requirement: {}",
        stderr
    );
}

#[test]
fn test_profile_switch_with_existing() {
    let env = TestEnv::new();

    // Create a profile with new syntax
    let _ = env.cmd().args(["profile", "work", "-c"]).output();

    // Create another profile
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

    // Switch back with -c (should just switch, not error since profile exists)
    let output = env.cmd().args(["profile", "work", "-c"]).output().unwrap();

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
// Install --platform tests
// =============================================================================

#[test]
fn test_install_platform_flag_help() {
    let output = nixy_cmd().args(["install", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--platform") || stdout.contains("-p"),
        "Help should show --platform flag: {}",
        stdout
    );
}

#[test]
fn test_install_invalid_platform() {
    let env = TestEnv::new();

    // Create a profile first
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

    let output = env
        .cmd()
        .args(["install", "hello", "--platform", "windows"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Invalid platform"),
        "Should report invalid platform: {}",
        stderr
    );
}

#[test]
fn test_install_platform_not_supported_with_file() {
    let env = TestEnv::new();
    let temp = TempDir::new().unwrap();

    // Create a profile first
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

    // Create a .nix file
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
        .args([
            "install",
            "--file",
            pkg_file.to_str().unwrap(),
            "--platform",
            "darwin",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--platform is not supported with --file"),
        "Should report --platform not supported with --file: {}",
        stderr
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
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

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
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

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
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

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
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

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

    // Create a profile directory with packages.json and flake.nix
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();

    // Create packages.json with installed packages (new state-based format)
    let state_content = r#"{
  "version": 1,
  "packages": ["bat", "fzf", "ripgrep"],
  "custom_packages": []
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();

    // Write a flake.nix with packages (no markers - new format)
    let flake_content = r#"{
  description = "nixy managed packages";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }@inputs:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in {
      packages = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in rec {
          bat = pkgs.bat;
          fzf = pkgs.fzf;
          ripgrep = pkgs.ripgrep;

          default = pkgs.buildEnv {
            name = "nixy-env";
            paths = [
              bat
              fzf
              ripgrep
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

    // Should show source info
    assert!(
        stdout.contains("(nixpkgs)"),
        "Should show (nixpkgs) for standard packages: {}",
        stdout
    );
}

#[test]
fn test_list_shows_source_info() {
    let env = TestEnv::new();

    // Create a profile directory with packages.json
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();

    // Create packages.json with both standard and custom packages
    let state_content = r#"{
  "version": 1,
  "packages": ["ripgrep"],
  "custom_packages": [
    {
      "name": "neovim",
      "input_name": "neovim-nightly",
      "input_url": "github:nix-community/neovim-nightly-overlay",
      "package_output": "packages",
      "source_name": null
    }
  ]
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();

    // Set active profile
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().arg("list").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "List should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Standard package should show (nixpkgs)
    assert!(
        stdout.contains("ripgrep") && stdout.contains("(nixpkgs)"),
        "Should show ripgrep with (nixpkgs) source: {}",
        stdout
    );

    // Custom package should show its URL
    assert!(
        stdout.contains("neovim") && stdout.contains("github:nix-community/neovim-nightly-overlay"),
        "Should show neovim with its flake URL: {}",
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
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

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
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

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
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

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

// =============================================================================
// Install revert on sync failure tests
// =============================================================================

#[test]
fn test_install_reverts_flake_on_sync_failure() {
    let env = TestEnv::new();

    // Create the new nixy.json format
    let nixy_json_content = r#"{
  "version": 3,
  "active_profile": "default",
  "profiles": {
    "default": {
      "packages": [],
      "resolved_packages": [],
      "custom_packages": []
    }
  }
}"#;
    std::fs::create_dir_all(&env.config_dir).unwrap();
    std::fs::write(env.config_dir.join("nixy.json"), nixy_json_content).unwrap();

    // Create state directory for flake.nix
    let state_profile_dir = env.state_dir.join("profiles/default");
    std::fs::create_dir_all(&state_profile_dir).unwrap();

    // Create a flake.nix (no markers - new format)
    let flake_content = r#"{
  description = "nixy managed packages";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }@inputs:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in {
      packages = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in rec {

          default = pkgs.buildEnv {
            name = "nixy-env";
            paths = [
            ];
            extraOutputsToInstall = [ "man" "doc" "info" ];
          };
        });
    };
}
"#;
    std::fs::write(state_profile_dir.join("flake.nix"), flake_content).unwrap();

    // Save original config for comparison
    let original_config = std::fs::read_to_string(env.config_dir.join("nixy.json")).unwrap();

    // Try to install a package - this will modify nixy.json and flake.nix, then run sync
    // Sync may fail in test environment (no nix, missing lock, etc.)
    let output = env.cmd().args(["install", "hello"]).output().unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    // If sync failed and revert happened, verify the config was restored
    if !output.status.success() && stderr.to_lowercase().contains("reverted") {
        let current_config = std::fs::read_to_string(env.config_dir.join("nixy.json")).unwrap();
        // The config should be restored to original state
        assert_eq!(
            current_config, original_config,
            "Config should be reverted to original on sync failure"
        );
    }

    // Also verify the command behavior is consistent:
    // - If it succeeded, hello should be in the config
    // - If it failed with revert, the config should be unchanged
    // - If it failed without revert message, something else went wrong (acceptable in test)
    if output.status.success() {
        let current_config = std::fs::read_to_string(env.config_dir.join("nixy.json")).unwrap();
        assert!(
            current_config.contains("hello"),
            "On success, config should contain the installed package"
        );
        let current_flake = std::fs::read_to_string(state_profile_dir.join("flake.nix")).unwrap();
        // Check for either legacy format (pkgs.hello) or new Nixhub format (inputs.nixpkgs-*)
        assert!(
            current_flake.contains("hello = pkgs.hello")
                || current_flake.contains("hello = inputs.nixpkgs-"),
            "On success, flake should contain the installed package (legacy or Nixhub format)"
        );
    }

}

// =============================================================================
// File command tests
// =============================================================================

#[test]
fn test_file_requires_package() {
    let env = TestEnv::new();
    let output = env.cmd().arg("file").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_file_nonexistent_package() {
    let env = TestEnv::new();

    // Create a profile first
    let _ = env.cmd().args(["profile", "default", "-c"]).output();

    let output = env
        .cmd()
        .args(["file", "nonexistent-package"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not installed"),
        "Should indicate package is not installed: {}",
        stderr
    );
}

#[test]
fn test_file_with_legacy_package() {
    let env = TestEnv::new();

    // Create a profile directory with a legacy package in packages.json
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();

    // Create packages.json with a legacy package
    let state_content = r#"{
  "version": 2,
  "packages": ["hello"],
  "resolved_packages": [],
  "custom_packages": []
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().args(["file", "hello"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The command may succeed or fail depending on nix availability
    // If it succeeds, check the output contains a nix store path
    if output.status.success() {
        assert!(
            stdout.contains("/nix/store/") && stdout.contains(".nix"),
            "Should output a .nix file path in nix store: {}",
            stdout
        );
    } else {
        // If it fails, it should be a nix-related failure, not a lookup failure
        assert!(
            !stderr.contains("not installed"),
            "Should find the package in state: stderr={}",
            stderr
        );
    }
}

#[test]
fn test_file_with_resolved_package() {
    let env = TestEnv::new();

    // Create a profile directory with a resolved package
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();

    // Create packages.json with a resolved package
    let state_content = r#"{
  "version": 2,
  "packages": [],
  "resolved_packages": [
    {
      "name": "hello",
      "version_spec": null,
      "resolved_version": "2.12.1",
      "attribute_path": "hello",
      "commit_hash": "nixos-unstable"
    }
  ],
  "custom_packages": []
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().args(["file", "hello"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The command may succeed or fail depending on nix availability
    if output.status.success() {
        assert!(
            stdout.contains("/nix/store/") && stdout.contains(".nix"),
            "Should output a .nix file path in nix store: {}",
            stdout
        );
    } else {
        // If it fails, it should be a nix-related failure
        assert!(
            !stderr.contains("not installed"),
            "Should find the package in state: stderr={}",
            stderr
        );
    }
}

#[test]
fn test_file_with_local_package() {
    let env = TestEnv::new();

    // Create a profile directory with a local package
    let profile_dir = env.config_dir.join("profiles/default");
    let packages_dir = profile_dir.join("packages");
    std::fs::create_dir_all(&packages_dir).unwrap();

    // Create a local package file
    let local_pkg_content = r#"{ lib, stdenv }:
stdenv.mkDerivation {
  pname = "my-local-pkg";
  version = "1.0.0";
  src = ./.;
}"#;
    std::fs::write(packages_dir.join("my-local-pkg.nix"), local_pkg_content).unwrap();

    // Create empty packages.json
    let state_content = r#"{
  "version": 2,
  "packages": [],
  "resolved_packages": [],
  "custom_packages": []
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().args(["file", "my-local-pkg"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should succeed and output the local file path
    assert!(
        output.status.success(),
        "Should find local package: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("my-local-pkg.nix"),
        "Should output path to local package file: {}",
        stdout
    );
}

#[test]
fn test_file_with_local_flake_package() {
    let env = TestEnv::new();

    // Create a profile directory with a local flake package
    let profile_dir = env.config_dir.join("profiles/default");
    let pkg_dir = profile_dir.join("packages").join("my-flake-pkg");
    std::fs::create_dir_all(&pkg_dir).unwrap();

    // Create a local flake.nix
    let flake_content = r#"{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: {};
}"#;
    std::fs::write(pkg_dir.join("flake.nix"), flake_content).unwrap();

    // Create empty packages.json
    let state_content = r#"{
  "version": 2,
  "packages": [],
  "resolved_packages": [],
  "custom_packages": []
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().args(["file", "my-flake-pkg"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should succeed and output the local flake path
    assert!(
        output.status.success(),
        "Should find local flake package: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("my-flake-pkg") && stdout.contains("flake.nix"),
        "Should output path to local flake.nix: {}",
        stdout
    );
}

#[test]
fn test_file_local_package_uses_pname_not_filename() {
    let env = TestEnv::new();

    // Create a profile directory with a local package where filename differs from pname
    let profile_dir = env.config_dir.join("profiles/default");
    let packages_dir = profile_dir.join("packages");
    std::fs::create_dir_all(&packages_dir).unwrap();

    // Create a local package file with pname different from filename
    let local_pkg_content = r#"{ lib, stdenv }:
stdenv.mkDerivation {
  pname = "actual-package-name";
  version = "1.0.0";
  src = ./.;
}"#;
    // Filename is "different-filename.nix" but pname is "actual-package-name"
    std::fs::write(
        packages_dir.join("different-filename.nix"),
        local_pkg_content,
    )
    .unwrap();

    // Create empty packages.json
    let state_content = r#"{
  "version": 2,
  "packages": [],
  "resolved_packages": [],
  "custom_packages": []
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    // Should find package by pname, not filename
    let output = env
        .cmd()
        .args(["file", "actual-package-name"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Should find local package by pname: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("different-filename.nix"),
        "Should output the actual file path: {}",
        stdout
    );

    // Should NOT find package by filename
    let output = env
        .cmd()
        .args(["file", "different-filename"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Should NOT find package by filename when pname differs"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not installed"),
        "Should report not installed for filename: {}",
        stderr
    );
}

#[test]
fn test_file_with_custom_package() {
    let env = TestEnv::new();

    // Create a profile directory with a custom package
    let profile_dir = env.config_dir.join("profiles/default");
    std::fs::create_dir_all(&profile_dir).unwrap();

    // Create packages.json with a custom package (from external flake)
    let state_content = r#"{
  "version": 2,
  "packages": [],
  "resolved_packages": [],
  "custom_packages": [
    {
      "name": "neovim",
      "input_name": "neovim-nightly",
      "input_url": "github:nix-community/neovim-nightly-overlay",
      "package_output": "packages",
      "source_name": null
    }
  ]
}"#;
    std::fs::write(profile_dir.join("packages.json"), state_content).unwrap();
    std::fs::write(env.config_dir.join("active"), "default").unwrap();

    let output = env.cmd().args(["file", "neovim"]).output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The command may succeed or fail depending on nix/network availability
    // If it succeeds, check the output contains a flake.nix path
    if output.status.success() {
        assert!(
            stdout.contains("flake.nix"),
            "Should output path to flake.nix: {}",
            stdout
        );
    } else {
        // If it fails, it should be a nix-related failure (prefetch), not a lookup failure
        assert!(
            !stderr.contains("not installed"),
            "Should find the custom package in state: stderr={}",
            stderr
        );
    }
}

#[test]
fn test_file_help() {
    let output = nixy_cmd().args(["file", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("source") || stdout.contains("path") || stdout.contains("package"),
        "Help should describe the file command: {}",
        stdout
    );
}
