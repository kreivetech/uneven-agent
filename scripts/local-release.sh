#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────────────
#  local-release.sh — Uneven AI release trigger
#
#  This script validates the code and pushes a tag to trigger Azure DevOps.
#  The pipeline handles all builds (Linux CUDA, Windows CUDA, Mac Metal).
# ──────────────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

# ── Flags ─────────────────────────────────────────────────────────────────────
SKIP_TESTS=false
for arg in "$@"; do
  if [[ "$arg" == "--skip-tests" ]]; then
    SKIP_TESTS=true
  fi
done

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}[release]${RESET} $*"; }
success() { echo -e "${GREEN}[release]${RESET} ✓ $*"; }
warn()    { echo -e "${YELLOW}[release]${RESET} ⚠ $*"; }
die()     { echo -e "${RED}[release]${RESET} ✗ $*" >&2; exit 1; }

# ── Version ───────────────────────────────────────────────────────────────────
VERSION=$(node -p "require('./package.json').version")
TAG="v$VERSION"

# ── Prerequisites ─────────────────────────────────────────────────────────────
check_prereqs() {
  info "Checking prerequisites..."
  local missing=()
  command -v gh                        &>/dev/null || missing+=("gh CLI")
  command -v git                       &>/dev/null || missing+=("git")
  command -v npm                       &>/dev/null || missing+=("npm")

  if (( ${#missing[@]} > 0 )); then
    die "Missing prerequisites: ${missing[*]}"
  fi
  success "Prerequisites met"
}

# ── Pre-flight ─────────────────────────────────────────────────────────────────
preflight() {
  if [[ "$SKIP_TESTS" == "true" ]]; then
    warn "Skipping pre-flight checks (--skip-tests)"
    return
  fi

  info "TypeScript type check..."
  npx tsc --noEmit

  success "Pre-flight validation passed (tests will run in the cloud)"
}

# ── Trigger ────────────────────────────────────────────────────────────────────
trigger() {
  if ! git diff --quiet HEAD; then
    die "Uncommitted changes detected. Commit or stash before releasing."
  fi

  if git rev-parse "$TAG" >/dev/null 2>&1; then
    warn "Tag $TAG already exists locally. If you need to re-release:"
    warn "  git tag -d $TAG"
    die "Aborting."
  fi

  info "Pushing tag $TAG to origin..."
  git tag -a "$TAG" -m "Release $TAG"
  git push origin "$TAG"

  success "Tag $TAG pushed! Azure DevOps is starting the build (CUDA/Metal)."
}

# ── Main ───────────────────────────────────────────────────────────────────────
REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null || echo "kreivesler/uneven")

echo ""
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo -e "${BOLD}  Uneven AI — Release Trigger — $TAG${RESET}"
echo -e "${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo ""

check_prereqs
preflight
trigger

echo ""
echo -e "${GREEN}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo -e "${GREEN}${BOLD}  Trigger Sent! Monitor the build in Azure DevOps${RESET}"
echo -e "${GREEN}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
echo ""
echo "  Azure DevOps: https://dev.azure.com/$(echo "$REPO" | cut -d/ -f1)"
echo "  GitHub Release: https://github.com/$REPO/releases/tag/$TAG"
echo ""
