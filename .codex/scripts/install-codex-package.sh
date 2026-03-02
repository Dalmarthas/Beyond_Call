#!/usr/bin/env bash
set -euo pipefail

REPO_URL="${REPO_URL:-git@github.com:Dalmarthas/codex-package.git}"
BRANCH="${BRANCH:-codex/codex-only}"
TARGET_DIR="${1:-.}"

if ! command -v git >/dev/null 2>&1; then
  echo "git is required" >&2
  exit 1
fi

if ! command -v rsync >/dev/null 2>&1; then
  echo "rsync is required" >&2
  exit 1
fi

TARGET_DIR="$(cd "$TARGET_DIR" && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Installing Codex package into: $TARGET_DIR"
echo "Source: $REPO_URL ($BRANCH)"

git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$TMP_DIR/pkg"

mkdir -p "$TARGET_DIR/.agents" "$TARGET_DIR/.codex"
rsync -a "$TMP_DIR/pkg/.agents/" "$TARGET_DIR/.agents/"
rsync -a "$TMP_DIR/pkg/.codex/" "$TARGET_DIR/.codex/"

if [ ! -f "$TARGET_DIR/AGENTS.md" ]; then
  cp "$TMP_DIR/pkg/AGENTS.md" "$TARGET_DIR/AGENTS.md"
  echo "Added AGENTS.md"
else
  echo "AGENTS.md already exists; left unchanged."
  echo "If needed, compare with: $TMP_DIR/pkg/AGENTS.md"
fi

echo
echo "Installed:"
echo "  - $TARGET_DIR/.agents"
echo "  - $TARGET_DIR/.codex"
echo
echo "Next:"
echo "  1) Review AGENTS.md and .codex/SKILL_PRECEDENCE.md"
echo "  2) Commit these files in your project"
