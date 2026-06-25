#!/usr/bin/env bash
# Validate that a single commit message follows the Conventional Commits spec.
#
# Usage:
#   scripts/check-commit-message.sh "<commit message subject>"
#
# Exit codes:
#   0  — message is valid
#   1  — message does not match the conventional commit format
#   2  — usage error

set -euo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 \"<commit message subject>\"" >&2
  exit 2
fi

MSG="$1"

# ── Conventional Commit regex ────────────────────────────────────────────────
# Format: <type>[optional scope][optional !]: <description>
# Examples:
#   feat(registry): add bulk-register function
#   fix!: correct fee rounding in registrar
#   chore: update dependencies
TYPES="feat|fix|perf|refactor|docs|test|build|ci|chore|revert|security|style"
PATTERN="^(${TYPES})(\([a-z0-9_/-]+\))?(!)?: .+"

if [[ "$MSG" =~ $PATTERN ]]; then
  echo "OK: \"$MSG\""
  exit 0
fi

cat >&2 <<EOF
ERROR: Commit message does not follow the Conventional Commits specification.

  Got: "$MSG"

Expected format:
  <type>[optional scope][optional !]: <short description>

Allowed types: feat, fix, perf, refactor, docs, test, build, ci, chore,
               revert, security, style

Examples:
  feat(registry): add bulk-register endpoint
  fix(registrar): correct fee rounding for 1-character labels
  docs: update SDK quickstart guide
  chore!: drop support for wasm32-unknown-unknown target (BREAKING)
  ci: pin git-cliff to v2

Breaking changes must include a "!" before the colon, or a "BREAKING CHANGE:"
footer in the commit body.

Reference: https://www.conventionalcommits.org/
EOF

exit 1
