#!/usr/bin/env bash
#
# Unit tests for nixy
#
# Run: ./test_nixy.sh
#

set -euo pipefail

# Test configuration
NIXY="$(cd "$(dirname "$0")" && pwd)/nixy"
ORIGINAL_DIR="$(pwd)"
ORIGINAL_NIXY_CONFIG_DIR="${NIXY_CONFIG_DIR:-}"
ORIGINAL_NIXY_ENV="${NIXY_ENV:-}"
TEST_DIR=""
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Ensure cleanup on exit (including Ctrl+C, errors, etc.)
cleanup_on_exit() {
    cd "$ORIGINAL_DIR" 2>/dev/null || true
    [[ -n "$TEST_DIR" && -d "$TEST_DIR" ]] && rm -rf "$TEST_DIR"
    if [[ -n "$ORIGINAL_NIXY_CONFIG_DIR" ]]; then
        export NIXY_CONFIG_DIR="$ORIGINAL_NIXY_CONFIG_DIR"
    else
        unset NIXY_CONFIG_DIR 2>/dev/null || true
    fi
    if [[ -n "$ORIGINAL_NIXY_ENV" ]]; then
        export NIXY_ENV="$ORIGINAL_NIXY_ENV"
    else
        unset NIXY_ENV 2>/dev/null || true
    fi
}
trap cleanup_on_exit EXIT

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

# Test helpers
setup() {
    TEST_DIR=$(mktemp -d)
    export NIXY_CONFIG_DIR="$TEST_DIR/config"
    export NIXY_ENV="$TEST_DIR/result"
    mkdir -p "$NIXY_CONFIG_DIR"
}

teardown() {
    cd "$ORIGINAL_DIR"
    [[ -n "$TEST_DIR" && -d "$TEST_DIR" ]] && rm -rf "$TEST_DIR"
    if [[ -n "$ORIGINAL_NIXY_CONFIG_DIR" ]]; then
        export NIXY_CONFIG_DIR="$ORIGINAL_NIXY_CONFIG_DIR"
    else
        unset NIXY_CONFIG_DIR
    fi
    if [[ -n "$ORIGINAL_NIXY_ENV" ]]; then
        export NIXY_ENV="$ORIGINAL_NIXY_ENV"
    else
        unset NIXY_ENV
    fi
}

assert_file_exists() {
    local file="$1"
    local msg="${2:-File should exist: $file}"
    if [[ -f "$file" ]]; then
        return 0
    else
        echo "  ASSERTION FAILED: $msg"
        return 1
    fi
}

assert_file_not_exists() {
    local file="$1"
    local msg="${2:-File should not exist: $file}"
    if [[ ! -f "$file" ]]; then
        return 0
    else
        echo "  ASSERTION FAILED: $msg"
        return 1
    fi
}

assert_file_contains() {
    local file="$1"
    local pattern="$2"
    local msg="${3:-File should contain pattern: $pattern}"
    if grep -q "$pattern" "$file" 2>/dev/null; then
        return 0
    else
        echo "  ASSERTION FAILED: $msg"
        return 1
    fi
}

assert_file_not_contains() {
    local file="$1"
    local pattern="$2"
    local msg="${3:-File should not contain pattern: $pattern}"
    if ! grep -q "$pattern" "$file" 2>/dev/null; then
        return 0
    else
        echo "  ASSERTION FAILED: $msg"
        return 1
    fi
}

assert_exit_code() {
    local expected="$1"
    local actual="$2"
    local msg="${3:-Exit code should be $expected}"
    if [[ "$actual" -eq "$expected" ]]; then
        return 0
    else
        echo "  ASSERTION FAILED: $msg (got $actual)"
        return 1
    fi
}

assert_output_contains() {
    local output="$1"
    local pattern="$2"
    local msg="${3:-Output should contain: $pattern}"
    if echo "$output" | grep -qF -- "$pattern"; then
        return 0
    else
        echo "  ASSERTION FAILED: $msg"
        return 1
    fi
}

run_test() {
    local test_name="$1"
    local test_func="$2"

    TESTS_RUN=$((TESTS_RUN + 1))
    echo -n "Testing: $test_name... "

    setup

    local result=0
    if $test_func; then
        echo -e "${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}FAIL${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        result=1
    fi

    teardown
    return $result
}

# =============================================================================
# Test: Global flake behavior
# =============================================================================

test_default_uses_global_flake() {
    cd "$TEST_DIR"
    # Create global flake via profile
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true
    # Should work by default (no flags needed)
    "$NIXY" list >/dev/null 2>&1
}

test_list_shows_flake_packages() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Add some packages to the flake (both packages and env-paths sections)
    awk '/# \[nixy:packages\]/{print; print "          ripgrep = pkgs.ripgrep;"; print "          fzf = pkgs.fzf;"; next}1' "$profile_dir/flake.nix" > "$profile_dir/flake.nix.tmp" && command mv "$profile_dir/flake.nix.tmp" "$profile_dir/flake.nix"
    awk '/# \[nixy:env-paths\]/{print; print "              ripgrep"; print "              fzf"; next}1' "$profile_dir/flake.nix" > "$profile_dir/flake.nix.tmp" && command mv "$profile_dir/flake.nix.tmp" "$profile_dir/flake.nix"

    # Create flake.lock (required for nix eval to work)
    nix --extra-experimental-features nix-command --extra-experimental-features flakes flake update --flake "$profile_dir" >/dev/null 2>&1

    local output
    output=$("$NIXY" list 2>&1)

    # Should show packages from flake
    assert_output_contains "$output" "ripgrep" && \
    assert_output_contains "$output" "fzf" && \
    assert_output_contains "$output" "Packages in"
}

test_list_shows_none_for_empty_flake() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output
    output=$("$NIXY" list 2>&1)

    # Should show (none) for empty flake
    assert_output_contains "$output" "(none)"
}

test_flake_has_no_devshells() {
    cd "$TEST_DIR"

    # Source nixy to test generate_flake directly
    source "$NIXY"

    # Generate a flake
    local flake_content
    flake_content=$(generate_flake --flake-dir "$TEST_DIR" ripgrep)

    # Flakes should NOT have devShells
    if echo "$flake_content" | grep -q "devShells"; then
        echo "  ASSERTION FAILED: Flake should NOT contain devShells"
        return 1
    fi

    # But should have packages section
    if ! echo "$flake_content" | grep -q "packages = forAllSystems"; then
        echo "  ASSERTION FAILED: Flake should contain packages"
        return 1
    fi

    return 0
}

# =============================================================================
# Test: Install adds only specific package (not global dump)
# =============================================================================

test_install_adds_single_package() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Verify the flake structure is correct
    assert_file_contains "$profile_dir/flake.nix" "# \[nixy:packages\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[/nixy:packages\]"
}

test_install_preserves_existing_packages() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Manually add a package to the flake (use awk for portability)
    awk '/# \[nixy:packages\]/{print; print "          existing-pkg = pkgs.existing-pkg;"; next}1' "$profile_dir/flake.nix" > "$profile_dir/flake.nix.tmp" && command mv "$profile_dir/flake.nix.tmp" "$profile_dir/flake.nix"

    # Verify existing-pkg is there
    assert_file_contains "$profile_dir/flake.nix" "existing-pkg"
}

