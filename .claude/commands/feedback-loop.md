# PR Feedback Loop

Monitor a Pull Request for new feedback and automatically address it.

## Usage

Run `/feedback-loop <pr-number>` or just `/feedback-loop` (uses current branch's PR).

## Arguments

- `$ARGUMENTS` - PR number (optional, defaults to current branch's PR)

## Instructions

1. **Determine the PR number**
   - If `$ARGUMENTS` is provided, use it as the PR number
   - Otherwise, get the PR for the current branch: `gh pr view --json number -q .number`

2. **Start monitoring** - Run this command in background to watch for new comments:
   ```bash
   PR=<number>
   REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
   LAST_COMMENTS=$(gh api repos/$REPO/pulls/$PR/comments --jq 'length')
   LAST_REVIEWS=$(gh api repos/$REPO/pulls/$PR/reviews --jq 'length')
   echo "Watching PR #$PR (current comments: $LAST_COMMENTS)"

   while true; do
     sleep 30
     COMMENTS=$(gh api repos/$REPO/pulls/$PR/comments --jq 'length')
     REVIEWS=$(gh api repos/$REPO/pulls/$PR/reviews --jq 'length')
     echo "$(date +%H:%M:%S) - Comments: $COMMENTS, Reviews: $REVIEWS"

     if [ "$COMMENTS" -gt "$LAST_COMMENTS" ] || [ "$REVIEWS" -gt "$LAST_REVIEWS" ]; then
       echo "NEW FEEDBACK"
       gh api repos/$REPO/pulls/$PR/comments --jq '.[-'$((COMMENTS-LAST_COMMENTS))':] | .[] | "File: \(.path):\(.line)\n\(.body)\n---"'
       LAST_COMMENTS=$COMMENTS
       LAST_REVIEWS=$REVIEWS
     fi
   done
   ```

3. **When new feedback is detected**:
   - Read and understand each comment
   - Fix the code issues in the relevant files
   - Run tests: `cargo test`
   - Commit and push changes with a descriptive message
   - Resolve review threads via GraphQL:
     ```bash
     # Get unresolved thread IDs
     OWNER=$(gh repo view --json owner -q .owner.login)
     REPO=$(gh repo view --json name -q .name)
     gh api graphql -f query='query { repository(owner: "'$OWNER'", name: "'$REPO'") { pullRequest(number: '$PR') { reviewThreads(first: 50) { nodes { id isResolved } } } } }' --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false) | .id'

     # Resolve each thread
     gh api graphql -f query='mutation { resolveReviewThread(input: {threadId: "THREAD_ID"}) { thread { isResolved } } }'
     ```
   - **Request re-review from Copilot**:
     > **Note**: There is no official API to trigger Copilot re-review. The user must manually
     > click the "Re-request review" button next to Copilot's name in the Reviewers menu on GitHub.
     > The API call below may work in some cases but is not guaranteed:
     ```bash
     gh api repos/$OWNER/$REPO/pulls/$PR/requested_reviewers -X POST -f 'reviewers[]=Copilot'
     ```
     If re-review doesn't trigger automatically, ask the user to click the re-request button.

4. **Continue monitoring** until the review passes with no new change requests

5. **Completion**: When Copilot re-reviews and has no new feedback, inform the user that all feedback has been addressed.
