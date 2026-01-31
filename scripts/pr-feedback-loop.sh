#!/bin/bash
# PR Feedback Loop - Monitor and wait for Copilot review
# Usage:
#   ./scripts/pr-feedback-loop.sh <PR>              - Full feedback loop
#   ./scripts/pr-feedback-loop.sh <PR> status       - Show current status
#   ./scripts/pr-feedback-loop.sh <PR> threads      - Show unresolved threads
#   ./scripts/pr-feedback-loop.sh <PR> resolve-all  - Resolve all threads
#   ./scripts/pr-feedback-loop.sh <PR> request      - Request Copilot review
#   ./scripts/pr-feedback-loop.sh <PR> wait         - Wait for new review

set -e

PR="${1:-$(gh pr view --json number -q .number 2>/dev/null)}"
ACTION="${2:-loop}"

if [ -z "$PR" ]; then
    echo "Usage: $0 <PR_NUMBER> [status|threads|resolve-all|request|wait|loop]"
    exit 1
fi

# Validate PR is a positive integer to prevent command injection
if ! [[ "$PR" =~ ^[0-9]+$ ]]; then
    echo "Error: PR number must be a positive integer, got: $PR"
    exit 1
fi

REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
OWNER=$(gh repo view --json owner -q .owner.login)
REPO_NAME=$(gh repo view --json name -q .name)

# ============================================================================
# Helper Functions
# ============================================================================

get_unresolved_count() {
    gh api graphql -f query='query { repository(owner: "'"$OWNER"'", name: "'"$REPO_NAME"'") { pullRequest(number: '$PR') { reviewThreads(first: 100) { nodes { isResolved } } } } }' --jq '[.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false)] | length' 2>/dev/null || echo "0"
}

get_unresolved_thread_ids() {
    gh api graphql -f query='query { repository(owner: "'"$OWNER"'", name: "'"$REPO_NAME"'") { pullRequest(number: '$PR') { reviewThreads(first: 100) { nodes { id isResolved } } } } }' --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false) | .id'
}

get_unresolved_threads() {
    gh api graphql -f query='query { repository(owner: "'"$OWNER"'", name: "'"$REPO_NAME"'") { pullRequest(number: '$PR') { reviewThreads(first: 100) { nodes { id isResolved comments(first: 1) { nodes { body path line } } } } } } }' --jq '.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false) | {id: .id, path: .comments.nodes[0].path, line: .comments.nodes[0].line, body: .comments.nodes[0].body}'
}

resolve_thread() {
    local THREAD_ID="$1"
    gh api graphql -f query='mutation { resolveReviewThread(input: {threadId: "'"$THREAD_ID"'"}) { thread { isResolved } } }' > /dev/null
}

get_copilot_review_count() {
    gh api repos/$REPO/pulls/$PR/reviews --jq '[.[] | select(.user.login == "copilot-pull-request-reviewer")] | length' 2>/dev/null || echo "0"
}

is_copilot_assigned() {
    gh api repos/$REPO/pulls/$PR/requested_reviewers --jq '.users[].login' 2>/dev/null | grep -q "Copilot"
}

# ============================================================================
# Actions
# ============================================================================

do_status() {
    echo "=== PR #$PR Status ==="
    echo "Repo: $REPO"
    echo ""
    echo "Reviews:"
    gh api repos/$REPO/pulls/$PR/reviews --jq '.[] | "  \(.user.login): \(.state)"' 2>/dev/null || echo "  (none)"
    echo ""
    echo "Pending reviewers:"
    gh api repos/$REPO/pulls/$PR/requested_reviewers --jq '.users[].login | "  \(.)"' 2>/dev/null || echo "  (none)"
    echo ""
    echo "Unresolved threads: $(get_unresolved_count)"
}

do_threads() {
    local COUNT=$(get_unresolved_count)
    echo "=== Unresolved Threads ($COUNT) ==="
    if [ "$COUNT" -gt "0" ]; then
        get_unresolved_threads
    else
        echo "(none)"
    fi
}

do_resolve_all() {
    echo "=== Resolving All Threads ==="
    local IDS=$(get_unresolved_thread_ids)
    if [ -z "$IDS" ]; then
        echo "No unresolved threads to resolve."
        return
    fi

    for id in $IDS; do
        resolve_thread "$id"
        echo "Resolved: $id"
    done
    echo ""
    echo "✓ All threads resolved"
}

do_request() {
    echo "=== Request Copilot Review ==="
    if is_copilot_assigned; then
        echo "✓ Copilot is already assigned as reviewer"
    else
        echo "→ Requesting Copilot review..."
        if ! gh copilot-review "$PR"; then
            echo "✗ Failed to request Copilot review."
            echo "  Make sure the extension is installed:"
            echo "  gh extension install ChrisCarini/gh-copilot-review"
            exit 1
        fi
        echo "✓ Copilot review requested"
    fi
}

do_wait() {
    echo "=== Waiting for Copilot Review ==="
    local LAST_COUNT=$(get_copilot_review_count)
    local MAX_ATTEMPTS=40  # 10 minutes with 15s intervals

    echo "Current review count: $LAST_COUNT"
    echo ""

    for i in $(seq 1 $MAX_ATTEMPTS); do
        sleep 15

        CURRENT_COUNT=$(get_copilot_review_count)

        if [ "$CURRENT_COUNT" -gt "$LAST_COUNT" ]; then
            echo ""
            echo "✓ New Copilot review received!"
            echo ""
            do_status
            return 0
        fi

        printf "\r$(date +%H:%M:%S) - Waiting... (attempt $i/$MAX_ATTEMPTS)"
    done

    echo ""
    echo "⚠ Timeout waiting for review"
    do_status
    return 1
}

do_loop() {
    echo "=== PR Feedback Loop for PR #$PR ==="
    echo "Repo: $REPO"
    echo ""

    # Step 1: Request review if needed
    echo "Step 1: Check/Request Copilot review"
    do_request

    echo ""
    do_status

    # Check for existing unresolved threads
    UNRESOLVED=$(get_unresolved_count)
    if [ "$UNRESOLVED" -gt "0" ]; then
        echo ""
        do_threads
        echo ""
        echo "Please fix the issues above, then run this script again."
        exit 0
    fi

    # Step 2: Wait for review
    echo ""
    echo "Step 2: Wait for Copilot review"
    if do_wait; then
        UNRESOLVED=$(get_unresolved_count)
        if [ "$UNRESOLVED" -eq "0" ]; then
            echo ""
            echo "✅ Copilot review complete with no issues! PR is ready to merge."
        else
            echo ""
            do_threads
            echo ""
            echo "Please fix the issues above, commit, push, then run this script again."
        fi
    fi
}

# ============================================================================
# Main
# ============================================================================

case "$ACTION" in
    status)
        do_status
        ;;
    threads)
        do_threads
        ;;
    resolve-all)
        do_resolve_all
        ;;
    request)
        do_request
        ;;
    wait)
        do_wait
        ;;
    loop|"")
        do_loop
        ;;
    *)
        echo "Unknown action: $ACTION"
        echo "Usage: $0 <PR_NUMBER> [status|threads|resolve-all|request|wait|loop]"
        exit 1
        ;;
esac