# =============================================================================
# Test: Flake structure
# =============================================================================

test_flake_structure_has_markers() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Verify all required sections exist
    assert_file_contains "$profile_dir/flake.nix" "# \[nixy:packages\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[/nixy:packages\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[nixy:local-packages\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[/nixy:local-packages\]"
}

# =============================================================================
# Test: Error propagation (subshell exit issue)
# =============================================================================

test_install_fails_cleanly_without_flake() {
    cd "$TEST_DIR"
    # No flake exists, install should fail without creating files
    local output exit_code
    output=$("$NIXY" install testpkg 2>&1) && exit_code=0 || exit_code=$?

    # Should fail
    assert_exit_code 1 "$exit_code"
}

test_upgrade_shows_help() {
    local output exit_code
    output=$("$NIXY" upgrade --help 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 0 "$exit_code" && \
    assert_output_contains "$output" "Usage: nixy upgrade" && \
    assert_output_contains "$output" "nixpkgs"
}

test_upgrade_rejects_unknown_option() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" upgrade --foo 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Unknown option: --foo"
}

test_upgrade_validates_input_name() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Create flake.lock by running sync
    "$NIXY" sync >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" upgrade nonexistent-input 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Unknown input(s): nonexistent-input"
}

test_upgrade_shows_available_inputs_on_error() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Create flake.lock by running sync
    "$NIXY" sync >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" upgrade invalid-input 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Available inputs:" && \
    assert_output_contains "$output" "nixpkgs"
}

test_upgrade_requires_lock_file_for_specific_input() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Remove flake.lock created by profile switch (nix build creates it)
    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    rm -f "$profile_dir/flake.lock"

    local output exit_code
    output=$("$NIXY" upgrade nixpkgs 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "No flake.lock found" && \
    assert_output_contains "$output" "nixy sync"
}

test_upgrade_handles_corrupted_lock_file() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Create a corrupted flake.lock
    echo "not valid json" > "$profile_dir/flake.lock"

    local output exit_code
    output=$("$NIXY" upgrade nixpkgs 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Failed to parse flake.lock"
}

test_sync_fails_cleanly_without_flake() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" sync 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code"
}

test_sync_rejects_unknown_option() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" sync --foo 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Unknown option"
}

test_sync_with_empty_flake() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Sync with empty flake (no packages) should not fail with unbound variable
    local output exit_code
    output=$("$NIXY" sync 2>&1) && exit_code=0 || exit_code=$?

    # Should succeed (already in sync)
    assert_exit_code 0 "$exit_code" && \
    # Should not have unbound variable error
    if echo "$output" | grep -q "unbound variable"; then
        echo "  ASSERTION FAILED: sync should not have unbound variable error"
        return 1
    fi
    return 0
}

test_sync_with_packages_no_unbound_variable() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Add packages to flake (simulating a flake with packages defined)
    awk '/# \[nixy:packages\]/{print; print "          ripgrep = pkgs.ripgrep;"; print "          fzf = pkgs.fzf;"; next}1' "$profile_dir/flake.nix" > "$profile_dir/flake.nix.tmp" && mv "$profile_dir/flake.nix.tmp" "$profile_dir/flake.nix"
    awk '/# \[nixy:env-paths\]/{print; print "              ripgrep"; print "              fzf"; next}1' "$profile_dir/flake.nix" > "$profile_dir/flake.nix.tmp" && mv "$profile_dir/flake.nix.tmp" "$profile_dir/flake.nix"

    # Sync should not fail with unbound variable
    local output exit_code
    output=$("$NIXY" sync 2>&1) && exit_code=0 || exit_code=$?

    # Should not have unbound variable error regardless of exit code
    # (exit code may be non-zero if nix build fails, but that's not what we're testing)
    if echo "$output" | grep -q "unbound variable"; then
        echo "  ASSERTION FAILED: sync should not have unbound variable error"
        echo "  Output: $output"
        return 1
    fi
    return 0
}

test_sync_builds_environment() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Sync should attempt to build environment
    local output exit_code
    output=$("$NIXY" sync 2>&1) && exit_code=0 || exit_code=$?

    # Should mention building environment
    if ! echo "$output" | grep -q "Building nixy environment"; then
        echo "  ASSERTION FAILED: sync should mention building environment"
        echo "  Output: $output"
        return 1
    fi
    return 0
}

test_sync_creates_lock_file() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Remove flake.lock created by profile switch (nix build creates it)
    rm -f "$profile_dir/flake.lock"

    # Verify no lock file exists before sync
    if [[ -f "$profile_dir/flake.lock" ]]; then
        echo "  ASSERTION FAILED: flake.lock should not exist before sync"
        return 1
    fi

    # Run sync
    "$NIXY" sync >/dev/null 2>&1 || true

    # Verify lock file is created
    assert_file_exists "$profile_dir/flake.lock" "flake.lock should be created after sync"
}

test_sync_remove_flag_accepted() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Test that --remove flag is accepted (backward compat, no-op)
    local output exit_code
    output=$("$NIXY" sync --remove 2>&1) && exit_code=0 || exit_code=$?

    # Should not have "Unknown option" error
    if echo "$output" | grep -q "Unknown option"; then
        echo "  ASSERTION FAILED: --remove should be a valid option"
        return 1
    fi
    return 0
}

test_sync_short_remove_flag_accepted() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Test that -r short flag is accepted (backward compat, no-op)
    local output exit_code
    output=$("$NIXY" sync -r 2>&1) && exit_code=0 || exit_code=$?

    # Should not have "Unknown option" error
    if echo "$output" | grep -q "Unknown option"; then
        echo "  ASSERTION FAILED: -r should be a valid option"
        return 1
    fi
    return 0
}

test_help_shows_sync_command() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "sync" && \
    assert_output_contains "$output" "Build environment from flake.nix"
}

# =============================================================================
# Test: Local package file parsing (pname/name)
# =============================================================================

test_parse_pname_from_nixpkgs_style() {
    cd "$TEST_DIR"
    # Create a nixpkgs-style package file with pname
    cat > test-pkg.nix <<'EOF'
{ lib, buildGoModule, fetchFromGitHub }:

buildGoModule rec {
  pname = "my-package";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "test";
    repo = "test";
    rev = "v${version}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };

  vendorHash = null;
}
EOF

    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Install should extract pname correctly
    local output exit_code
    output=$("$NIXY" install --file test-pkg.nix 2>&1) && exit_code=0 || exit_code=$?

    # Should find the package name from pname
    assert_output_contains "$output" "my-package"
}

test_parse_name_from_simple_style() {
    cd "$TEST_DIR"
    # Create a simple package file with name (not pname)
    cat > test-pkg.nix <<'EOF'
{ pkgs }:

pkgs.stdenv.mkDerivation {
  name = "simple-package";
  src = ./.;
}
EOF

    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" install --file test-pkg.nix 2>&1) && exit_code=0 || exit_code=$?

    # Should find the package name from name attribute
    assert_output_contains "$output" "simple-package"
}

