# PRD: Local Call Recorder + AI Review Desktop App (macOS + Windows)

## Summary
Build a local-first desktop app (Tauri + React) that records calls from user-selected audio sources (mic + speaker/system path), transcribes locally (Whisper), and runs local AI actions (Qwen3 via Ollama): summarize, analyze, and role-based critique.  
v1 focuses on reliable capture-to-insight workflow with nested folders, editable artifacts, revision history, and one-click Markdown+audio export.  
Future “chat with entry/folder/compare” is feasible, but should be deferred until v1 storage/indexing is stable.

## Product Scope

### Goals (v1)
1. Record calls from 2+ selected sources with pre-recording configuration.
2. Organize work in unlimited nested folders and entries.
3. Generate transcript, summary, analysis, and 3 critique modes locally.
4. Let users manually edit transcript and AI outputs.
5. Export each entry as Markdown + original audio in one click.
6. Keep all data local, with no cloud dependency.

### Non-goals (v1)
1. Live transcription/analysis during recording.
2. Auto speaker diarization.
3. Team collaboration/sync.
4. Folder-level or cross-folder AI chat.

## Users and Core Jobs
1. Recruiters: review interview quality, candidate signals, interviewer performance.
2. Sales reps/leads: identify objections, missing discovery, next-step quality.
3. Customer success teams: detect churn risk, adoption blockers, handoff quality.

## UX and Functional Requirements

### Information Architecture
1. Left navigation: folder tree with unlimited nesting.
2. Main pane: selected folder entries list or selected entry detail.
3. Trash section: soft-deleted folders/entries with restore/purge.
4. Settings: model/runtime setup, role prompts, export defaults, legal text.

### Entry Lifecycle
1. Create folder/subfolder.
2. Create entry inside any folder.
3. Configure recording inputs before start.
4. Mandatory consent confirmation before each recording.
5. Record, pause/resume, stop.
6. Auto-transcribe after stop.
7. User triggers summarize/analyze/critique buttons.
8. User edits transcript or AI outputs.
9. Edited transcript marks AI artifacts as stale until rerun.
10. Reruns create timestamped revisions, latest shown by default.
11. One-click export to Markdown + audio.

### Critique Modes
1. Recruitment Head
2. Sales Head
3. Customer Success Lead  
Each has editable global prompt template in settings.

## Technical Architecture

### Stack
1. Desktop shell: Tauri (Rust backend + React/TypeScript frontend).
2. Local model runtime: Ollama (required for Qwen3; Whisper via local inference path integrated by backend).
3. Storage: human-readable files + lightweight local SQLite index.

### Proposed Repo Layout
1. `/Users/niberium/Documents/AI Transcribe/apps/desktop` (Tauri + React app)
2. `/Users/niberium/Documents/AI Transcribe/apps/desktop/src` (frontend UI/state)
3. `/Users/niberium/Documents/AI Transcribe/apps/desktop/src-tauri` (Rust commands/audio/AI orchestration)
4. `/Users/niberium/Documents/AI Transcribe/packages/shared-types` (TypeScript domain types/contracts)
5. `/Users/niberium/Documents/AI Transcribe/docs/PRD.md` (this finalized PRD)
6. `/Users/niberium/Documents/AI Transcribe/docs/prompts/defaults.md` (default role prompts)

### Recording Approach (v1)
1. Guided setup for loopback devices:
2. macOS: BlackHole-based route guidance.
3. Windows: VB-CABLE-based route guidance.
4. User selects input sources pre-recording (mic + loopback/system path).
5. Backend mixes selected streams into a single WAV/PCM master track for transcription.
6. Save original recording artifact per entry.

## Public Interfaces / Types (Decision-Complete)

### Core Entities
1. `Folder`: `id`, `parent_id|null`, `name`, `created_at`, `updated_at`, `deleted_at|null`.
2. `Entry`: `id`, `folder_id`, `title`, `status`, `duration_sec`, `created_at`, `updated_at`, `deleted_at|null`.
3. `RecordingConfig`: `input_devices[]`, `sample_rate`, `channels`, `consent_confirmed_at`.
4. `TranscriptRevision`: `id`, `entry_id`, `version`, `text`, `language`, `is_manual_edit`, `created_at`.
5. `ArtifactRevision`: `id`, `entry_id`, `type(summary|analysis|critique_recruitment|critique_sales|critique_cs)`, `version`, `text`, `source_transcript_version`, `is_stale`, `created_at`.
6. `PromptTemplate`: `id`, `role`, `prompt_text`, `updated_at`.

