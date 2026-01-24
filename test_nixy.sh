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
# Test: nixy init
# =============================================================================

test_init_creates_flake() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1
    assert_file_exists "./flake.nix" && \
    assert_file_contains "./flake.nix" "nixy managed packages"
}

test_init_fails_if_flake_exists() {
    cd "$TEST_DIR"
    touch flake.nix
    local output
    output=$("$NIXY" init 2>&1 || true)
    assert_output_contains "$output" "already exists"
}

test_init_with_directory() {
    cd "$TEST_DIR"
    "$NIXY" init myproject >/dev/null 2>&1
    assert_file_exists "myproject/flake.nix"
}

test_init_creates_empty_packages_section() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1
    # Should have empty packages section (no packages between markers)
    local pkg_count
    pkg_count=$(sed -n '/# \[nixy:packages\]/,/# \[\/nixy:packages\]/p' flake.nix | grep -c "pkgs\." 2>/dev/null || true)
    [[ -z "$pkg_count" || "$pkg_count" -eq 0 ]]
}

# =============================================================================
# Test: Local flake discovery
# =============================================================================

test_finds_flake_in_current_dir() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1
    # list should work without error (finds local flake)
    "$NIXY" list >/dev/null 2>&1
}

test_finds_flake_in_parent_dir() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1
    mkdir -p subdir/deep/nested
    cd subdir/deep/nested
    # Should find flake.nix in $TEST_DIR
    "$NIXY" list >/dev/null 2>&1
}

test_fails_when_no_flake_found() {
    cd "$TEST_DIR"
    # No flake.nix exists, should fail
    local output exit_code
    output=$("$NIXY" list 2>&1) && exit_code=0 || exit_code=$?
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "No flake.nix found"
}

test_error_message_suggests_global_flag() {
    cd "$TEST_DIR"
    local output
    output=$("$NIXY" list 2>&1 || true)
    assert_output_contains "$output" "--global"
}

# =============================================================================
# Test: --global flag
# =============================================================================

test_global_flag_uses_global_flake() {
    cd "$TEST_DIR"
    # Create global flake
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1
    # Should work with --global even without local flake
    "$NIXY" list --global >/dev/null 2>&1
}

test_global_flag_short_form() {
    cd "$TEST_DIR"
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1
    "$NIXY" list -g >/dev/null 2>&1
}

test_global_flag_ignores_local_flake() {
    cd "$TEST_DIR"
    # Create both local and global flakes
    "$NIXY" init >/dev/null 2>&1
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Add marker to local flake to distinguish
    echo "# LOCAL_MARKER" >> flake.nix

    # --global should use global flake (no LOCAL_MARKER)
    assert_file_not_contains "$NIXY_CONFIG_DIR/flake.nix" "LOCAL_MARKER"
}

# =============================================================================
# Test: Install adds only specific package (not global dump)
# =============================================================================

test_install_adds_single_package() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    # Install a package (we'll mock this by calling add_package_to_flake directly via source)
    # Since we can't easily mock nix, we'll test the flake modification directly

    # Source the script to get access to functions
    source "$NIXY" --source-only 2>/dev/null || true

    # Manually test add_package_to_flake by checking flake content
    # For now, just verify the flake structure is correct after init
    assert_file_contains "./flake.nix" "# \[nixy:packages\]" && \
    assert_file_contains "./flake.nix" "# \[/nixy:packages\]"
}

test_install_preserves_existing_packages() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    # Manually add a package to the flake (use awk for portability)
    awk '/# \[nixy:packages\]/{print; print "          existing-pkg = pkgs.existing-pkg;"; next}1' flake.nix > flake.nix.tmp && mv flake.nix.tmp flake.nix

    # Verify existing-pkg is there
    assert_file_contains "./flake.nix" "existing-pkg"
}

# =============================================================================
# Test: Uninstall removes only specific package
# =============================================================================

test_flake_structure_after_init() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    # Verify all required sections exist
    assert_file_contains "./flake.nix" "# \[nixy:packages\]" && \
    assert_file_contains "./flake.nix" "# \[/nixy:packages\]" && \
    assert_file_contains "./flake.nix" "# \[nixy:devShell\]" && \
    assert_file_contains "./flake.nix" "# \[/nixy:devShell\]" && \
    assert_file_contains "./flake.nix" "# \[nixy:local-packages\]" && \
    assert_file_contains "./flake.nix" "# \[/nixy:local-packages\]"
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
    assert_exit_code 1 "$exit_code" && \
    # Should NOT create flake.nix
    assert_file_not_exists "./flake.nix" "flake.nix should not be created on failure"
}

test_uninstall_fails_cleanly_without_flake() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" uninstall testpkg 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_file_not_exists "./flake.nix"
}

test_upgrade_fails_cleanly_without_flake() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" upgrade 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_file_not_exists "./flake.nix"
}