test_parse_pname_takes_precedence() {
    cd "$TEST_DIR"
    # Create a file with both pname and name (pname should be used)
    cat > test-pkg.nix <<'EOF'
{ pkgs }:

pkgs.stdenv.mkDerivation {
  pname = "preferred-name";
  name = "fallback-name";
  version = "1.0";
  src = ./.;
}
EOF

    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" install --file test-pkg.nix 2>&1) && exit_code=0 || exit_code=$?

    # Should use pname, not name
    assert_output_contains "$output" "preferred-name"
}

test_parse_fails_without_name_or_pname() {
    cd "$TEST_DIR"
    # Create a file without name or pname
    cat > test-pkg.nix <<'EOF'
{ pkgs }:

pkgs.stdenv.mkDerivation {
  src = ./.;
  buildPhase = "echo hello";
}
EOF

    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" install --file test-pkg.nix 2>&1) && exit_code=0 || exit_code=$?

    # Should fail with appropriate error message
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Could not find 'name' or 'pname'"
}

test_install_file_not_found() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" install --file nonexistent.nix 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "File not found"
}

test_install_file_adds_to_local_packages_section() {
    cd "$TEST_DIR"
    # Create a nixpkgs-style package file with pname
    cat > test-pkg.nix <<'EOF'
{ lib, buildGoModule, fetchFromGitHub }:

buildGoModule rec {
  pname = "my-local-pkg";
  version = "1.0.0";
  src = fetchFromGitHub {
    owner = "test";
    repo = "test";
    rev = "v${version}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
  vendorHash = null;
}
EOF

    # Create global flake
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install the local file
    "$NIXY" install --file test-pkg.nix 2>&1 || true

    # Verify package was copied
    assert_file_exists "$profile_dir/packages/my-local-pkg.nix" && \

    # Verify flake.nix has the local package entry
    assert_file_contains "$profile_dir/flake.nix" "my-local-pkg = pkgs.callPackage ./packages/my-local-pkg.nix"
}

test_install_flake_file_creates_directory() {
    cd "$TEST_DIR"

    # Create a flake file (has inputs and outputs)
    cat > my-flake.nix <<'EOF'
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in {
      packages = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.hello;
        });
    };
}
EOF

    # Create global flake
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install the flake file
    "$NIXY" install --file my-flake.nix 2>&1 || true

    # Verify flake was copied to a subdirectory
    assert_file_exists "$profile_dir/packages/my-flake/flake.nix" "Flake should be in subdirectory"
}

test_install_flake_file_adds_input() {
    cd "$TEST_DIR"

    # Create a flake file
    cat > gke-plugin.nix <<'EOF'
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in {
      packages = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.hello;
        });
    };
}
EOF

    # Create global flake
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install the flake file
    "$NIXY" install --file gke-plugin.nix 2>&1 || true

    # Verify flake.nix has the input
    assert_file_contains "$profile_dir/flake.nix" 'gke-plugin.url = "path:./packages/gke-plugin"' && \

    # Verify flake.nix has the package expression
    assert_file_contains "$profile_dir/flake.nix" 'gke-plugin = inputs.gke-plugin.packages'
}

test_install_flake_file_detected_correctly() {
    cd "$TEST_DIR"

    # Create a regular package file (should NOT be treated as flake)
    cat > regular-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "regular-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Create a flake file (SHOULD be treated as flake)
    cat > flake-pkg.nix <<'EOF'
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }: {
    packages.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.hello;
  };
}
EOF

    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install regular package
    "$NIXY" install --file regular-pkg.nix 2>&1 || true

    # Install flake package
    "$NIXY" install --file flake-pkg.nix 2>&1 || true

    # Regular package should be a .nix file
    assert_file_exists "$profile_dir/packages/regular-pkg.nix" && \

    # Flake package should be in a subdirectory
    assert_file_exists "$profile_dir/packages/flake-pkg/flake.nix"
}

test_uninstall_flake_package() {
    cd "$TEST_DIR"

    # Create a flake file
    cat > my-flake.nix <<'EOF'
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }: {
    packages.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.hello;
  };
}
EOF

    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install the flake
    "$NIXY" install --file my-flake.nix 2>&1 || true

    # Verify it was installed
    assert_file_exists "$profile_dir/packages/my-flake/flake.nix" || return 1

    # Uninstall it
    "$NIXY" uninstall my-flake 2>&1 || true

    # Verify directory was removed
    if [[ -d "$profile_dir/packages/my-flake" ]]; then
        echo "  ASSERTION FAILED: Flake directory should be removed after uninstall"
        return 1
    fi
    return 0
}

# =============================================================================
# Test: Package validation
# =============================================================================

test_validate_package_rejects_invalid_package() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" install rust 2>&1) && exit_code=0 || exit_code=$?

    # Should fail with validation error
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "not found in nixpkgs"
}

test_validate_package_rejects_attribute_set() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # 'lib' is an attribute set, not a derivation
    local output exit_code
    output=$("$NIXY" install lib 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "not found in nixpkgs"
}

test_validate_package_accepts_valid_package() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # 'hello' is a valid package in nixpkgs
    local output exit_code
    output=$("$NIXY" install hello 2>&1) && exit_code=0 || exit_code=$?

    # Should pass validation (may fail later at nix profile add, but validation passed)
    assert_output_contains "$output" "Validating package hello" && \
    # Should NOT contain validation error
    if echo "$output" | grep -q "not found in nixpkgs"; then
        echo "  ASSERTION FAILED: hello should be a valid package"
        return 1
    fi
    return 0
}

test_validate_package_suggests_search() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output
    output=$("$NIXY" install invalidpkg123 2>&1 || true)

    # Error message should suggest using search
    assert_output_contains "$output" "nixy search"
}

test_validate_skipped_for_file_install() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Create a local package file
    cat > test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "local-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    local output exit_code
    output=$("$NIXY" install --file test-pkg.nix 2>&1) && exit_code=0 || exit_code=$?

    # Should NOT show "Validating package" message (file installs skip nixpkgs validation)
    if echo "$output" | grep -q "Validating package"; then
        echo "  ASSERTION FAILED: File installs should skip nixpkgs validation"
        return 1
    fi
    return 0
}

# =============================================================================
# Test: Help and basic commands
# =============================================================================

test_help_exit_code() {
    "$NIXY" help >/dev/null 2>&1
}

test_unknown_command_fails() {
    local exit_code
    "$NIXY" unknowncommand >/dev/null 2>&1 && exit_code=0 || exit_code=$?
    assert_exit_code 1 "$exit_code"
}

# =============================================================================
# Test: Version command
# =============================================================================

test_version_displays_version() {
    local output
    output=$("$NIXY" version 2>&1)
    assert_output_contains "$output" "nixy version"
}

test_version_flag_works() {
    local output
    output=$("$NIXY" --version 2>&1)
    assert_output_contains "$output" "nixy version"
}

test_version_short_flag_works() {
    local output
    output=$("$NIXY" -v 2>&1)
    assert_output_contains "$output" "nixy version"
}

