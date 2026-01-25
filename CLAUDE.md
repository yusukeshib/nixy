# Claude Code Instructions for nixy

## Project Overview

nixy is a Homebrew-style wrapper for Nix using flake.nix. It's a bash script that manages Nix packages through a declarative flake.nix file.

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
- Edit `nixy` (the main script)
- Follow existing code patterns and style
- **Update documentation**: If adding new commands or options, update both `README.md` and `README_ja.md`

### 3. Add Tests
- Add tests to `test_nixy.sh`
- Tests use a simple assertion-based framework
- Place tests in the appropriate section (create new section if needed)
- Add test function calls to `main()` function

Test structure:
```bash
test_feature_name() {
    cd "$TEST_DIR"
    "$NIXY" init >/dev/null 2>&1

    local output exit_code
    output=$("$NIXY" command 2>&1) && exit_code=0 || exit_code=$?

    assert_exit_code 0 "$exit_code" && \
    assert_output_contains "$output" "expected text"
}
```

### 4. Run Tests
```bash
./test_nixy.sh
```
All tests must pass before committing.

### 5. Update Version
- Bump `NIXY_VERSION` in `nixy` (line ~19)
- Use semantic versioning (MAJOR.MINOR.PATCH)

### 6. Commit, Push, and Create PR
```bash
git add nixy test_nixy.sh CLAUDE.md
git commit -m "Description of change, bump to X.Y.Z

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
git push -u origin <username>/<feature-name>
gh pr create --title "Feature description" --body "## Summary
- Change 1
- Change 2

## Test plan
- [ ] Run ./test_nixy.sh
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

- Use `info()`, `success()`, `warn()`, `error()`, `die()` for output
- Use `NIX_FLAGS` array for nix command flags
- Parse args with `while [[ $# -gt 0 ]]; do case "$1" in ... esac; done`
- Validate inputs early and fail fast with `die()`
- Use `|| true` for commands that may fail in pipelines

## Key Files

- `nixy` - Main executable script
- `test_nixy.sh` - Unit tests
- `install.sh` - Installation script
- `flake.nix` - Nix flake for nixy itself

## Testing Notes

- Tests use isolated `TEST_DIR` and `NIXY_CONFIG_DIR`
- Tests run in subshells to avoid polluting environment
- Use `|| true` after `run_test` calls in main() to continue on failure
- Validation tests that need valid packages should use `hello` (always available in nixpkgs)

## Claude Code Commands

### pr-watch
Monitor a PR for new feedback. Run in background and notify when new comments arrive.
```
PR=<number> REPO="owner/repo" INTERVAL=300; LAST=$(gh api repos/$REPO/pulls/$PR/comments --jq 'length'); while true; do sleep $INTERVAL; C=$(gh api repos/$REPO/pulls/$PR/comments --jq 'length'); if [ "$C" -gt "$LAST" ]; then echo "🔔 NEW FEEDBACK"; gh api repos/$REPO/pulls/$PR/comments --jq '.[-'$((C-LAST))':] | .[] | "File: \(.path):\(.line)\n\(.body)\n---"'; LAST=$C; fi; done
```

### pr-feedback-loop
When user asks to monitor and resolve PR feedback automatically:
1. Start pr-watch in background
2. When new feedback arrives:
   - Read the comments
   - Fix the code issues
   - Run tests (`cargo test --test integration`)
   - Commit and push changes
   - Resolve review threads via GraphQL:
     ```
     gh api graphql -f query='query { repository(owner: "OWNER", name: "REPO") { pullRequest(number: NUM) { reviewThreads(first: 50) { nodes { id isResolved } } } } }' --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false) | .id'
     ```
     Then for each thread_id:
     ```
     gh api graphql -f query='mutation { resolveReviewThread(input: {threadId: "THREAD_ID"}) { thread { isResolved } } }'
     ```
   - Copilot automatically re-reviews on new commits (no manual trigger needed)
3. Continue monitoring
4. When Copilot review has no new change feedback, the task is complete
