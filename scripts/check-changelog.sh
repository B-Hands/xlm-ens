#!/usr/bin/env bash
# Verify that any pull request modifying contract source includes a CHANGELOG.md update.
#
# Usage (in CI):
#   scripts/check-changelog.sh [base-ref]
#
# base-ref defaults to origin/main.  In GitHub Actions the caller should pass
# the merge-base so that force-pushed branches are handled correctly:
#
#   BASE=$(git merge-base origin/main HEAD)
#   scripts/check-changelog.sh "$BASE"
#
# Exit codes:
#   0  — no contract changes, or changelog was updated
#   1  — contract changed but CHANGELOG.md was not updated
#   2  — usage or environment error

set -euo pipefail

BASE="${1:-origin/main}"

# ── Collect changed file paths ───────────────────────────────────────────────
if ! CHANGED=$(git diff --name-only "${BASE}...HEAD" 2>/dev/null); then
  echo "error: could not diff against '${BASE}'. Make sure the base ref is fetched." >&2
  echo "  Try: git fetch origin && scripts/check-changelog.sh origin/main" >&2
  exit 2
fi

if [ -z "$CHANGED" ]; then
  echo "No changes detected relative to ${BASE}. Nothing to check."
  exit 0
fi

# ── Detect contract source changes ───────────────────────────────────────────
# Match any lib.rs directly under a contract crate's src/ directory.
CONTRACT_FILES=$(printf '%s\n' "$CHANGED" \
  | grep -E '^contracts/[^/]+/src/.*\.rs$' || true)

if [ -z "$CONTRACT_FILES" ]; then
  echo "No contract source changes detected — changelog check skipped."
  exit 0
fi

echo "Contract source changes detected:"
printf '  %s\n' $CONTRACT_FILES
echo ""

# ── Detect CHANGELOG.md update ───────────────────────────────────────────────
CHANGELOG_CHANGED=$(printf '%s\n' "$CHANGED" | grep -E '^CHANGELOG\.md$' || true)

if [ -z "$CHANGELOG_CHANGED" ]; then
  cat <<'EOF'
ERROR: Contract source was modified but CHANGELOG.md was not updated.

Every pull request that changes a contract interface (adds, removes, or
renames a function, type, error, or event) must include an entry under the
[Unreleased] section of CHANGELOG.md.

  1. Open CHANGELOG.md.
  2. Find the [Unreleased] section for the affected contract.
  3. Add an entry using one of: Added / Changed / Deprecated / Removed / Fixed.
  4. Follow the format used in the existing entries.

If your change is purely internal (test-only, comment, refactor with no
observable interface change), you may skip the changelog entry by adding the
label 'changelog-exempt' to your pull request and re-running this check.
(The check reads the label via the CHANGELOG_EXEMPT env variable:
  CHANGELOG_EXEMPT=true scripts/check-changelog.sh)

EOF
  if [ "${CHANGELOG_EXEMPT:-false}" = "true" ]; then
    echo "CHANGELOG_EXEMPT=true is set — skipping changelog requirement."
    exit 0
  fi
  exit 1
fi

echo "CHANGELOG.md updated — check passed."
exit 0