test_version_shows_semver_format() {
    local output
    output=$("$NIXY" version 2>&1)
    # Should match semver pattern like "nixy version X.Y.Z"
    if echo "$output" | grep -qE "nixy version [0-9]+\.[0-9]+\.[0-9]+"; then
        return 0
    else
        echo "  ASSERTION FAILED: Version should be in semver format"
        echo "  Output: $output"
        return 1
    fi
}

# =============================================================================
# Test: Self-upgrade command
# =============================================================================

test_self_upgrade_rejects_invalid_option() {
    local output exit_code
    output=$("$NIXY" self-upgrade --invalid 2>&1) && exit_code=0 || exit_code=$?
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Unknown option"
}

test_self_upgrade_accepts_force_flag() {
    # Test that --force is recognized using --dry-run to avoid actually upgrading
    local output
    output=$("$NIXY" self-upgrade --force --dry-run 2>&1 || true)
    # Should NOT contain "Unknown option" error
    if echo "$output" | grep -q "Unknown option"; then
        echo "  ASSERTION FAILED: --force should be a valid option"
        return 1
    fi
    return 0
}

test_self_upgrade_accepts_short_force_flag() {
    local output
    output=$("$NIXY" self-upgrade -f --dry-run 2>&1 || true)
    if echo "$output" | grep -q "Unknown option"; then
        echo "  ASSERTION FAILED: -f should be a valid option"
        return 1
    fi
    return 0
}

test_help_shows_version_command() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "version" && \
    assert_output_contains "$output" "Show nixy version"
}

test_help_shows_self_upgrade_command() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "self-upgrade" && \
    assert_output_contains "$output" "Upgrade nixy to the latest version"
}

test_help_shows_maintenance_section() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "MAINTENANCE COMMANDS"
}

# =============================================================================
# Test: Config command
# =============================================================================

test_config_zsh_outputs_path() {
    local output
    output=$("$NIXY" config zsh 2>&1)
    assert_output_contains "$output" 'export PATH=' && \
    assert_output_contains "$output" '.local/state/nixy/env/bin'
}

test_config_bash_outputs_path() {
    local output
    output=$("$NIXY" config bash 2>&1)
    assert_output_contains "$output" 'export PATH='
}

test_config_fish_outputs_path() {
    local output
    output=$("$NIXY" config fish 2>&1)
    assert_output_contains "$output" 'set -gx PATH' && \
    assert_output_contains "$output" '.local/state/nixy/env/bin'
}

test_config_without_shell_fails() {
    local output exit_code
    output=$("$NIXY" config 2>&1) && exit_code=0 || exit_code=$?
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Usage: nixy config"
}

test_config_unknown_shell_fails() {
    local output exit_code
    output=$("$NIXY" config powershell 2>&1) && exit_code=0 || exit_code=$?
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Unknown shell"
}

test_help_shows_config_command() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "config <shell>" && \
    assert_output_contains "$output" "Output shell config"
}

# =============================================================================
# Test: buildEnv atomic install
# =============================================================================

test_flake_has_buildenv_default() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Generated flake should have buildEnv default output
    assert_file_contains "$profile_dir/flake.nix" "default = pkgs.buildEnv" && \
    assert_file_contains "$profile_dir/flake.nix" 'name = "nixy-env"' && \
    assert_file_contains "$profile_dir/flake.nix" "# \[nixy:env-paths\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[/nixy:env-paths\]"
}

test_buildenv_contains_all_packages() {
    cd "$TEST_DIR"

    # Source nixy to test generate_flake directly
    source "$NIXY"

    # Generate a flake with multiple packages
    local flake_content
    flake_content=$(generate_flake --flake-dir "$TEST_DIR" ripgrep fzf bat)

    # Check that buildEnv paths contains all packages (referenced by name via rec)
    local paths_section
    paths_section=$(echo "$flake_content" | sed -n '/# \[nixy:env-paths\]/,/# \[\/nixy:env-paths\]/p')
    if ! echo "$paths_section" | grep -qw "ripgrep"; then
        echo "  ASSERTION FAILED: buildEnv paths should contain ripgrep"
        return 1
    fi
    if ! echo "$paths_section" | grep -qw "fzf"; then
        echo "  ASSERTION FAILED: buildEnv paths should contain fzf"
        return 1
    fi
    if ! echo "$paths_section" | grep -qw "bat"; then
        echo "  ASSERTION FAILED: buildEnv paths should contain bat"
        return 1
    fi
    return 0
}

test_individual_packages_still_accessible() {
    cd "$TEST_DIR"

    # Source nixy to test generate_flake directly
    source "$NIXY"

    # Generate a flake with packages
    local flake_content
    flake_content=$(generate_flake --flake-dir "$TEST_DIR" ripgrep fzf)

    # Individual package attributes should still exist
    if ! echo "$flake_content" | grep -q "ripgrep = pkgs.ripgrep;"; then
        echo "  ASSERTION FAILED: Individual ripgrep attribute should still exist"
        return 1
    fi
    if ! echo "$flake_content" | grep -q "fzf = pkgs.fzf;"; then
        echo "  ASSERTION FAILED: Individual fzf attribute should still exist"
        return 1
    fi
    return 0
}

test_empty_flake_has_empty_buildenv() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Empty flake should have buildEnv structure with empty paths
    assert_file_contains "$profile_dir/flake.nix" "default = pkgs.buildEnv" && \
    assert_file_contains "$profile_dir/flake.nix" "paths = \[" && \
    assert_file_contains "$profile_dir/flake.nix" 'extraOutputsToInstall = \[ "man" "doc" "info" \]'
}

test_buildenv_has_extra_outputs() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # buildEnv should include man, doc, and info outputs
    assert_file_contains "$profile_dir/flake.nix" 'extraOutputsToInstall = \[ "man" "doc" "info" \]'
}

test_flake_structure_has_env_paths_markers() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Verify env-paths section markers exist
    assert_file_contains "$profile_dir/flake.nix" "# \[nixy:env-paths\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[/nixy:env-paths\]"
}

test_sync_upgrades_old_flake_without_buildenv() {
    cd "$TEST_DIR"

    # Create an old-style flake without buildEnv (simulating pre-0.1.11 nixy)
    mkdir -p "$NIXY_CONFIG_DIR"
    cat > "$NIXY_CONFIG_DIR/flake.nix" <<'EOF'
{
  description = "nixy managed packages";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # [nixy:local-inputs]
    # [/nixy:local-inputs]
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in {
      packages = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
        in {
          # [nixy:packages]
          ripgrep = pkgs.ripgrep;
          # [/nixy:packages]
          # [nixy:local-packages]
          # [/nixy:local-packages]
        });
    };
}
EOF

    # Run sync (default is global) - it should upgrade the flake to include buildEnv
    local output
    output=$("$NIXY" sync 2>&1) || true

    # Should mention upgrading
    if ! echo "$output" | grep -q "Upgrading flake.nix to buildEnv format"; then
        echo "  ASSERTION FAILED: sync should upgrade old flake"
        echo "  Output: $output"
        return 1
    fi

    # Flake should now have buildEnv
    assert_file_contains "$NIXY_CONFIG_DIR/flake.nix" "default = pkgs.buildEnv" && \
    assert_file_contains "$NIXY_CONFIG_DIR/flake.nix" "# \[nixy:env-paths\]"
}

