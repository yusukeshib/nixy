# Claude Code Instructions for nixy

## Project Overview

nixy is a Homebrew-style wrapper for Nix using flake.nix. It's a Rust CLI tool that manages Nix packages through a declarative flake.nix file.

## Development Workflow

When adding new features or fixing bugs, follow this workflow:

### 1. Create a Feature Branch with Worktree
For new features, use `git wt` to work in an isolated directory:
```bash
git wt <username>/<feature-name>
```
This creates the branch and worktree automatically, then switches to the worktree directory.
This keeps the main worktree clean and allows parallel development.

### 2. Make Changes
- Edit source files in `src/`
- Follow existing code patterns and style
- **Update documentation**: If adding new commands or options, update both `README.md` and `README_ja.md`

### 3. Add Tests
- Add integration tests to `tests/integration.rs`
- Add unit tests in the relevant source file's `#[cfg(test)]` module
- Use the `TestEnv` helper for tests that need isolated config/env directories

Integration test structure:
```rust
#[test]
fn test_feature_name() {
    let env = TestEnv::new();

    let output = env
        .cmd()
        .args(["command", "arg"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("expected text"));
}
```

Unit test structure:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        let result = function_to_test();
        assert_eq!(result, expected_value);
    }
}
```

### 4. Run Tests
```bash
cargo test
```
All tests must pass before committing.

### 5. Update Version
- Bump version in `Cargo.toml`
- Use semantic versioning (MAJOR.MINOR.PATCH)

### 6. Commit, Push, and Create PR
```bash
git add src/ tests/ Cargo.toml CLAUDE.md
git commit -m "Description of change, bump to X.Y.Z

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
git push -u origin <username>/<feature-name>
gh pr create --title "Feature description" --body "## Summary
- Change 1
- Change 2

## Test plan
- [ ] Run cargo test
"
```

### 7. Cleanup Worktree (after PR is merged)
```bash
git wt -d <username>/<feature-name>
```
This removes both the worktree and the branch.

Commit message style:
- Start with verb (Add, Fix, Update, Make, etc.)
- Brief description of the change
- Include version bump if applicable
- Keep subject line under 72 characters

## Code Style

- Use `info()`, `success()`, `warn()`, `error()` from `src/commands/mod.rs` for output
- Use clap for argument parsing (defined in `src/cli.rs`)
- Return `Result<(), Error>` from command functions
- Validate inputs early and return errors with `?` operator
- Use the `Error` enum from `src/error.rs` for error handling

## Key Files

- `src/main.rs` - CLI entry point
- `src/cli.rs` - Command-line interface definitions
- `src/commands/` - Individual command implementations
- `src/flake/` - Flake.nix parsing, editing, and template generation
- `src/profile.rs` - Profile management
- `src/nix.rs` - Nix command wrapper
- `src/config.rs` - Configuration and paths
- `src/error.rs` - Error types
- `tests/integration.rs` - Integration tests
- `install.sh` - Installation script
- `flake.nix` - Nix flake for nixy itself

## Testing Notes

- Integration tests use `TestEnv` for isolated `NIXY_CONFIG_DIR` and `NIXY_ENV`
- Tests run in parallel by default; use `TestEnv` to avoid conflicts
- Unit tests are embedded in source files with `#[cfg(test)]`
- Validation tests that need valid packages should use `hello` (always available in nixpkgs)
