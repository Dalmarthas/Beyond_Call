# PRD / Delivery Plan: Local Call Recorder + AI Review Desktop App (macOS + Windows)

Last updated: March 4, 2026
Status: v1 core pipeline is implemented; compliance, platform hardening, and advanced workflows remain.

## 1) Current Product Summary
This app is now a local-first desktop product (Tauri + React) that records from selected audio sources, transcribes with local Whisper tooling, and generates local AI outputs (Ollama/Qwen): summary, analysis, and role-based critiques.

Compared with the original plan, the major architecture decisions are now concrete in code and should be treated as the source of truth below.

## 2) Architecture Decisions (Now Locked by Implementation)

### 2.1 Runtime and repo structure
1. App is implemented as a single Tauri workspace at `/Users/niberium/Documents/AI Transcribe`.
2. The previously proposed monorepo layout (`apps/desktop`, `packages/shared-types`) is not used.
3. Frontend/backend contracts are defined via Tauri commands and TypeScript types in-place (`src/lib/types.ts`, `src/lib/api.ts`).

### 2.2 Local storage model
1. Data root is OS app-data directory + `ai-transcribe-local` (not a fixed repo-relative data folder).
2. SQLite DB is `app.db` under that app-data root.
3. Entry file tree is under `entries/<entry_id>/` with `audio/`, `transcript/`, `artifacts/`, `exports/`.
4. Prompt templates and model settings are stored in SQLite (`prompt_templates`, `settings`).

### 2.3 Recording architecture
1. Source selection is explicit in UI and passed to backend (`format`, `input`).
2. macOS supports native system audio source via ScreenCaptureKit (`screencapturekit/system`).
3. Non-native and multi-source capture use ffmpeg.
4. Native system + mic capture is supported with OS/version guards; backend mixes tracks when needed.
5. Segmented recording append/merge behavior is implemented for repeated recording sessions.
6. Pause/resume is implemented via process signaling (`set_recording_paused`).
7. Recording meter telemetry is implemented (bytes + normalized level).

### 2.4 Transcription architecture
1. Two Whisper paths are supported:
   - `whisper-cli` for whisper.cpp/ggml `.bin` models.
   - `whisper` (OpenAI Whisper CLI) for standard model names.
2. Selected whisper model is user-configurable and persisted in settings.
3. Language auto-detection + normalization are implemented (including full-name mapping like `russian -> ru`).
4. Transcript revisions are versioned and persisted.

### 2.5 AI generation architecture
1. Ollama local endpoint (`127.0.0.1:11434`) is used.
2. Backend ensures readiness (auto-start attempt, model existence check, optional warmup).
3. If model is missing, backend starts background pull and returns status.
4. Artifact generation is per-entry, per-type, versioned, and linked to transcript version.

### 2.6 Export architecture
1. Export is a ZIP generated per entry.
2. ZIP includes `entry.md` and source audio file when present.
3. Markdown includes transcript + latest artifacts.

## 3) Scope Status vs Original v1 Goals

### 3.1 Implemented (Done)
1. Record calls from 2+ selected sources with pre-record configuration.
2. Nested folders + entries with CRUD and hierarchy.
3. Local transcript + summary + analysis + 3 critique modes.
4. Manual editing for transcript/artifacts.
5. Transcript edits mark artifacts stale; regeneration creates new revisions.
6. Soft-delete trash flow with restore + purge for folders and entries.
7. One-click export to ZIP (markdown + audio).
8. Local-only processing path (no cloud dependency).
9. Model and prompt template editing in settings.
10. UI redesign completed and merged to `main` (current branded UI with updated settings/trash UX).

### 3.2 Partially implemented / adjusted
1. Guided loopback setup wizard:
   - Device discovery and hints exist.
   - Full guided wizard UX for BlackHole/VB-CABLE is not implemented as a dedicated flow.
2. Proposed command interface changed slightly:
   - `pause_recording` + `resume_recording` are represented by `set_recording_paused(session_id, paused)`.
3. Data layout differs from original path proposal:
   - Uses app-data location instead of static repo data path.

### 3.3 Not implemented yet (still planned)
1. Mandatory consent confirmation gate before each recording session.
2. Settings sections for export defaults and legal/compliance text.
3. Platform hardening for Windows parity (device routing UX and acceptance evidence comparable to macOS).
4. Expanded automated verification suite:
   - Full functional E2E automation.
   - Performance baselines (e.g., 2-hour run) as repeatable automated checks.
5. Future chat scope remains deferred:
   - Entry chat -> folder chat -> cross-folder comparison.

## 4) Non-goals (still unchanged for v1)
1. Live transcription during recording.
2. Live AI generation while recording.
3. Speaker diarization.
4. Team collaboration/sync.

## 5) Quality / Privacy / Compliance Status
1. Local-only architecture is in place.
2. No app-level encryption layer is implemented (still OS-level storage trust).
3. Consent compliance gate is still outstanding and should be treated as blocking for broader release.
4. Language handling supports auto + manual override.

## 6) Current Backend Command Surface (Implemented)
1. `list_recording_devices`
2. `list_audio_device_hints`
3. `recording_meter`
4. `bootstrap_state`
5. `get_entry_bundle`
6. `create_folder`
7. `rename_folder`
8. `create_entry`
9. `rename_entry`
10. `move_to_trash`
11. `restore_from_trash`
12. `purge_entity`
13. `start_recording`
14. `set_recording_paused`
15. `stop_recording`
16. `transcribe_entry`
17. `generate_artifact`
18. `update_transcript`
19. `update_artifact`
20. `update_prompt_template`
21. `update_model_name`
22. `prepare_ai_backend`
23. `list_whisper_models`
24. `update_whisper_model`
25. `export_entry_markdown`

## 7) Recommended Next Milestones

### Milestone A: Release-readiness hardening (recommended next)
1. Add explicit consent modal/gate in frontend + persisted consent timestamp per recording action.
2. Add legal/compliance text management in settings.
3. Add smoke checklist for Windows and run parity pass.
4. Add regression tests for source selection and transcription language mapping paths.

### Milestone B: Reliability + observability
1. Add structured error categories surfaced to UI (permissions, missing runtime, model not ready, etc.).
2. Add background task progress indicators for long transcribe/generate/export operations.
3. Add recovery UX for interrupted sessions.

### Milestone C: Post-v1 roadmap
1. Entry-level chat over transcript + artifacts.
2. Folder-level retrieval/chat.
3. Cross-folder comparison flows.

## 8) Open Questions For Product Decision
1. Is mandatory consent now a release blocker (must-have before any wider rollout), or can it be deferred to v1.1?
2. For Windows v1, do we require full setup wizard UX, or is device picker + written guide acceptable?
3. Should we keep ZIP as the only export format, or add plain folder export (`entry.md` + audio) as an option?
4. For v1 close-out, which quality gate is required: manual smoke only, or minimum automated E2E coverage?

## 9) Suggested Acceptance Criteria for v1 Close
1. Consent gate implemented and tested.
2. macOS + Windows source selection and recording verified with documented test evidence.
3. Transcribe + artifact generation + stale/rerun behavior validated on at least 3 realistic recordings.
4. Export output validated for compatibility (Markdown opens cleanly, audio playable).
5. Critical-path regressions covered by repeatable tests.
