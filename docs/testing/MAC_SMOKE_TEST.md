# macOS Smoke Test Guide

## 1. Preflight

Run:

```bash
bash scripts/macos/preflight.sh
```

Fix all `[FAIL]` items first.

## 2. Create report file

Run:

```bash
bash scripts/macos/new-smoke-report.sh
```

This creates a timestamped report in `test-reports/`.

## 3. Run app

```bash
npm install
npm run tauri dev
```

Use the report checklist while testing.

## 4. Attach evidence

For any failed item, include exact steps and at least one screenshot or log snippet path in the report.

## 5. Commit test evidence

```bash
bash scripts/macos/commit-smoke-report.sh test-reports/mac-smoke-YYYYMMDD-HHMMSS.md
```
