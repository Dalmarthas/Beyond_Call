# Windows Port Plan (Derived from Existing Plan + Updated Plan)

Date: March 4, 2026
Branch: `codex/windows-port-planning`
Status: Planning complete, implementation not started

## 1) Source Plans Located

1. Original product plan: `docs/plans/PLAN.original.user.md`
2. Updated implementation plan: `docs/plans/PLAN.updated.2026-03-04.md`

## 2) Current App Architecture (As Implemented)

1. Frontend: React + TypeScript in `src/App.tsx`, `src/lib/api.ts`, `src/lib/types.ts`
2. Backend: Tauri + Rust command surface in `src-tauri/src/lib.rs`
3. Local storage: SQLite + file tree under app data dir (`ai-transcribe-local`)
4. Runtime dependencies:
5. `ffmpeg` for recording/device listing/merge/mix
6. `whisper-cli` or `whisper` for transcription
7. `ollama` for local LLM artifacts
8. macOS native system audio helper: `src-tauri/macos/screen_capture_audio.swift`

## 3) macOS-Specific Implementations To Replace or Generalize

1. Native system audio source is macOS-only:
2. `screencapturekit/system` source path (`src-tauri/src/lib.rs`)
3. Runtime Swift compile via `xcrun swiftc` (`ensure_sck_recorder_binary`)
4. Device listing path branches:
5. macOS uses ffmpeg `avfoundation`
6. Windows uses ffmpeg `dshow` (already present)
7. Pause/resume currently depends on Unix signals:
8. `set_process_paused` uses `kill -STOP/-CONT`
9. On non-Unix (Windows), pause/resume returns unsupported error
10. QA and preflight automation is mac-only:
11. `scripts/macos/*`
12. `docs/testing/MAC_SMOKE_TEST.md`
13. `package.json` has only `qa:mac:*` scripts
14. Several error/help strings are mac-specific and need Windows-aware messaging

## 4) Windows Readiness Matrix

1. Folder/entry CRUD + trash + revisions: Works on Windows (platform-agnostic SQL/filesystem code)
2. Device enumeration: Implemented for Windows (`dshow` parser exists)
3. Basic recording with ffmpeg sources: Implemented for Windows
4. Multi-source mix/append: Implemented (ffmpeg filter graphs)
5. Native system audio capture: Not applicable (mac-only by design)
6. Pause/resume: Broken on Windows (backend intentionally unsupported)
7. Startup source defaults: Suboptimal on Windows (defaults to mic-like source, not loopback+mic pair)
8. Device hints UX: Backend command exists but frontend does not display hints
9. Consent gate before recording: Missing on all platforms (still open from updated plan)
10. Windows preflight/smoke automation: Missing
11. Windows packaging/release checklist: Missing

## 5) Proposed Windows Solutions

### Milestone W1: Runtime Parity (High Priority)

1. Replace pause/resume implementation with cross-platform segment-based pause:
2. On pause: gracefully stop recorder and keep session open
3. On resume: start a new segment for same session
4. On stop: merge all segments in order
5. Remove Unix signal dependency for user-visible pause/resume behavior
6. Update startup source defaults to prefer loopback + microphone on Windows when available:
7. First source: loopback-marked device (VB-CABLE / Stereo Mix / Monitor)
8. Second source: mic-like non-loopback device
9. Keep current source picker override flow
10. Make recording errors platform-specific:
11. Windows errors should mention microphone privacy permissions and VB-CABLE/stereo mix routing
12. macOS errors keep ScreenCaptureKit privacy guidance
13. Fix custom whisper model path handling for Windows path separators (`\\` and relative `.\path` forms)
14. Gate pause/resume UI by backend capability so unsupported states are never exposed

### Milestone W2: UX and Setup Guidance

1. Surface `list_audio_device_hints` in UI under recording source controls
2. Add platform-aware setup copy:
3. Windows: VB-CABLE install + routing checklist
4. macOS: BlackHole/native system option guidance
5. Add consent gate modal before `start_recording` (required by updated plan)

### Milestone W3: Tooling, QA, and Release

1. Add Windows QA scripts:
2. `scripts/windows/preflight.ps1`
3. `scripts/windows/new-smoke-report.ps1`
4. Optional `scripts/windows/commit-smoke-report.ps1`
5. Add docs:
6. `docs/testing/WINDOWS_SMOKE_TEST.md`
7. Update README with Windows QA flow (parallel to mac flow)
8. Add `package.json` scripts:
9. `qa:win:preflight`, `qa:win:report`, `qa:win`
10. Add a Windows packaging checklist for Tauri build artifacts and installation prerequisites

### Milestone W4: Tests

1. Add Rust unit tests for Windows device parsing edge cases:
2. Duplicate DirectShow names
3. Quoted names with spaces/symbols
4. Loopback device classification markers
5. Add tests for new pause/resume segment logic
6. Add tests for Windows-first default source selection behavior

## 6) Implementation Order

1. Implement W1 first (runtime parity blockers)
2. Implement W2 next (operator guidance + compliance gate)
3. Implement W3 for reproducible QA and release flow
4. Implement W4 to prevent regressions before merge

## 7) Definition of Done for Windows Branch

1. Fresh Windows machine can complete preflight and launch app
2. User can record loopback+mic, pause/resume, stop, transcribe, generate artifacts, export
3. Source selection UX clearly guides VB-CABLE/stereo-mix routing
4. Consent confirmation is required before every recording start
5. Windows smoke report template exists and is used for evidence
6. No macOS-specific error/help text shown during Windows flows

## 8) Key Risks and Mitigations

1. Risk: Windows pause/resume at process-signal layer is unreliable or API-complex
2. Mitigation: Use segment-based pause/resume (backend-controlled, deterministic, cross-platform)
3. Risk: Loopback devices vary by driver naming and locale
4. Mitigation: Keep marker-based detection plus manual picker override
5. Risk: Device probing output differs across ffmpeg builds
6. Mitigation: Harden parser tests with representative fixture strings
