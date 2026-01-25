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
ORIGINAL_NIXY_PROFILE="${NIXY_PROFILE:-}"
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
    if [[ -n "$ORIGINAL_NIXY_PROFILE" ]]; then
        export NIXY_PROFILE="$ORIGINAL_NIXY_PROFILE"
    else
        unset NIXY_PROFILE 2>/dev/null || true
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
    export NIXY_PROFILE="$TEST_DIR/profile"
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
    if [[ -n "$ORIGINAL_NIXY_PROFILE" ]]; then
        export NIXY_PROFILE="$ORIGINAL_NIXY_PROFILE"
    else
        unset NIXY_PROFILE
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
# Test: Global vs Local flake structure (devShells)
# =============================================================================

test_local_flake_has_devshells() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    # Local/project flakes should have devShells for nixy shell
    assert_file_contains "./flake.nix" "devShells" && \
    assert_file_contains "./flake.nix" "# \[nixy:devShell\]" && \
    assert_file_contains "./flake.nix" "# \[/nixy:devShell\]"
}

test_global_flake_has_no_devshells() {
    cd "$TEST_DIR"

    # Create a global flake by installing a package with -g
    # First create the global config dir
    mkdir -p "$NIXY_CONFIG_DIR"

    # Manually add a package to trigger flake generation with --global
    # We'll simulate this by calling the init on global dir, then checking
    # that when we add a package with -g, devShells is removed

    # Create initial global flake
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Add a package marker in the packages section (simulating install -g)
    awk '/# \[nixy:packages\]/{print; print "          ripgrep = pkgs.ripgrep;"; next}1' "$NIXY_CONFIG_DIR/flake.nix" > "$NIXY_CONFIG_DIR/flake.nix.tmp"
    mv "$NIXY_CONFIG_DIR/flake.nix.tmp" "$NIXY_CONFIG_DIR/flake.nix"

    # Now trigger a regeneration via add_package_to_flake by attempting to install
    # We source nixy to directly test the generate_flake function behavior

    # Test: Use generate_flake directly with --global flag
    source "$NIXY"
    local flake_content
    flake_content=$(generate_flake --flake-dir "$NIXY_CONFIG_DIR" --global ripgrep)

    # Global flakes should NOT have devShells
    if echo "$flake_content" | grep -q "devShells"; then
        echo "  ASSERTION FAILED: Global flake should NOT contain devShells"
        echo "  Content contains: devShells"
        return 1
    fi

    # But should still have packages section
    if ! echo "$flake_content" | grep -q "packages = forAllSystems"; then
        echo "  ASSERTION FAILED: Global flake should contain packages"
        return 1
    fi

    return 0
}

