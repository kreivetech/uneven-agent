#!/usr/bin/env bash
set -euo pipefail

TAG=""
if [[ "${1:-}" == "--tag" && -n "${2:-}" ]]; then
  TAG="$2"
fi

REMOTE="org"
if ! git remote get-url "$REMOTE" >/dev/null 2>&1; then
  git remote add "$REMOTE" git@github.com:kreivetech/uneven-agent.git
fi

git fetch "$REMOTE" main

TMP_DIR="$(mktemp -d)"
cleanup() {
  git worktree remove --force "$TMP_DIR" >/dev/null 2>&1 || rm -rf "$TMP_DIR"
}
trap cleanup EXIT

# Checkout a detached worktree at HEAD and strip private-only packaging files
git worktree add --detach "$TMP_DIR" HEAD >/dev/null
pushd "$TMP_DIR" >/dev/null

rm -rf snap .snapcraftignore
git add -A

TREE_SHA="$(git write-tree)"
PARENT_SHA="$(git rev-parse "${REMOTE}/main")"
COMMIT_MSG="$(git log -1 --pretty=%B HEAD)"

NEW_COMMIT="$(git commit-tree "$TREE_SHA" -p "$PARENT_SHA" -m "$COMMIT_MSG")"
git push "$REMOTE" "${NEW_COMMIT}:refs/heads/main"

if [[ -n "$TAG" ]]; then
  git push -f "$REMOTE" "${NEW_COMMIT}:refs/tags/${TAG}"
fi

popd >/dev/null
echo "Synced to ${REMOTE}/main (${NEW_COMMIT})${TAG:+ and tag ${TAG}}."
