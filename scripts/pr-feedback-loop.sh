#!/bin/bash
# PR Feedback Loop - Monitor and wait for Copilot review
# Usage: ./scripts/pr-feedback-loop.sh <PR_NUMBER>

set -e

PR="${1:-$(gh pr view --json number -q .number 2>/dev/null)}"
if [ -z "$PR" ]; then
    echo "Usage: $0 <PR_NUMBER>"
    exit 1
fi

REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
OWNER=$(gh repo view --json owner -q .owner.login)
REPO_NAME=$(gh repo view --json name -q .name)

echo "=== PR Feedback Loop for PR #$PR ==="
echo "Repo: $REPO"

# Check if Copilot is assigned, if not request review
check_and_request_copilot() {
    REVIEWERS=$(gh api repos/$REPO/pulls/$PR/requested_reviewers --jq '.users[].login' 2>/dev/null || echo "")
    if echo "$REVIEWERS" | grep -q "Copilot"; then
        echo "✓ Copilot is already assigned as reviewer"
    else
        echo "→ Requesting Copilot review..."
        gh copilot-review $PR
        echo "✓ Copilot review requested"
    fi
}

# Get unresolved thread count
get_unresolved_count() {
    gh api graphql -f query='query { repository(owner: "'"$OWNER"'", name: "'"$REPO_NAME"'") { pullRequest(number: '$PR') { reviewThreads(first: 50) { nodes { isResolved } } } } }' --jq '[.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false)] | length' 2>/dev/null || echo "0"
}

# Get unresolved threads with details
get_unresolved_threads() {
    gh api graphql -f query='query { repository(owner: "'"$OWNER"'", name: "'"$REPO_NAME"'") { pullRequest(number: '$PR') { reviewThreads(first: 50) { nodes { id isResolved comments(first: 1) { nodes { body path line } } } } } } }' --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false) | {id: .id, path: .comments.nodes[0].path, line: .comments.nodes[0].line, body: .comments.nodes[0].body}'
}

# Get Copilot review count
get_copilot_review_count() {
    gh api repos/$REPO/pulls/$PR/reviews --jq '[.[] | select(.user.login == "copilot-pull-request-reviewer")] | length' 2>/dev/null || echo "0"
}

# Show current status
show_status() {
    echo ""
    echo "=== Current Status ==="
    echo "Reviews:"
    gh api repos/$REPO/pulls/$PR/reviews --jq '.[] | "  \(.user.login): \(.state)"' 2>/dev/null || echo "  (none)"
    echo ""
    echo "Pending reviewers:"
    gh api repos/$REPO/pulls/$PR/requested_reviewers --jq '.users[].login | "  \(.)"' 2>/dev/null || echo "  (none)"
    echo ""
    UNRESOLVED=$(get_unresolved_count)
    echo "Unresolved threads: $UNRESOLVED"
}

# Wait for new Copilot review
wait_for_review() {
    local LAST_COUNT=$(get_copilot_review_count)
    local MAX_ATTEMPTS=40  # 10 minutes with 15s intervals

    echo ""
    echo "Waiting for Copilot review (current review count: $LAST_COUNT)..."

    for i in $(seq 1 $MAX_ATTEMPTS); do
        sleep 15

        CURRENT_COUNT=$(get_copilot_review_count)

        if [ "$CURRENT_COUNT" -gt "$LAST_COUNT" ]; then
            echo ""
            echo "✓ New Copilot review received!"
            return 0
        fi

        printf "\r$(date +%H:%M:%S) - Waiting... (attempt $i/$MAX_ATTEMPTS)"
    done

    echo ""
    echo "⚠ Timeout waiting for review"
    return 1
}

# Main flow
echo ""
echo "Step 1: Check/Request Copilot review"
check_and_request_copilot

show_status

UNRESOLVED=$(get_unresolved_count)
if [ "$UNRESOLVED" -gt "0" ]; then
    echo ""
    echo "=== Unresolved Feedback ==="
    get_unresolved_threads
    echo ""
    echo "Please fix the issues above, then run this script again."
    exit 0
fi

echo ""
echo "Step 2: Wait for Copilot review"
if wait_for_review; then
    UNRESOLVED=$(get_unresolved_count)
    if [ "$UNRESOLVED" -eq "0" ]; then
        echo ""
        echo "✅ Copilot review complete with no issues! PR is ready to merge."
    else
        echo ""
        echo "=== New Feedback ($UNRESOLVED unresolved threads) ==="
        get_unresolved_threads
        echo ""
        echo "Please fix the issues above, commit, push, then run this script again."
    fi
else
    show_status
fi
