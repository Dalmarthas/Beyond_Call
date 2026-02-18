# AI Transcribe Local

Local-first desktop app (macOS + Windows) for recording calls, transcribing locally, and generating AI summaries/analyses/critiques.

## Implemented MVP

- Nested folders and entries (local SQLite metadata)
- Entry lifecycle: create, rename, soft delete, restore, purge
- Recording pipeline via `ffmpeg` from user-selected sources
- Post-call transcription via local Whisper executable (`whisper-cli` or `whisper`)
- AI actions via local Ollama (`summary`, `analysis`, `critique_recruitment`, `critique_sales`, `critique_cs`)
- Editable transcript and artifact content with revision history
- Transcript edits mark artifacts as stale
- One-click export to ZIP with `entry.md` + source audio
- Editable global critique prompts
- Local model name setting (default: `qwen3:8b`)

## Stack

- Desktop: Tauri + React + TypeScript
- Backend storage: SQLite (`rusqlite`) + local file tree
- AI runtime: local Ollama HTTP API (`http://127.0.0.1:11434`)

## Prerequisites

Install these before running:

- Node.js 20+
- Rust stable toolchain
- `ffmpeg` (and optional `ffprobe`)
- Whisper executable in PATH (`whisper-cli` preferred)
- Ollama running locally with your model pulled (example: `qwen3:8b`)

Audio loopback setup for mixed call capture:

- macOS: BlackHole (or equivalent)
- Windows: VB-CABLE (or equivalent)

## Run

```bash
npm install
npm run tauri dev
```

## macOS QA flow (recommended before release)

1. Run dependency/runtime preflight:

```bash
npm run qa:mac:preflight
```

2. Generate a timestamped smoke-test report template:

```bash
npm run qa:mac:report
```

3. Launch the app and execute the checklist in the generated report:

```bash
npm run tauri dev
```

4. Commit test evidence from `test-reports/` after execution.

Helper:

```bash
bash scripts/macos/commit-smoke-report.sh test-reports/mac-smoke-YYYYMMDD-HHMMSS.md
```

## Packaging

```bash
npm run tauri build
```

## Notes

- Recording source fields map directly to ffmpeg arguments (`format`, `input`).
- Default source presets in UI are macOS-oriented (`avfoundation`).
- On Windows, use `dshow` with appropriate device names.
- v1 does not include speaker diarization.
- v1 does not include chat-over-entry/folder/compare; this is future scope.
