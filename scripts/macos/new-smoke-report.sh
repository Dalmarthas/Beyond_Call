#!/usr/bin/env bash
set -euo pipefail

REPORT_DIR="test-reports"
mkdir -p "$REPORT_DIR"

STAMP="$(date +"%Y%m%d-%H%M%S")"
REPORT_PATH="$REPORT_DIR/mac-smoke-${STAMP}.md"
COMMIT_SHA="$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")"

cat > "$REPORT_PATH" <<REPORT
# Mac Smoke Test Report

- Date: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
- Commit: ${COMMIT_SHA}
- Tester: 
- Machine: 
- macOS version: 

## Preflight

Run and paste output summary:

\`\`\`bash
bash scripts/macos/preflight.sh
\`\`\`

Outcome:
- [ ] PASS
- [ ] FAIL (include reason)

## Smoke Checklist

1. App launch
- [ ] \`npm install\` completed.
- [ ] \`npm run tauri dev\` launches app window.

2. Folder/entry flow
- [ ] Create root folder.
- [ ] Create nested subfolder.
- [ ] Create entry in selected folder.
- [ ] Rename folder and entry.

3. Recording flow
- [ ] Click **Detect Audio Inputs** and verify listed devices.
- [ ] Configure mic + loopback sources.
- [ ] Start recording (consent prompt shown).
- [ ] Stop recording, status becomes recorded.

4. AI pipeline
- [ ] Run **Transcribe** successfully.
- [ ] Run **Summarize** successfully.
- [ ] Run **Analyze** successfully.
- [ ] Run all 3 critique actions successfully.

5. Editing and stale behavior
- [ ] Edit transcript and save.
- [ ] Artifacts show stale state.
- [ ] Regenerate one artifact and verify new version.

6. Export
- [ ] Click **Export Markdown + Audio**.
- [ ] Export zip exists on disk.
- [ ] Zip contains \`entry.md\` and \`audio/original.*\`.

7. Trash flow
- [ ] Move entry to trash and restore.
- [ ] Move folder to trash and restore.

## Bugs / Findings

- Severity: 
- Steps to reproduce: 
- Expected result: 
- Actual result: 
- Screenshot/log path: 

## Final Sign-off

- [ ] Smoke test accepted for this commit
- [ ] Blocked, fixes required before release

REPORT

printf "Created report template: %s\n" "$REPORT_PATH"
printf "Next: fill it while testing, then commit it as evidence.\n"
