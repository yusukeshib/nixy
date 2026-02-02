# PR Feedback Loop

Monitor a Pull Request for feedback and automatically address it using sub-agents.

## Usage

Run `/feedback-loop <pr-number>` or just `/feedback-loop` (uses current branch's PR).

## Arguments

- `$ARGUMENTS` - PR number (optional, defaults to current branch's PR)

## Instructions

### Phase 1: Setup and Status Check

1. **Determine the PR number**
   - If `$ARGUMENTS` is provided, use it as the PR number
   - Otherwise, get the PR for the current branch: `gh pr view --json number -q .number`
   - Store in variable: `PR=<number>`

2. **Get repository info**
   ```bash
   REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
   OWNER=$(gh repo view --json owner -q .owner.login)
   REPO_NAME=$(gh repo view --json name -q .name)
   ```

3. **Check current PR status**
   ```bash
   ./scripts/pr-feedback-loop.sh $PR status
   ```

### Phase 2: Get Unresolved Feedback

4. **Fetch all unresolved review threads**
   ```bash
   ./scripts/pr-feedback-loop.sh $PR threads
   ```

5. **If there are unresolved threads**, extract each thread's details using:
   ```bash
   gh api graphql -f query='query { repository(owner: "'$OWNER'", name: "'$REPO_NAME'") { pullRequest(number: '$PR') { reviewThreads(first: 100) { nodes { id isResolved comments(first: 10) { nodes { body path line author { login } } } } } } } }' --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false)'
   ```

### Phase 3: Fix Each Feedback Item (Sub-Agents)

6. **For EACH unresolved feedback thread**, spawn a sub-agent using the Task tool:

   Use `subagent_type: "general-purpose"` with a prompt like:
   ```
   Fix the following PR review feedback:

   File: <path>
   Line: <line>
   Feedback: <body>

   Instructions:
   1. Read the file mentioned in the feedback
   2. Understand the issue being raised
   3. Make the necessary code changes to address the feedback
   4. Run `cargo fmt` to format the code
   5. Run `cargo clippy -- -D warnings` to check for issues
   6. Run `cargo test` to verify nothing is broken
   7. Report what changes you made

   Do NOT commit - just make the changes and report back.
   ```

   **IMPORTANT**: Launch sub-agents in parallel when feedback items are independent (different files). Launch sequentially if they affect the same code.

### Phase 4: Commit and Push

7. **After all sub-agents complete**, verify and commit:
   ```bash
   git status
   git diff
   cargo test
   ```

8. **Stage and commit changes**:
   ```bash
   git add -A
   git commit -m "Address PR review feedback

   Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
   ```

9. **Push changes**:
   ```bash
   git push
   ```

### Phase 5: Resolve Threads and Request Re-review

10. **Resolve all addressed threads**:
    ```bash
    ./scripts/pr-feedback-loop.sh $PR resolve-all
    ```

11. **Request Copilot re-review** (IMPORTANT: verify it's actually requested):
    ```bash
    ./scripts/pr-feedback-loop.sh $PR request
    ```

12. **Verify review was requested** by checking pending reviewers:
    ```bash
    gh api repos/$REPO/pulls/$PR/requested_reviewers --jq '.users[].login'
    ```
    If "Copilot" is NOT in the list, inform the user they need to manually click "Re-request review" on GitHub.

### Phase 6: Wait for New Review

13. **Only if review was successfully requested**, wait for the new review:
    ```bash
    ./scripts/pr-feedback-loop.sh $PR wait
    ```

14. **After receiving new review**:
    - If no new feedback: Inform user "All feedback addressed! PR is ready to merge."
    - If new feedback exists: Go back to Phase 2 and repeat

## Key Points

- **Use sub-agents (Task tool)** for fixing each feedback item to prevent context explosion
- **Run sub-agents in parallel** when fixing independent files
- **Always verify review is requested** before waiting
- **Don't commit from sub-agents** - let the main agent handle commits to avoid conflicts
- The script `./scripts/pr-feedback-loop.sh` handles most GitHub API interactions