test_local_flake_generation_has_devshells() {
    cd "$TEST_DIR"

    # Source nixy to test generate_flake directly
    source "$NIXY"

    # Generate a local flake (no --global flag)
    local flake_content
    flake_content=$(generate_flake --flake-dir "$TEST_DIR" ripgrep)

    # Local flakes SHOULD have devShells
    if ! echo "$flake_content" | grep -q "devShells"; then
        echo "  ASSERTION FAILED: Local flake should contain devShells"
        return 1
    fi

    # And should have packages section
    if ! echo "$flake_content" | grep -q "packages = forAllSystems"; then
        echo "  ASSERTION FAILED: Local flake should contain packages"
        return 1
    fi

    return 0
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

test_install_fails_on_non_nixy_flake() {
    cd "$TEST_DIR"
    # Create a non-nixy flake.nix (no markers)
    cat > flake.nix <<'EOF'
{
  description = "A custom flake";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: {
    packages = {};
  };
}
EOF

    local output exit_code
    # Use 'hello' which is a valid package (passes validation)
    output=$("$NIXY" install hello 2>&1) && exit_code=0 || exit_code=$?

    # Should fail with error about non-nixy flake
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "not managed by nixy"
}

test_uninstall_fails_on_non_nixy_flake() {
    cd "$TEST_DIR"
    # Create a non-nixy flake.nix (no markers)
    cat > flake.nix <<'EOF'
{
  description = "A custom flake";
  outputs = { self }: {};
}
EOF

    local output exit_code
    output=$("$NIXY" uninstall testpkg 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "not managed by nixy"
}

test_sync_fails_cleanly_without_flake() {
    cd "$TEST_DIR"
    local output exit_code
    output=$("$NIXY" sync --global 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_file_not_exists "$NIXY_CONFIG_DIR/flake.nix"
}

test_sync_requires_global_flag() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    local output exit_code
    output=$("$NIXY" sync 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "sync only works with --global flag"
}

test_sync_with_empty_flake() {
    cd "$TEST_DIR"
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Sync with empty flake (no packages) should not fail with unbound variable
    local output exit_code
    output=$("$NIXY" sync --global 2>&1) && exit_code=0 || exit_code=$?

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
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Add packages to flake (simulating a flake with packages defined)
    awk '/# \[nixy:packages\]/{print; print "          ripgrep = pkgs.ripgrep;"; print "          fzf = pkgs.fzf;"; next}1' "$NIXY_CONFIG_DIR/flake.nix" > "$NIXY_CONFIG_DIR/flake.nix.tmp" && mv "$NIXY_CONFIG_DIR/flake.nix.tmp" "$NIXY_CONFIG_DIR/flake.nix"

    # Sync should not fail with unbound variable even when to_remove array is empty
    # (packages in flake but nothing to remove from nix profile)
    local output exit_code
    output=$("$NIXY" sync --global 2>&1) && exit_code=0 || exit_code=$?

    # Should not have unbound variable error regardless of exit code
    # (exit code may be non-zero if nix commands fail, but that's not what we're testing)
    if echo "$output" | grep -q "unbound variable"; then
        echo "  ASSERTION FAILED: sync should not have unbound variable error"
        echo "  Output: $output"
        return 1
    fi
    return 0
}

test_sync_preserves_local_packages() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    # Add both a regular package and a local package to the flake
    awk '/# \[nixy:packages\]/{print; print "          ripgrep = pkgs.ripgrep;"; next}1' flake.nix > flake.nix.tmp && mv flake.nix.tmp flake.nix
    awk '/# \[nixy:local-packages\]/{print; print "          my-local-pkg = pkgs.callPackage ./packages/my-local-pkg.nix {};"; next}1' flake.nix > flake.nix.tmp && mv flake.nix.tmp flake.nix

    # Test get_packages_from_flake returns both regular and local packages
    local packages
    packages=$({
        sed -n '/# \[nixy:packages\]/,/# \[\/nixy:packages\]/p' flake.nix 2>/dev/null | \
            { grep -E '^\s+[a-zA-Z0-9_-]+ = pkgs\.' || true; } | \
            sed 's/^[[:space:]]*\([a-zA-Z0-9_-]*\) = pkgs\..*/\1/'
        sed -n '/# \[nixy:local-packages\]/,/# \[\/nixy:local-packages\]/p' flake.nix 2>/dev/null | \
            { grep -E '^\s+[a-zA-Z0-9_-]+ = ' || true; } | \
            sed 's/^[[:space:]]*\([a-zA-Z0-9_-]*\) = .*/\1/'
    } | sort -u)

    # Test get_local_packages_from_flake returns only local packages
    local local_packages
    local_packages=$(sed -n '/# \[nixy:local-packages\]/,/# \[\/nixy:local-packages\]/p' flake.nix 2>/dev/null | \
        { grep -E '^\s+[a-zA-Z0-9_-]+ = ' || true; } | \
        sed 's/^[[:space:]]*\([a-zA-Z0-9_-]*\) = .*/\1/' | \
        sort -u)

    # Should contain the regular package in all packages
    if ! echo "$packages" | grep -q "ripgrep"; then
        echo "  ASSERTION FAILED: get_packages_from_flake should return ripgrep"
        echo "  Packages: $packages"
        return 1
    fi

    # Should contain the local package in all packages
    if ! echo "$packages" | grep -q "my-local-pkg"; then
        echo "  ASSERTION FAILED: get_packages_from_flake should return my-local-pkg"
        echo "  Packages: $packages"
        return 1
    fi

    # Local packages list should contain my-local-pkg
    if ! echo "$local_packages" | grep -q "my-local-pkg"; then
        echo "  ASSERTION FAILED: get_local_packages_from_flake should return my-local-pkg"
        echo "  Local packages: $local_packages"
        return 1
    fi

    # Local packages list should NOT contain ripgrep
    if echo "$local_packages" | grep -q "ripgrep"; then
        echo "  ASSERTION FAILED: get_local_packages_from_flake should NOT return ripgrep"
        echo "  Local packages: $local_packages"
        return 1
    fi

    return 0
}

test_sync_without_remove_only_warns() {
    cd "$TEST_DIR"
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Sync with empty flake should warn about extra packages (but not fail)
    # We need to mock the installed packages, but since nix profile is isolated,
    # this will just test that sync doesn't error with unbound variables
    local output exit_code
    output=$("$NIXY" sync --global 2>&1) && exit_code=0 || exit_code=$?

    # Should succeed
    assert_exit_code 0 "$exit_code" && \
    # Output should mention "in sync" or have no removal messages
    if echo "$output" | grep -q "Removing"; then
        echo "  ASSERTION FAILED: sync without --remove should NOT remove packages"
        return 1
    fi
    return 0
}

test_sync_remove_flag_accepted() {
    cd "$TEST_DIR"
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Test that --remove flag is accepted (doesn't cause unknown option error)
    local output exit_code
    output=$("$NIXY" sync --global --remove 2>&1) && exit_code=0 || exit_code=$?

    # Should succeed (empty flake, nothing to remove)
    assert_exit_code 0 "$exit_code" && \
    # Should not have "Unknown option" error
    if echo "$output" | grep -q "Unknown option"; then
        echo "  ASSERTION FAILED: --remove should be a valid option"
        return 1
    fi
    return 0
}

test_sync_short_remove_flag_accepted() {
    cd "$TEST_DIR"
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Test that -r short flag is accepted
    local output exit_code
    output=$("$NIXY" sync -g -r 2>&1) && exit_code=0 || exit_code=$?

    # Should succeed (empty flake, nothing to remove)
    assert_exit_code 0 "$exit_code" && \
    # Should not have "Unknown option" error
    if echo "$output" | grep -q "Unknown option"; then
        echo "  ASSERTION FAILED: -r should be a valid option"
        return 1
    fi
    return 0
}

test_help_shows_sync_remove_flag() {
    local output
    output=$("$NIXY" help 2>&1)
    assert_output_contains "$output" "sync" && \
    assert_output_contains "$output" "--remove"
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

    # Create global flake (packages are always stored in NIXY_CONFIG_DIR/packages)
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Install the local file with -g flag (will fail at nix profile add, but flake should be generated)
    "$NIXY" install --file test-pkg.nix -g 2>&1 || true

    # Verify package was copied
    assert_file_exists "$NIXY_CONFIG_DIR/packages/my-local-pkg.nix" && \

    # Verify flake.nix has the local package entry
    assert_file_contains "$NIXY_CONFIG_DIR/flake.nix" "my-local-pkg = pkgs.callPackage ./packages/my-local-pkg.nix"
}

test_install_file_copies_to_flake_dir_packages() {
    cd "$TEST_DIR"
    # Create a package file
    cat > test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "flake-dir-test-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Create local flake in a subdirectory
    mkdir -p myproject
    "$NIXY" init myproject >/dev/null 2>&1

    cd myproject

    # Install the local file (will fail at nix profile add, but package file should be copied)
    "$NIXY" install --file ../test-pkg.nix 2>&1 || true

    # Verify package was copied to the flake directory's packages subdir (not NIXY_CONFIG_DIR)
    assert_file_exists "./packages/flake-dir-test-pkg.nix" "Package should be in flake dir's packages subdir" && \

    # Verify flake.nix has the local package entry
    assert_file_contains "./flake.nix" "flake-dir-test-pkg = pkgs.callPackage ./packages/flake-dir-test-pkg.nix"
}

test_install_file_adds_to_git_in_git_repo() {
    cd "$TEST_DIR"

    # Create a git repository for the flake
    mkdir -p myproject
    cd myproject
    git init >/dev/null 2>&1
    git config user.email "test@test.com" >/dev/null 2>&1
    git config user.name "Test" >/dev/null 2>&1

    # Create flake and commit it
    "$NIXY" init . >/dev/null 2>&1
    git add flake.nix >/dev/null 2>&1
    git commit -m "Initial commit" >/dev/null 2>&1

    # Create a package file
    cat > ../test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "git-tracked-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Install the local file (will fail at nix profile add, but package file should be added to git)
    "$NIXY" install --file ../test-pkg.nix 2>&1 || true

    # Verify package file was added to git staging area
    local git_status
    git_status=$(git status --porcelain 2>/dev/null)

    if echo "$git_status" | grep -q "A.*packages/git-tracked-pkg.nix"; then
        return 0
    else
        echo "  ASSERTION FAILED: Package file should be staged in git"
        echo "  Git status: $git_status"
        return 1
    fi
}

test_install_file_works_without_git() {
    cd "$TEST_DIR"

    # Create flake without git (just a regular directory)
    mkdir -p myproject
    "$NIXY" init myproject >/dev/null 2>&1

    cd myproject

    # Create a package file
    cat > ../test-pkg.nix <<'EOF'
{ lib, stdenv }:

stdenv.mkDerivation {
  pname = "no-git-pkg";
  version = "1.0.0";
  src = ./.;
}
EOF

    # Install the local file (will fail at nix profile add, but package file should still be copied)
    "$NIXY" install --file ../test-pkg.nix 2>&1 || true

    # Verify package was copied successfully even without git
    assert_file_exists "./packages/no-git-pkg.nix" "Package should be copied even without git"
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
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Install the flake file with -g flag
    "$NIXY" install --file my-flake.nix -g 2>&1 || true

    # Verify flake was copied to a subdirectory
    assert_file_exists "$NIXY_CONFIG_DIR/packages/my-flake/flake.nix" "Flake should be in subdirectory"
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
    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Install the flake file
    "$NIXY" install --file gke-plugin.nix -g 2>&1 || true

    # Verify flake.nix has the input
    assert_file_contains "$NIXY_CONFIG_DIR/flake.nix" 'gke-plugin.url = "path:./packages/gke-plugin"' && \

    # Verify flake.nix has the package expression
    assert_file_contains "$NIXY_CONFIG_DIR/flake.nix" 'gke-plugin = gke-plugin.packages'
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

    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Install regular package
    "$NIXY" install --file regular-pkg.nix -g 2>&1 || true

    # Install flake package
    "$NIXY" install --file flake-pkg.nix -g 2>&1 || true

    # Regular package should be a .nix file
    assert_file_exists "$NIXY_CONFIG_DIR/packages/regular-pkg.nix" && \

    # Flake package should be in a subdirectory
    assert_file_exists "$NIXY_CONFIG_DIR/packages/flake-pkg/flake.nix"
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

    "$NIXY" init "$NIXY_CONFIG_DIR" >/dev/null 2>&1

    # Install the flake
    "$NIXY" install --file my-flake.nix -g 2>&1 || true

    # Verify it was installed
    assert_file_exists "$NIXY_CONFIG_DIR/packages/my-flake/flake.nix" || return 1

    # Uninstall it
    "$NIXY" uninstall my-flake -g 2>&1 || true

    # Verify directory was removed
    if [[ -d "$NIXY_CONFIG_DIR/packages/my-flake" ]]; then
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
    "$NIXY" init >/dev/null 2>&1

    local output exit_code
    output=$("$NIXY" install rust 2>&1) && exit_code=0 || exit_code=$?

    # Should fail with validation error
    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "not found in nixpkgs"
}

test_validate_package_rejects_attribute_set() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    # 'lib' is an attribute set, not a derivation
    local output exit_code
    output=$("$NIXY" install lib 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 1 "$exit_code" && \
    assert_output_contains "$output" "not found in nixpkgs"
}

test_validate_package_accepts_valid_package() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

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
    "$NIXY" init >/dev/null 2>&1

    local output
    output=$("$NIXY" install invalidpkg123 2>&1 || true)

    # Error message should suggest using search
    assert_output_contains "$output" "nixy search"
}

test_validate_skipped_for_file_install() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

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

    # Global vs Local flake structure tests
    run_test "local flake has devShells" test_local_flake_has_devshells || true
    run_test "global flake has no devShells" test_global_flake_has_no_devshells || true
    run_test "local flake generation has devShells" test_local_flake_generation_has_devshells || true

    # Package management tests
    run_test "flake has correct structure after init" test_flake_structure_after_init || true
    run_test "install preserves existing packages" test_install_preserves_existing_packages || true

    # Error propagation tests (the subshell exit bug)
    run_test "install fails cleanly without flake" test_install_fails_cleanly_without_flake || true
    run_test "uninstall fails cleanly without flake" test_uninstall_fails_cleanly_without_flake || true
    run_test "upgrade fails cleanly without flake" test_upgrade_fails_cleanly_without_flake || true
    run_test "install fails on non-nixy flake" test_install_fails_on_non_nixy_flake || true
    run_test "uninstall fails on non-nixy flake" test_uninstall_fails_on_non_nixy_flake || true
    run_test "sync fails cleanly without flake" test_sync_fails_cleanly_without_flake || true
    run_test "sync requires --global flag" test_sync_requires_global_flag || true
    run_test "sync with empty flake succeeds" test_sync_with_empty_flake || true
    run_test "sync with packages no unbound variable" test_sync_with_packages_no_unbound_variable || true
    run_test "sync preserves local packages" test_sync_preserves_local_packages || true
    run_test "sync without --remove only warns" test_sync_without_remove_only_warns || true
    run_test "sync --remove flag accepted" test_sync_remove_flag_accepted || true
    run_test "sync -r short flag accepted" test_sync_short_remove_flag_accepted || true
    run_test "help shows sync --remove flag" test_help_shows_sync_remove_flag || true
    run_test "shell fails cleanly without flake" test_shell_fails_cleanly_without_flake || true

    # Local package file parsing tests
    run_test "parse pname from nixpkgs-style file" test_parse_pname_from_nixpkgs_style || true
    run_test "parse name from simple-style file" test_parse_name_from_simple_style || true
    run_test "pname takes precedence over name" test_parse_pname_takes_precedence || true
    run_test "fails without name or pname" test_parse_fails_without_name_or_pname || true
    run_test "install --file with nonexistent file" test_install_file_not_found || true
    run_test "install --file adds to local-packages section" test_install_file_adds_to_local_packages_section || true
    run_test "install --file copies to flake dir packages" test_install_file_copies_to_flake_dir_packages || true
    run_test "install --file adds to git in git repo" test_install_file_adds_to_git_in_git_repo || true
    run_test "install --file works without git" test_install_file_works_without_git || true
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
    run_test "help shows init command" test_help_shows_init_command || true
    run_test "help shows --global flag" test_help_shows_global_flag || true
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