### Backend Command Contracts (Tauri)
1. `start_recording(config)` -> `session_id`
2. `pause_recording(session_id)` -> `ok`
3. `resume_recording(session_id)` -> `ok`
4. `stop_recording(session_id)` -> `recording_path`, `duration_sec`
5. `transcribe_entry(entry_id, options)` -> `transcript_revision_id`
6. `generate_artifact(entry_id, artifact_type)` -> `artifact_revision_id`
7. `update_transcript(entry_id, text)` -> `transcript_revision_id` + stale flags updated
8. `update_artifact(artifact_revision_id, text)` -> `artifact_revision_id`
9. `export_entry_markdown(entry_id)` -> `export_path`
10. `move_to_trash(entity_type, id)` -> `ok`
11. `restore_from_trash(entity_type, id)` -> `ok`
12. `purge(entity_type, id)` -> `ok`

## Data Storage Specification

### On-Disk Entry Structure
1. `/Users/niberium/Documents/AI Transcribe/data/<workspace>/folders/...`
2. `/Users/niberium/Documents/AI Transcribe/data/<workspace>/entries/<entry_id>/audio/original.wav`
3. `/Users/niberium/Documents/AI Transcribe/data/<workspace>/entries/<entry_id>/transcript/v001.md`
4. `/Users/niberium/Documents/AI Transcribe/data/<workspace>/entries/<entry_id>/artifacts/<type>/v001.md`
5. `/Users/niberium/Documents/AI Transcribe/data/<workspace>/entries/<entry_id>/exports/<timestamp>.zip`  
SQLite indexes IDs, relationships, status, stale flags, and timestamps.

## Export Specification
1. One-click export creates archive containing:
2. `entry.md` with transcript + latest summary/analysis/critiques.
3. `audio/original.wav`.
4. Markdown includes metadata block: entry title, duration, created date, transcript version, artifact versions.

## Quality, Privacy, and Compliance
1. Local-only processing by default; no cloud API calls.
2. No app-level encryption in v1 (relies on OS account/disk security).
3. Mandatory consent confirmation before every recording session.
4. Language: Whisper auto-detect with user override option.
5. Target support: recordings up to 2 hours on typical modern laptops.

## Future Feature Feasibility
1. Entry chat is feasible as v1.5 if transcript/artifact indexing is stable.
2. Folder chat and cross-folder comparison are feasible but higher complexity due to retrieval strategy, context limits, and UI for scoped selection.
3. Recommendation: sequence as Entry chat -> Folder chat -> Comparison.

## Test Plan and Acceptance Scenarios

### Functional Tests
1. Create/edit/delete/restore nested folders at 3+ depth levels.
2. Create entry in nested folder, move entry, and verify index consistency.
3. Record with mic + loopback, stop, and verify playable audio saved.
4. Transcribe recording and verify transcript revision creation.
5. Edit transcript and verify all derived artifacts become stale.
6. Regenerate one artifact and verify new version linked to latest transcript.
7. Edit artifact text manually and verify revision tracking.
8. Export entry and verify markdown + audio present and readable.
9. Soft delete folder with entries, restore, and validate full recovery.

### Platform Tests
1. macOS setup wizard with BlackHole path and permission flow.
2. Windows setup wizard with VB-CABLE path and permission flow.
3. Device selection persistence across app restarts.

### Performance Tests
1. 2-hour recording does not crash and saves complete audio.
2. Post-record transcription completes within acceptable local baseline for chosen Whisper model.
3. Artifact generation remains responsive with progress state.

## Assumptions and Defaults Locked
1. v1 stack: Tauri + React.
2. v1 AI runtime: Ollama integration.
3. v1 scope: core pipeline only (no chat features).
4. Capture mode: guided loopback setup.
5. Data model: files + SQLite index.
6. Critique prompts: globally editable per role.
7. Sharing default: Markdown + original audio.
8. Transcript edits invalidate AI outputs; user reruns manually.
9. Revision history retained for regenerated artifacts.
10. Soft delete with Trash.
11. No speaker labels in v1.
12. Auto language detection.
13. Mandatory consent gate each recording.
14. Batch processing after recording (no live AI).