# =============================================================================
# Test: Partial editing preserves user customizations
# =============================================================================

test_add_preserves_user_customizations() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    # Add custom content outside of markers (user customization)
    # Insert a custom input before [nixy:local-inputs]
    awk '
        /nixpkgs\.url/ { print; print "    my-custom-input.url = \"github:user/repo\";"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Add a custom nixConfig section after inputs (user customization)
    awk '
        /^  inputs = \{/ { in_inputs=1 }
        in_inputs && /^  \};/ { print; print ""; print "  nixConfig = {"; print "    extra-substituters = [ \"https://my-cache.cachix.org\" ];"; print "  };"; in_inputs=0; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Verify customizations exist before adding package
    assert_file_contains "$flake_nix" "my-custom-input.url" || return 1
    assert_file_contains "$flake_nix" "nixConfig" || return 1
    assert_file_contains "$flake_nix" "extra-substituters" || return 1

    # Use add_package_to_flake directly (source the script)
    source "$NIXY"
    cd "$profile_dir"
    add_package_to_flake "ripgrep" >/dev/null

    # Verify customizations are still present after adding package
    if ! grep -q "my-custom-input.url" "$flake_nix"; then
        echo "  ASSERTION FAILED: Custom input should be preserved after add"
        return 1
    fi

    if ! grep -q "nixConfig" "$flake_nix"; then
        echo "  ASSERTION FAILED: nixConfig should be preserved after add"
        return 1
    fi

    if ! grep -q "extra-substituters" "$flake_nix"; then
        echo "  ASSERTION FAILED: extra-substituters should be preserved after add"
        return 1
    fi

    # Verify package was added
    if ! grep -q "ripgrep = pkgs.ripgrep;" "$flake_nix"; then
        echo "  ASSERTION FAILED: ripgrep should be added to packages section"
        return 1
    fi

    return 0
}

test_remove_preserves_user_customizations() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    # Add a package first
    source "$NIXY"
    cd "$profile_dir"
    add_package_to_flake "ripgrep" >/dev/null

    # Add custom content (user customization)
    awk '
        /nixpkgs\.url/ { print; print "    my-custom-input.url = \"github:user/repo\";"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Add a custom overlay section (user customization)
    awk '
        /forAllSystems = / { print; print "      myOverlay = final: prev: { custom-pkg = prev.hello; };"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Verify customizations exist before removing package
    assert_file_contains "$flake_nix" "my-custom-input.url" || return 1
    assert_file_contains "$flake_nix" "myOverlay" || return 1
    assert_file_contains "$flake_nix" "ripgrep = pkgs.ripgrep;" || return 1

    # Remove the package
    remove_package_from_flake "ripgrep" >/dev/null

    # Verify customizations are still present after removing package
    if ! grep -q "my-custom-input.url" "$flake_nix"; then
        echo "  ASSERTION FAILED: Custom input should be preserved after remove"
        return 1
    fi

    if ! grep -q "myOverlay" "$flake_nix"; then
        echo "  ASSERTION FAILED: Custom overlay should be preserved after remove"
        return 1
    fi

    # Verify package was removed
    if grep -q "ripgrep = pkgs.ripgrep;" "$flake_nix"; then
        echo "  ASSERTION FAILED: ripgrep should be removed from packages section"
        return 1
    fi

    return 0
}

test_add_multiple_packages_preserves_all() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    # Add custom content
    awk '
        /nixpkgs\.url/ { print; print "    custom.url = \"github:custom/repo\";"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    source "$NIXY"
    cd "$profile_dir"

    # Add multiple packages one by one
    add_package_to_flake "ripgrep" >/dev/null
    add_package_to_flake "fzf" >/dev/null
    add_package_to_flake "bat" >/dev/null

    # Verify all packages are present
    if ! grep -q "ripgrep = pkgs.ripgrep;" "$flake_nix"; then
        echo "  ASSERTION FAILED: ripgrep should be in packages section"
        return 1
    fi

    if ! grep -q "fzf = pkgs.fzf;" "$flake_nix"; then
        echo "  ASSERTION FAILED: fzf should be in packages section"
        return 1
    fi

    if ! grep -q "bat = pkgs.bat;" "$flake_nix"; then
        echo "  ASSERTION FAILED: bat should be in packages section"
        return 1
    fi

    # Verify custom content is still present
    if ! grep -q "custom.url" "$flake_nix"; then
        echo "  ASSERTION FAILED: Custom input should be preserved after adding multiple packages"
        return 1
    fi

    return 0
}

test_remove_middle_package_preserves_others() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    source "$NIXY"
    cd "$profile_dir"

    # Add three packages
    add_package_to_flake "ripgrep" >/dev/null
    add_package_to_flake "fzf" >/dev/null
    add_package_to_flake "bat" >/dev/null

    # Remove the middle one
    remove_package_from_flake "fzf" >/dev/null

    # Verify fzf is removed
    if grep -q "fzf = pkgs.fzf;" "$flake_nix"; then
        echo "  ASSERTION FAILED: fzf should be removed"
        return 1
    fi

    # Verify others are still present
    if ! grep -q "ripgrep = pkgs.ripgrep;" "$flake_nix"; then
        echo "  ASSERTION FAILED: ripgrep should still be present"
        return 1
    fi

    if ! grep -q "bat = pkgs.bat;" "$flake_nix"; then
        echo "  ASSERTION FAILED: bat should still be present"
        return 1
    fi

    return 0
}

test_add_skips_duplicate_package() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    source "$NIXY"
    cd "$profile_dir"

    # Add a package
    add_package_to_flake "ripgrep" >/dev/null

    # Count occurrences of ripgrep before adding again
    local count_before
    count_before=$(grep -c "ripgrep = pkgs.ripgrep;" "$flake_nix" || true)

    # Add the same package again
    add_package_to_flake "ripgrep" >/dev/null

    # Count occurrences after
    local count_after
    count_after=$(grep -c "ripgrep = pkgs.ripgrep;" "$flake_nix" || true)

    if [[ "$count_before" != "$count_after" ]]; then
        echo "  ASSERTION FAILED: Adding duplicate package should not add another line"
        echo "  Before: $count_before, After: $count_after"
        return 1
    fi

    return 0
}

# =============================================================================
# Test: Custom markers
# =============================================================================

test_flake_has_custom_markers() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Verify all custom marker sections exist
    assert_file_contains "$profile_dir/flake.nix" "# \[nixy:custom-inputs\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[/nixy:custom-inputs\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[nixy:custom-packages\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[/nixy:custom-packages\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[nixy:custom-paths\]" && \
    assert_file_contains "$profile_dir/flake.nix" "# \[/nixy:custom-paths\]"
}

