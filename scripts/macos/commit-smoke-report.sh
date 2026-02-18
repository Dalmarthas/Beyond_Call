#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "Usage: bash scripts/macos/commit-smoke-report.sh test-reports/mac-smoke-YYYYMMDD-HHMMSS.md"
  exit 1
fi

REPORT_PATH="$1"
if [ ! -f "$REPORT_PATH" ]; then
  echo "Report file not found: $REPORT_PATH"
  exit 1
fi

TARGET_COMMIT="$(grep -E '^- Commit:' "$REPORT_PATH" | head -n 1 | awk '{print $3}' || true)"
if [ -z "$TARGET_COMMIT" ]; then
  TARGET_COMMIT="unknown"
fi

DATE_TAG="$(date +"%Y-%m-%d")"

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Working tree has changes. Only smoke-report-related files will be staged by this script."
fi

git add "$REPORT_PATH" docs/testing/MAC_SMOKE_TEST.md scripts/macos/preflight.sh scripts/macos/new-smoke-report.sh scripts/macos/commit-smoke-report.sh README.md package.json test-reports/.gitkeep

git commit -m "test(mac): smoke report ${DATE_TAG} for ${TARGET_COMMIT}"

echo "Committed smoke test evidence."