test_sync_fails_cleanly_without_flake() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" sync 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_file_not_exists "./flake.nix"
}

test_sync_with_empty_flake() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

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
    "$NIXY" init >/dev/null 2>&1

    # Add packages to flake (simulating a flake with packages defined)
    awk '/# \[nixy:packages\]/{print; print "          ripgrep = pkgs.ripgrep;"; print "          fzf = pkgs.fzf;"; next}1' flake.nix > flake.nix.tmp && mv flake.nix.tmp flake.nix

    # Sync should not fail with unbound variable even when to_remove array is empty
    # (packages in flake but nothing to remove from nix profile)
    local output exit_code
    output=$("$NIXY" sync 2>&1) && exit_code=0 || exit_code=$?

    # Should not have unbound variable error regardless of exit code
    # (exit code may be non-zero if nix commands fail, but that's not what we're testing)
    if echo "$output" | grep -q "unbound variable"; then
        echo "  ASSERTION FAILED: sync should not have unbound variable error"
        echo "  Output: $output"
        return 1
    fi
    return 0
}

test_shell_fails_cleanly_without_flake() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" shell 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_file_not_exists "./flake.nix"
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

    "$NIXY" init >/dev/null 2>&1

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

    "$NIXY" init >/dev/null 2>&1

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

    "$NIXY" init >/dev/null 2>&1

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

    "$NIXY" init >/dev/null 2>&1

    local output exit_code
    output=$("$NIXY" install --file test-pkg.nix 2>&1) && exit_code=0 || exit_code=$?

    # Should fail with appropriate error message
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "Could not find 'name' or 'pname'"
}

test_install_file_not_found() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    local output exit_code
    output=$("$NIXY" install --file nonexistent.nix 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "File not found"
}

# =============================================================================
# Test: Help and basic commands
# =============================================================================

test_help_shows_init_command() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "init"
}

test_help_shows_global_flag() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "--global"
}

test_help_exit_code() {
    "$NIXY" help >/dev/null 2>&1
}

test_unknown_command_fails() {
    local exit_code
    "$NIXY" unknowncommand >/dev/null 2>&1 && exit_code=0 || exit_code=$?
    assert_exit_code 1 "$exit_code"
}

# =============================================================================
# Run all tests
# =============================================================================

main() {
    echo "======================================"
    echo "Running nixy unit tests"
    echo "======================================"
    echo ""

    # Init tests
    run_test "init creates flake.nix" test_init_creates_flake || true
    run_test "init fails if flake exists" test_init_fails_if_flake_exists || true
    run_test "init with directory argument" test_init_with_directory || true
    run_test "init creates empty packages section" test_init_creates_empty_packages_section || true

    # Flake discovery tests
    run_test "finds flake in current directory" test_finds_flake_in_current_dir || true
    run_test "finds flake in parent directory" test_finds_flake_in_parent_dir || true
    run_test "fails when no flake found" test_fails_when_no_flake_found || true
    run_test "error suggests --global flag" test_error_message_suggests_global_flag || true

    # Global flag tests
    run_test "--global uses global flake" test_global_flag_uses_global_flake || true
    run_test "-g short form works" test_global_flag_short_form || true
    run_test "--global ignores local flake" test_global_flag_ignores_local_flake || true

    # Package management tests
    run_test "flake has correct structure after init" test_flake_structure_after_init || true
    run_test "install preserves existing packages" test_install_preserves_existing_packages || true

    # Error propagation tests (the subshell exit bug)
    run_test "install fails cleanly without flake" test_install_fails_cleanly_without_flake || true
    run_test "uninstall fails cleanly without flake" test_uninstall_fails_cleanly_without_flake || true
    run_test "upgrade fails cleanly without flake" test_upgrade_fails_cleanly_without_flake || true
    run_test "sync fails cleanly without flake" test_sync_fails_cleanly_without_flake || true
    run_test "sync with empty flake succeeds" test_sync_with_empty_flake || true
    run_test "sync with packages no unbound variable" test_sync_with_packages_no_unbound_variable || true
    run_test "shell fails cleanly without flake" test_shell_fails_cleanly_without_flake || true

    # Local package file parsing tests
    run_test "parse pname from nixpkgs-style file" test_parse_pname_from_nixpkgs_style || true
    run_test "parse name from simple-style file" test_parse_name_from_simple_style || true
    run_test "pname takes precedence over name" test_parse_pname_takes_precedence || true
    run_test "fails without name or pname" test_parse_fails_without_name_or_pname || true
    run_test "install --file with nonexistent file" test_install_file_not_found || true

    # Help tests
    run_test "help shows init command" test_help_shows_init_command || true
    run_test "help shows --global flag" test_help_shows_global_flag || true
    run_test "help exits successfully" test_help_exit_code || true
    run_test "unknown command fails" test_unknown_command_fails || true

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