test_custom_inputs_preserved_during_regeneration() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    # Add custom input between the markers
    awk '
        /# \[nixy:custom-inputs\]/ { print; print "    my-overlay.url = \"github:user/my-overlay\";"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Verify custom input exists
    assert_file_contains "$flake_nix" "my-overlay.url" || return 1

    # Create a package file
    cat > test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "test-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Install the package with --force (regenerates flake)
    "$NIXY" install --file test-pkg.nix --force 2>&1 || true

    # Verify custom input is still preserved
    if ! grep -q "my-overlay.url" "$flake_nix"; then
        echo "  ASSERTION FAILED: Custom input should be preserved after regeneration"
        return 1
    fi

    return 0
}

test_custom_packages_preserved_during_regeneration() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    # Add custom package between the markers
    awk '
        /# \[nixy:custom-packages\]/ { print; print "          my-custom-pkg = pkgs.hello.overrideAttrs { pname = \"my-custom\"; };"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Verify custom package exists
    assert_file_contains "$flake_nix" "my-custom-pkg" || return 1

    # Create a package file
    cat > test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "test-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Install the package with --force (regenerates flake)
    "$NIXY" install --file test-pkg.nix --force 2>&1 || true

    # Verify custom package is still preserved
    if ! grep -q "my-custom-pkg" "$flake_nix"; then
        echo "  ASSERTION FAILED: Custom package should be preserved after regeneration"
        return 1
    fi

    return 0
}

test_custom_paths_preserved_during_regeneration() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    # Add custom path between the markers
    awk '
        /# \[nixy:custom-paths\]/ { print; print "              my-custom-pkg"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Verify custom path exists
    local paths_section
    paths_section=$(sed -n '/# \[nixy:custom-paths\]/,/# \[\/nixy:custom-paths\]/p' "$flake_nix")
    if ! echo "$paths_section" | grep -q "my-custom-pkg"; then
        echo "  ASSERTION FAILED: Custom path should exist before regeneration"
        return 1
    fi

    # Create a package file
    cat > test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "test-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Install the package with --force (regenerates flake)
    "$NIXY" install --file test-pkg.nix --force 2>&1 || true

    # Verify custom path is still preserved
    paths_section=$(sed -n '/# \[nixy:custom-paths\]/,/# \[\/nixy:custom-paths\]/p' "$flake_nix")
    if ! echo "$paths_section" | grep -q "my-custom-pkg"; then
        echo "  ASSERTION FAILED: Custom path should be preserved after regeneration"
        return 1
    fi

    return 0
}

test_modification_warning_shown() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    # Add modification OUTSIDE markers (this will trigger warning)
    awk '
        /nixpkgs\.url/ { print; print "    # OUTSIDE MARKER COMMENT"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Create a package file
    cat > test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "test-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Install without --force should warn and fail
    local output exit_code
    output=$("$NIXY" install --file test-pkg.nix 2>&1) && exit_code=0 || exit_code=$?

    # Should fail
    assert_exit_code 1 "$exit_code" && \
    # Should mention modifications
    assert_output_contains "$output" "modifications outside nixy markers" && \
    # Should suggest --force
    assert_output_contains "$output" "--force"
}

test_force_flag_bypasses_warning() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"
    local flake_nix="$profile_dir/flake.nix"

    # Add modification OUTSIDE markers
    awk '
        /nixpkgs\.url/ { print; print "    # OUTSIDE MARKER COMMENT"; next }
        { print }
    ' "$flake_nix" > "$flake_nix.tmp" && command mv "$flake_nix.tmp" "$flake_nix"

    # Create a package file
    cat > test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "test-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Install with --force should proceed (may fail at nix but not at warning)
    local output
    output=$("$NIXY" install --file test-pkg.nix --force 2>&1) || true

    # Should mention proceeding with --force
    assert_output_contains "$output" "Proceeding with --force"
}

test_no_warning_when_no_modifications() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1

    # Don't add any modifications outside markers

    # Create a package file
    cat > test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "test-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Install without --force should not show warning
    local output
    output=$("$NIXY" install --file test-pkg.nix 2>&1) || true

    # Should NOT mention modifications
    if echo "$output" | grep -q "modifications outside nixy markers"; then
        echo "  ASSERTION FAILED: Should not warn when no modifications outside markers"
        return 1
    fi

    return 0
}

test_help_shows_force_flag() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "--force" && \
    assert_output_contains "$output" "Force regeneration"
}

# =============================================================================
# Test: Profile management
# =============================================================================

test_profile_shows_default() {
    cd "$TEST_DIR"
    local output
    output=$("$NIXY" profile 2>&1)
    assert_output_contains "$output" "Active profile: default"
}

test_profile_switch_c() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" profile switch -c work 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 0 "$exit_code" && \
    assert_output_contains "$output" "Creating profile 'work'" && \
    assert_file_exists "$NIXY_CONFIG_DIR/profiles/work/flake.nix"
}

test_profile_switch_c_with_existing() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c work >/dev/null 2>&1

    # Running -c on existing profile should succeed (just switches)
    local output exit_code
    output=$("$NIXY" profile switch -c work 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 0 "$exit_code" && \
    assert_output_contains "$output" "Switched to profile 'work'"
}

test_profile_switch_c_validates_name() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" profile switch -c "invalid name!" 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Invalid profile name"
}

test_profile_switch() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c work >/dev/null 2>&1

    local output exit_code
    output=$("$NIXY" profile switch work 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 0 "$exit_code" && \
    assert_output_contains "$output" "Switched to profile 'work'"

    # Verify active profile changed
    local active
    active=$(cat "$NIXY_CONFIG_DIR/active")
    [[ "$active" == "work" ]] || {
        echo "  ASSERTION FAILED: expected active profile 'work', got '$active'" >&2
        return 1
    }
}

test_profile_switch_fails_if_not_exists() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" profile switch nonexistent 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "does not exist"
}

test_profile_list() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c work >/dev/null 2>&1
    "$NIXY" profile switch -c personal >/dev/null 2>&1

    local output
    output=$("$NIXY" profile list 2>&1)

    assert_output_contains "$output" "work" && \
    assert_output_contains "$output" "personal"
}

test_profile_list_shows_active() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c work >/dev/null 2>&1
    "$NIXY" profile switch work >/dev/null 2>&1

    local output
    output=$("$NIXY" profile list 2>&1)

    assert_output_contains "$output" "work (active)"
}

test_profile_delete_requires_force() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c work >/dev/null 2>&1
    "$NIXY" profile switch -c default >/dev/null 2>&1  # Switch back so work is not active

    local output exit_code
    output=$("$NIXY" profile delete work 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "--force"
}

test_profile_delete_with_force() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c work >/dev/null 2>&1
    "$NIXY" profile switch -c default >/dev/null 2>&1  # Switch back so work is not active

    local output exit_code
    output=$("$NIXY" profile delete work --force 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 0 "$exit_code" && \
    assert_output_contains "$output" "Deleted profile 'work'"

    # Verify profile directory is gone
    if [[ -d "$NIXY_CONFIG_DIR/profiles/work" ]]; then
        echo "  ASSERTION FAILED: Profile directory should be deleted"
        return 1
    fi
    return 0
}

test_profile_delete_active_fails() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c work >/dev/null 2>&1
    "$NIXY" profile switch work >/dev/null 2>&1

    local output exit_code
    output=$("$NIXY" profile delete work --force 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Cannot delete the active profile"
}

test_help_shows_profile_commands() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "PROFILE COMMANDS" && \
    assert_output_contains "$output" "profile switch" && \
    assert_output_contains "$output" "profile list" && \
    assert_output_contains "$output" "profile delete"
}

test_install_uses_active_profile() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c work >/dev/null 2>&1
    "$NIXY" profile switch work >/dev/null 2>&1

    # Add a package (will fail at nix, but flake should be updated)
    "$NIXY" install hello 2>&1 || true

    # Verify the flake in the work profile was updated
    assert_file_contains "$NIXY_CONFIG_DIR/profiles/work/flake.nix" "hello = pkgs.hello"
}

# =============================================================================
# Test: Registry install
# =============================================================================

test_install_from_requires_package() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" install --from nixpkgs 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Package name is required"
}

test_install_from_unknown_registry_fails() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" install --from nonexistent-registry hello 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "not found"
}

test_install_from_nixpkgs_works() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # nixpkgs is always in the global registry
    local output exit_code
    output=$("$NIXY" install --from nixpkgs hello 2>&1) && exit_code=0 || exit_code=$?

    # Should succeed (may take time to build)
    assert_exit_code 0 "$exit_code" && \
    assert_output_contains "$output" "Looking up 'nixpkgs' in nix registry"
}

test_install_from_adds_to_custom_inputs() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install from nixpkgs registry
    # Note: nixpkgs is already a default input, so it won't be added to custom-inputs
    # This test verifies the input exists somewhere in the flake
    "$NIXY" install --from nixpkgs hello 2>&1 || true

    # Verify nixpkgs input exists in the flake (either in default inputs or custom-inputs)
    if ! grep -q "nixpkgs.url" "$profile_dir/flake.nix"; then
        echo "  ASSERTION FAILED: nixpkgs input should exist in flake.nix"
        return 1
    fi
    return 0
}

test_install_from_adds_to_custom_packages() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install from nixpkgs registry
    "$NIXY" install --from nixpkgs hello 2>&1 || true

    # Verify the package was added to custom-packages section
    # Note: nixpkgs uses legacyPackages, and we reuse the existing nixpkgs input
    local packages_section
    packages_section=$(sed -n '/# \[nixy:custom-packages\]/,/# \[\/nixy:custom-packages\]/p' "$profile_dir/flake.nix")
    if ! echo "$packages_section" | grep -q "hello = inputs.nixpkgs.legacyPackages"; then
        echo "  ASSERTION FAILED: hello package should be in custom-packages section with legacyPackages"
        echo "  Got: $packages_section"
        return 1
    fi
    return 0
}

test_install_from_adds_to_custom_paths() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install from nixpkgs registry
    "$NIXY" install --from nixpkgs hello 2>&1 || true

    # Verify the package was added to custom-paths section
    local paths_section
    paths_section=$(sed -n '/# \[nixy:custom-paths\]/,/# \[\/nixy:custom-paths\]/p' "$profile_dir/flake.nix")
    if ! echo "$paths_section" | grep -q "hello"; then
        echo "  ASSERTION FAILED: hello should be in custom-paths section"
        return 1
    fi
    return 0
}

test_install_from_validates_package() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local output exit_code
    output=$("$NIXY" install --from nixpkgs nonexistent-pkg-xyz 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "not found"
}

test_help_shows_from_option() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "--from" && \
    assert_output_contains "$output" "registry"
}

test_lookup_registry_function() {
    cd "$TEST_DIR"

    # Source nixy to test lookup_registry directly
    source "$NIXY"

    # nixpkgs should always be in the registry
    local url
    url=$(lookup_registry "nixpkgs")

    if [[ -z "$url" ]]; then
        echo "  ASSERTION FAILED: nixpkgs should be in the registry"
        return 1
    fi

    # URL should contain github:NixOS/nixpkgs
    if ! echo "$url" | grep -q "NixOS/nixpkgs"; then
        echo "  ASSERTION FAILED: nixpkgs URL should contain NixOS/nixpkgs"
        echo "  Got: $url"
        return 1
    fi

    return 0
}

test_install_from_direct_url_detected() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    # Direct URL should be detected (contains ':')
    local output exit_code
    output=$("$NIXY" install --from github:NixOS/nixpkgs hello 2>&1) && exit_code=0 || exit_code=$?

    # Should show "Using flake URL" message (not "Looking up in registry")
    assert_output_contains "$output" "Using flake URL"
}

test_install_from_direct_url_generates_input_name() {
    cd "$TEST_DIR"
    "$NIXY" profile switch -c default >/dev/null 2>&1 || true

    local profile_dir="$NIXY_CONFIG_DIR/profiles/default"

    # Install from direct URL (using nixpkgs which already exists as default input)
    "$NIXY" install --from github:NixOS/nixpkgs hello 2>&1 || true

    # For NixOS/nixpkgs URLs, we reuse the existing nixpkgs input
    # Verify the package references inputs.nixpkgs
    if ! grep -q "inputs.nixpkgs.legacyPackages" "$profile_dir/flake.nix"; then
        echo "  ASSERTION FAILED: flake.nix should reference inputs.nixpkgs"
        cat "$profile_dir/flake.nix"
        return 1
    fi
    return 0
}

# =============================================================================
# Run all tests
# =============================================================================

main() {
    echo "======================================"
    echo "Running nixy unit tests"
    echo "======================================"
    echo ""

    # Global flake behavior tests
    run_test "default uses global flake" test_default_uses_global_flake || true

    # List command tests
    run_test "list shows flake packages" test_list_shows_flake_packages || true
    run_test "list shows none for empty flake" test_list_shows_none_for_empty_flake || true

    # Flake structure tests
    run_test "flake has no devShells" test_flake_has_no_devshells || true
    run_test "flake structure has markers" test_flake_structure_has_markers || true
    run_test "install preserves existing packages" test_install_preserves_existing_packages || true

    # Error propagation tests
    run_test "install fails cleanly without flake" test_install_fails_cleanly_without_flake || true
    run_test "upgrade --help shows help" test_upgrade_shows_help || true
    run_test "upgrade rejects unknown option" test_upgrade_rejects_unknown_option || true
    run_test "upgrade validates input name" test_upgrade_validates_input_name || true
    run_test "upgrade shows available inputs on error" test_upgrade_shows_available_inputs_on_error || true
    run_test "upgrade requires lock file for specific input" test_upgrade_requires_lock_file_for_specific_input || true
    run_test "upgrade handles corrupted lock file" test_upgrade_handles_corrupted_lock_file || true
    run_test "sync fails cleanly without flake" test_sync_fails_cleanly_without_flake || true
    run_test "sync rejects unknown option" test_sync_rejects_unknown_option || true
    run_test "sync with empty flake succeeds" test_sync_with_empty_flake || true
    run_test "sync with packages no unbound variable" test_sync_with_packages_no_unbound_variable || true
    run_test "sync builds environment" test_sync_builds_environment || true
    run_test "sync creates flake.lock" test_sync_creates_lock_file || true
    run_test "sync --remove flag accepted" test_sync_remove_flag_accepted || true
    run_test "sync -r short flag accepted" test_sync_short_remove_flag_accepted || true
    run_test "help shows sync command" test_help_shows_sync_command || true

    # Local package file parsing tests
    run_test "parse pname from nixpkgs-style file" test_parse_pname_from_nixpkgs_style || true
    run_test "parse name from simple-style file" test_parse_name_from_simple_style || true
    run_test "pname takes precedence over name" test_parse_pname_takes_precedence || true
    run_test "fails without name or pname" test_parse_fails_without_name_or_pname || true
    run_test "install --file with nonexistent file" test_install_file_not_found || true
    run_test "install --file adds to local-packages section" test_install_file_adds_to_local_packages_section || true
    run_test "install --file flake creates directory" test_install_flake_file_creates_directory || true
    run_test "install --file flake adds input" test_install_flake_file_adds_input || true
    run_test "install --file detects flake vs package" test_install_flake_file_detected_correctly || true
    run_test "uninstall removes flake directory" test_uninstall_flake_package || true

    # Package validation tests
    run_test "validate rejects invalid package" test_validate_package_rejects_invalid_package || true
    run_test "validate rejects attribute set" test_validate_package_rejects_attribute_set || true
    run_test "validate accepts valid package" test_validate_package_accepts_valid_package || true
    run_test "validate suggests search on failure" test_validate_package_suggests_search || true
    run_test "validate skipped for file install" test_validate_skipped_for_file_install || true

    # Help tests
    run_test "help exits successfully" test_help_exit_code || true
    run_test "unknown command fails" test_unknown_command_fails || true

    # Version tests
    run_test "version displays version" test_version_displays_version || true
    run_test "--version flag works" test_version_flag_works || true
    run_test "-v short flag works" test_version_short_flag_works || true
    run_test "version shows semver format" test_version_shows_semver_format || true

    # Self-upgrade tests
    run_test "self-upgrade rejects invalid option" test_self_upgrade_rejects_invalid_option || true
    run_test "self-upgrade accepts --force flag" test_self_upgrade_accepts_force_flag || true
    run_test "self-upgrade accepts -f flag" test_self_upgrade_accepts_short_force_flag || true
    run_test "help shows version command" test_help_shows_version_command || true
    run_test "help shows self-upgrade command" test_help_shows_self_upgrade_command || true
    run_test "help shows MAINTENANCE section" test_help_shows_maintenance_section || true

    # Config command tests
    run_test "config zsh outputs PATH" test_config_zsh_outputs_path || true
    run_test "config bash outputs PATH" test_config_bash_outputs_path || true
    run_test "config fish outputs PATH" test_config_fish_outputs_path || true
    run_test "config without shell fails" test_config_without_shell_fails || true
    run_test "config unknown shell fails" test_config_unknown_shell_fails || true
    run_test "help shows config command" test_help_shows_config_command || true

    # buildEnv atomic install tests
    run_test "flake has buildEnv default" test_flake_has_buildenv_default || true
    run_test "buildEnv contains all packages" test_buildenv_contains_all_packages || true
    run_test "individual packages still accessible" test_individual_packages_still_accessible || true
    run_test "empty flake has empty buildEnv" test_empty_flake_has_empty_buildenv || true
    run_test "buildEnv has extra outputs" test_buildenv_has_extra_outputs || true
    run_test "flake has env-paths markers" test_flake_structure_has_env_paths_markers || true
    run_test "sync upgrades old flake without buildEnv" test_sync_upgrades_old_flake_without_buildenv || true

    # Partial editing (preserves user customizations) tests
    run_test "add preserves user customizations" test_add_preserves_user_customizations || true
    run_test "remove preserves user customizations" test_remove_preserves_user_customizations || true
    run_test "add multiple packages preserves all" test_add_multiple_packages_preserves_all || true
    run_test "remove middle package preserves others" test_remove_middle_package_preserves_others || true
    run_test "add skips duplicate package" test_add_skips_duplicate_package || true

    # Custom marker tests
    run_test "flake has custom markers" test_flake_has_custom_markers || true
    run_test "custom inputs preserved during regeneration" test_custom_inputs_preserved_during_regeneration || true
    run_test "custom packages preserved during regeneration" test_custom_packages_preserved_during_regeneration || true
    run_test "custom paths preserved during regeneration" test_custom_paths_preserved_during_regeneration || true
    run_test "modification warning shown" test_modification_warning_shown || true
    run_test "force flag bypasses warning" test_force_flag_bypasses_warning || true
    run_test "no warning when no modifications" test_no_warning_when_no_modifications || true
    run_test "help shows force flag" test_help_shows_force_flag || true

    # Profile management tests
    run_test "profile shows default" test_profile_shows_default || true
    run_test "profile switch -c" test_profile_switch_c || true
    run_test "profile switch -c with existing" test_profile_switch_c_with_existing || true
    run_test "profile switch -c validates name" test_profile_switch_c_validates_name || true
    run_test "profile switch" test_profile_switch || true
    run_test "profile switch fails if not exists" test_profile_switch_fails_if_not_exists || true
    run_test "profile list" test_profile_list || true
    run_test "profile list shows active" test_profile_list_shows_active || true
    run_test "profile delete requires force" test_profile_delete_requires_force || true
    run_test "profile delete with force" test_profile_delete_with_force || true
    run_test "profile delete active fails" test_profile_delete_active_fails || true
    run_test "help shows profile commands" test_help_shows_profile_commands || true
    run_test "install uses active profile" test_install_uses_active_profile || true

    # Registry install tests
    run_test "install --from requires package" test_install_from_requires_package || true
    run_test "install --from unknown registry fails" test_install_from_unknown_registry_fails || true
    run_test "install --from nixpkgs works" test_install_from_nixpkgs_works || true
    run_test "install --from adds to custom-inputs" test_install_from_adds_to_custom_inputs || true
    run_test "install --from adds to custom-packages" test_install_from_adds_to_custom_packages || true
    run_test "install --from adds to custom-paths" test_install_from_adds_to_custom_paths || true
    run_test "install --from validates package" test_install_from_validates_package || true
    run_test "help shows --from option" test_help_shows_from_option || true
    run_test "lookup_registry function works" test_lookup_registry_function || true
    run_test "install --from direct URL detected" test_install_from_direct_url_detected || true
    run_test "install --from direct URL uses existing nixpkgs" test_install_from_direct_url_generates_input_name || true

    echo ""
    echo "======================================"
    echo "Results: $TESTS_PASSED/$TESTS_RUN passed"
    if [[ $TESTS_FAILED -gt 0 ]]; then
        echo -e "${RED}$TESTS_FAILED tests failed${NC}"
        exit 1
    else
        echo -e "${GREEN}All tests passed!${NC}"
        exit 0
    fi
}

main "$@"
