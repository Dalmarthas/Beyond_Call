use chrono::Utc;
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{Manager, State};
use uuid::Uuid;
use zip::write::FileOptions;

const MODEL_NAME_KEY: &str = "model_name";
const DEFAULT_MODEL_NAME: &str = "qwen3:8b";
#[cfg(target_os = "macos")]
const SCK_RECORDER_SWIFT: &str = include_str!("../macos/screen_capture_audio.swift");

struct AppState {
    sessions: Mutex<HashMap<String, RecordingSession>>,
    data_dir: PathBuf,
    db_path: PathBuf,
}

struct RecordingSession {
    entry_id: String,
    output_path: PathBuf,
    existing_path: Option<PathBuf>,
    child: Child,
    telemetry: Arc<Mutex<RecordingTelemetry>>,
    paused: bool,
}

#[derive(Debug, Default)]
struct RecordingTelemetry {
    bytes_written: u64,
    level: f32,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Folder {
    id: String,
    parent_id: Option<String>,
    name: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    id: String,
    folder_id: String,
    title: String,
    status: String,
    duration_sec: i64,
    recording_path: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranscriptRevision {
    id: String,
    entry_id: String,
    version: i64,
    text: String,
    language: String,
    is_manual_edit: bool,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArtifactRevision {
    id: String,
    entry_id: String,
    artifact_type: String,
    version: i64,
    text: String,
    source_transcript_version: i64,
    is_stale: bool,
    is_manual_edit: bool,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PromptTemplate {
    role: String,
    prompt_text: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BootstrapState {
    folders: Vec<Folder>,
    entries: Vec<Entry>,
    prompt_templates: Vec<PromptTemplate>,
    model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntryBundle {
    transcript_revisions: Vec<TranscriptRevision>,
    artifact_revisions: Vec<ArtifactRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordingSource {
    label: String,
    format: String,
    input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordingDevice {
    name: String,
    format: String,
    input: String,
    is_loopback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordingMeter {
    bytes_written: u64,
    level: f32,
}

fn now_ts() -> String {
    Utc::now().to_rfc3339()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

fn data_dir(state: &State<'_, AppState>) -> Result<PathBuf, String> {
    Ok(state.data_dir.clone())
}

fn db_path(state: &State<'_, AppState>) -> Result<PathBuf, String> {
    Ok(state.db_path.clone())
}

fn connection(path: &Path) -> Result<Connection, String> {
    Connection::open(path).map_err(|e| format!("Failed to open database: {e}"))
}

fn init_database(db_path: &Path) -> Result<(), String> {
    let conn = connection(db_path)?;
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS folders (
            id TEXT PRIMARY KEY,
            parent_id TEXT NULL,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT NULL
        );

        CREATE TABLE IF NOT EXISTS entries (
            id TEXT PRIMARY KEY,
            folder_id TEXT NOT NULL,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            duration_sec INTEGER NOT NULL DEFAULT 0,
            recording_path TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT NULL,
            FOREIGN KEY(folder_id) REFERENCES folders(id)
        );

        CREATE TABLE IF NOT EXISTS transcript_revisions (
            id TEXT PRIMARY KEY,
            entry_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            text TEXT NOT NULL,
            language TEXT NOT NULL,
            is_manual_edit INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(entry_id) REFERENCES entries(id)
        );

        CREATE TABLE IF NOT EXISTS artifact_revisions (
            id TEXT PRIMARY KEY,
            entry_id TEXT NOT NULL,
            artifact_type TEXT NOT NULL,
            version INTEGER NOT NULL,
            text TEXT NOT NULL,
            source_transcript_version INTEGER NOT NULL,
            is_stale INTEGER NOT NULL,
            is_manual_edit INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(entry_id) REFERENCES entries(id)
        );

        CREATE TABLE IF NOT EXISTS prompt_templates (
            role TEXT PRIMARY KEY,
            prompt_text TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_entries_folder ON entries(folder_id);
        CREATE INDEX IF NOT EXISTS idx_entries_deleted ON entries(deleted_at);
        CREATE INDEX IF NOT EXISTS idx_transcript_entry_version ON transcript_revisions(entry_id, version DESC);
        CREATE INDEX IF NOT EXISTS idx_artifact_entry_type_version ON artifact_revisions(entry_id, artifact_type, version DESC);
        "#,
    )
    .map_err(|e| format!("Failed to initialize schema: {e}"))?;

    seed_defaults(&conn)?;
    Ok(())
}

fn seed_defaults(conn: &Connection) -> Result<(), String> {
    let now = now_ts();
    let defaults = vec![
        (
            "summary",
            "Create a concise markdown summary of this call. Include goals, what happened, and next actions.",
        ),
        (
            "analysis",
            "Analyze this call in markdown. Cover communication quality, risks, strengths, and concrete improvements.",
        ),
        (
            "critique_recruitment",
            "You are a Recruitment Head. Critique the interview quality, question depth, candidate signal quality, and hiring recommendation clarity.",
        ),
        (
            "critique_sales",
            "You are a Sales Head. Critique discovery quality, objection handling, value articulation, and deal progression discipline.",
        ),
        (
            "critique_cs",
            "You are a Customer Success Lead. Critique retention risk detection, expectation management, adoption coaching, and next-step ownership.",
        ),
    ];

    for (role, prompt) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO prompt_templates(role, prompt_text, updated_at) VALUES(?1, ?2, ?3)",
            params![role, prompt, now],
        )
        .map_err(|e| format!("Failed to seed prompts: {e}"))?;
    }

    conn.execute(
        "INSERT OR IGNORE INTO settings(key, value, updated_at) VALUES(?1, ?2, ?3)",
        params![MODEL_NAME_KEY, DEFAULT_MODEL_NAME, now],
    )
    .map_err(|e| format!("Failed to seed settings: {e}"))?;

    Ok(())
}

fn ensure_entry_dirs(base_data_dir: &Path, entry_id: &str) -> Result<PathBuf, String> {
    let entry_dir = base_data_dir.join("entries").join(entry_id);
    fs::create_dir_all(entry_dir.join("audio")).map_err(|e| format!("Failed to create audio dir: {e}"))?;
    fs::create_dir_all(entry_dir.join("transcript"))
        .map_err(|e| format!("Failed to create transcript dir: {e}"))?;
    fs::create_dir_all(entry_dir.join("artifacts"))
        .map_err(|e| format!("Failed to create artifacts dir: {e}"))?;
    fs::create_dir_all(entry_dir.join("exports")).map_err(|e| format!("Failed to create exports dir: {e}"))?;
    Ok(entry_dir)
}

fn entry_dir(base_data_dir: &Path, entry_id: &str) -> PathBuf {
    base_data_dir.join("entries").join(entry_id)
}

fn get_next_transcript_version(conn: &Connection, entry_id: &str) -> Result<i64, String> {
    let mut stmt = conn
        .prepare("SELECT COALESCE(MAX(version), 0) + 1 FROM transcript_revisions WHERE entry_id = ?1")
        .map_err(|e| format!("Failed to prepare transcript version query: {e}"))?;
    stmt.query_row(params![entry_id], |row| row.get(0))
        .map_err(|e| format!("Failed to query transcript version: {e}"))
}

fn get_next_artifact_version(conn: &Connection, entry_id: &str, artifact_type: &str) -> Result<i64, String> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM artifact_revisions WHERE entry_id = ?1 AND artifact_type = ?2",
        )
        .map_err(|e| format!("Failed to prepare artifact version query: {e}"))?;
    stmt.query_row(params![entry_id, artifact_type], |row| row.get(0))
        .map_err(|e| format!("Failed to query artifact version: {e}"))
}

fn latest_transcript(conn: &Connection, entry_id: &str) -> Result<Option<TranscriptRevision>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, entry_id, version, text, language, is_manual_edit, created_at
             FROM transcript_revisions
             WHERE entry_id = ?1
             ORDER BY version DESC
             LIMIT 1",
        )
        .map_err(|e| format!("Failed to prepare latest transcript query: {e}"))?;

    let mut rows = stmt
        .query(params![entry_id])
        .map_err(|e| format!("Failed to execute latest transcript query: {e}"))?;

    if let Some(row) = rows.next().map_err(|e| format!("Failed to read latest transcript row: {e}"))? {
        Ok(Some(TranscriptRevision {
            id: row.get(0).map_err(|e| e.to_string())?,
            entry_id: row.get(1).map_err(|e| e.to_string())?,
            version: row.get(2).map_err(|e| e.to_string())?,
            text: row.get(3).map_err(|e| e.to_string())?,
            language: row.get(4).map_err(|e| e.to_string())?,
            is_manual_edit: row.get::<_, i64>(5).map_err(|e| e.to_string())? == 1,
            created_at: row.get(6).map_err(|e| e.to_string())?,
        }))
    } else {
        Ok(None)
    }
}

fn latest_artifact_by_type(conn: &Connection, entry_id: &str, artifact_type: &str) -> Result<Option<ArtifactRevision>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, entry_id, artifact_type, version, text, source_transcript_version, is_stale, is_manual_edit, created_at
             FROM artifact_revisions
             WHERE entry_id = ?1 AND artifact_type = ?2
             ORDER BY version DESC
             LIMIT 1",
        )
        .map_err(|e| format!("Failed to prepare latest artifact query: {e}"))?;

    let mut rows = stmt
        .query(params![entry_id, artifact_type])
        .map_err(|e| format!("Failed to execute latest artifact query: {e}"))?;

    if let Some(row) = rows.next().map_err(|e| format!("Failed to read latest artifact row: {e}"))? {
        Ok(Some(ArtifactRevision {
            id: row.get(0).map_err(|e| e.to_string())?,
            entry_id: row.get(1).map_err(|e| e.to_string())?,
            artifact_type: row.get(2).map_err(|e| e.to_string())?,
            version: row.get(3).map_err(|e| e.to_string())?,
            text: row.get(4).map_err(|e| e.to_string())?,
            source_transcript_version: row.get(5).map_err(|e| e.to_string())?,
            is_stale: row.get::<_, i64>(6).map_err(|e| e.to_string())? == 1,
            is_manual_edit: row.get::<_, i64>(7).map_err(|e| e.to_string())? == 1,
            created_at: row.get(8).map_err(|e| e.to_string())?,
        }))
    } else {
        Ok(None)
    }
}

fn validate_artifact_type(artifact_type: &str) -> Result<(), String> {
    match artifact_type {
        "summary" | "analysis" | "critique_recruitment" | "critique_sales" | "critique_cs" => Ok(()),
        _ => Err(format!("Invalid artifact type: {artifact_type}")),
    }
}

fn validate_prompt_role(role: &str) -> Result<(), String> {
    validate_artifact_type(role)
}

fn model_name(conn: &Connection) -> Result<String, String> {
    let mut stmt = conn
        .prepare("SELECT value FROM settings WHERE key = ?1")
        .map_err(|e| format!("Failed to prepare model name query: {e}"))?;

    let result: Result<String, _> = stmt.query_row(params![MODEL_NAME_KEY], |row| row.get(0));
    Ok(result.unwrap_or_else(|_| DEFAULT_MODEL_NAME.to_string()))
}

fn prompt_for_role(conn: &Connection, role: &str) -> Result<String, String> {
    let mut stmt = conn
        .prepare("SELECT prompt_text FROM prompt_templates WHERE role = ?1")
        .map_err(|e| format!("Failed to prepare prompt query: {e}"))?;
    let result: Result<String, _> = stmt.query_row(params![role], |row| row.get(0));

    Ok(result.unwrap_or_else(|_| match role {
        "summary" => "Create a concise markdown summary of this call.".to_string(),
        "analysis" => "Analyze this call in markdown with strengths, risks, and improvements.".to_string(),
        "critique_recruitment" => "Critique this call as Recruitment Head in markdown.".to_string(),
        "critique_sales" => "Critique this call as Sales Head in markdown.".to_string(),
        "critique_cs" => "Critique this call as Customer Success Lead in markdown.".to_string(),
        _ => "Analyze this call.".to_string(),
    }))
}

fn ensure_entry_exists(conn: &Connection, entry_id: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM entries WHERE id = ?1 AND deleted_at IS NULL")
        .map_err(|e| format!("Failed to prepare entry existence query: {e}"))?;
    let count: i64 = stmt
        .query_row(params![entry_id], |row| row.get(0))
        .map_err(|e| format!("Failed to run entry existence query: {e}"))?;

    if count == 0 {
        return Err("Entry not found or deleted".to_string());
    }

    Ok(())
}

fn ensure_folder_exists(conn: &Connection, folder_id: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT COUNT(*) FROM folders WHERE id = ?1 AND deleted_at IS NULL")
        .map_err(|e| format!("Failed to prepare folder existence query: {e}"))?;
    let count: i64 = stmt
        .query_row(params![folder_id], |row| row.get(0))
        .map_err(|e| format!("Failed to run folder existence query: {e}"))?;

    if count == 0 {
        return Err("Folder not found or deleted".to_string());
    }

    Ok(())
}

fn descendant_folder_ids(conn: &Connection, root_folder_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE folder_tree(id) AS (
                SELECT id FROM folders WHERE id = ?1
                UNION ALL
                SELECT f.id
                FROM folders f
                JOIN folder_tree t ON f.parent_id = t.id
            )
            SELECT id FROM folder_tree",
        )
        .map_err(|e| format!("Failed to prepare folder recursion query: {e}"))?;

    let rows = stmt
        .query_map(params![root_folder_id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to read descendant folder ids: {e}"))?;

    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(|e| format!("Failed to parse descendant row: {e}"))?);
    }

    Ok(ids)
}

fn entry_ids_for_folder_ids(conn: &Connection, folder_ids: &[String]) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    let mut stmt = conn
        .prepare("SELECT id FROM entries WHERE folder_id = ?1")
        .map_err(|e| format!("Failed to prepare entry by folder query: {e}"))?;

    for folder_id in folder_ids {
        let rows = stmt
            .query_map(params![folder_id], |row| row.get::<_, String>(0))
            .map_err(|e| format!("Failed to query entries for folder: {e}"))?;
        for row in rows {
            ids.push(row.map_err(|e| format!("Failed to parse entry id row: {e}"))?);
        }
    }

    Ok(ids)
}

fn find_executable(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn probe_duration_seconds(recording_path: &str) -> i64 {
    if !find_executable("ffprobe") {
        return 0;
    }

    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(recording_path)
        .output();

    if let Ok(result) = output {
        if let Ok(text) = String::from_utf8(result.stdout) {
            if let Ok(value) = text.trim().parse::<f64>() {
                return value.round() as i64;
            }
        }
    }

    0
}

#[cfg(target_os = "macos")]
fn macos_version_major() -> Option<u32> {
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?;
    let major = value.trim().split('.').next()?.parse::<u32>().ok()?;
    Some(major)
}

#[cfg(target_os = "macos")]
fn supports_native_system_audio_capture() -> bool {
    macos_version_major().map(|major| major >= 13).unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn supports_native_system_audio_capture() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn ensure_sck_recorder_binary(base_data_dir: &Path) -> Result<PathBuf, String> {
    let bin_dir = base_data_dir.join("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create helper directory {}: {e}", bin_dir.display()))?;

    let source_path = bin_dir.join("screen_capture_audio.swift");
    fs::write(&source_path, SCK_RECORDER_SWIFT)
        .map_err(|e| format!("Failed to write ScreenCaptureKit helper source: {e}"))?;

    let binary_path = bin_dir.join("screen_capture_audio");
    let should_compile = if !binary_path.exists() {
        true
    } else {
        let source_mtime = fs::metadata(&source_path)
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let binary_mtime = fs::metadata(&binary_path)
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        source_mtime > binary_mtime
    };

    if should_compile {
        let output = Command::new("xcrun")
            .arg("swiftc")
            .arg("-parse-as-library")
            .arg(&source_path)
            .arg("-o")
            .arg(&binary_path)
            .output()
            .map_err(|e| format!("Failed to run Swift compiler for ScreenCaptureKit helper: {e}"))?;

        if !output.status.success() {
            let stderr_text = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Failed to compile native system-audio helper. Ensure Xcode Command Line Tools are installed. Details: {stderr_text}"
            ));
        }
    }

    Ok(binary_path)
}

fn spawn_recording_telemetry(stderr: impl std::io::Read + Send + 'static, telemetry: Arc<Mutex<RecordingTelemetry>>) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(value) = line.strip_prefix("sck_error=") {
                if let Ok(mut state) = telemetry.lock() {
                    state.last_error = Some(value.trim().to_string());
                }
                continue;
            }

            if let Some(value) = line.strip_prefix("total_size=") {
                if let Ok(bytes) = value.trim().parse::<u64>() {
                    if let Ok(mut state) = telemetry.lock() {
                        state.bytes_written = bytes;
                    }
                }
                continue;
            }

            if let Some(value) = line.strip_prefix("out_time_us=") {
                if let Ok(micros) = value.trim().parse::<u64>() {
                    let estimated = estimated_pcm_bytes_from_us(micros);
                    if let Ok(mut state) = telemetry.lock() {
                        if estimated > state.bytes_written {
                            state.bytes_written = estimated;
                        }
                    }
                }
                continue;
            }

            if let Some(value) = line.strip_prefix("level=") {
                if let Ok(level) = value.trim().parse::<f32>() {
                    if let Ok(mut state) = telemetry.lock() {
                        state.level = (state.level * 0.6 + level * 0.4).clamp(0.0, 1.0);
                    }
                }
                continue;
            }

            if let Some(pos) = line.find("lavfi.astats.Overall.RMS_level=") {
                let value = &line[(pos + "lavfi.astats.Overall.RMS_level=".len())..];
                let trimmed = value.trim();
                let mapped = if trimmed.eq_ignore_ascii_case("-inf") {
                    0.0
                } else if let Ok(db) = trimmed.parse::<f32>() {
                    rms_db_to_level(db)
                } else {
                    continue;
                };
                if let Ok(mut state) = telemetry.lock() {
                    state.level = (state.level * 0.6 + mapped * 0.4).clamp(0.0, 1.0);
                }
            }
        }
    });
}

fn wait_for_recorder_shutdown(child: &mut Child) {
    for _ in 0..30 {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => return,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
}

fn concat_recordings(first: &Path, second: &Path, output: &Path) -> Result<(), String> {
    let out = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(first)
        .arg("-i")
        .arg(second)
        .arg("-filter_complex")
        .arg("[0:a][1:a]concat=n=2:v=0:a=1[a]")
        .arg("-map")
        .arg("[a]")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg(output)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg concat: {e}"))?;

    if !out.status.success() {
        let stderr_text = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Failed to append recording segments: {stderr_text}"));
    }

    Ok(())
}

fn set_process_paused(pid: u32, paused: bool) -> Result<(), String> {
    #[cfg(unix)]
    {
        let signal = if paused { "-STOP" } else { "-CONT" };
        let status = Command::new("kill")
            .arg(signal)
            .arg(pid.to_string())
            .status()
            .map_err(|e| format!("Failed to send pause signal: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("Failed to update recording pause state".to_string())
        }
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        let _ = paused;
        Err("Pause/resume is currently supported on macOS/Linux only".to_string())
    }
}

fn resolve_whisper_model_path(base_data_dir: &Path) -> Result<PathBuf, String> {
    let min_model_bytes = 10 * 1024 * 1024_u64;

    let validate_model = |path: &Path| -> Result<bool, String> {
        if !path.exists() {
            return Ok(false);
        }
        let metadata = fs::metadata(path)
            .map_err(|e| format!("Failed to inspect whisper model at {}: {e}", path.display()))?;
        if metadata.len() < min_model_bytes {
            return Err(format!(
                "Whisper model at {} looks invalid ({} bytes). Install a real model with `bash scripts/macos/install-whisper-model.sh`.",
                path.display(),
                metadata.len()
            ));
        }
        Ok(true)
    };

    if let Ok(explicit) = std::env::var("WHISPER_MODEL_PATH") {
        let candidate = PathBuf::from(explicit);
        if validate_model(&candidate)? {
            return Ok(candidate);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    // Prefer multilingual models for language auto-detection.
    candidates.push(base_data_dir.join("models").join("ggml-base.bin"));
    candidates.push(base_data_dir.join("models").join("ggml-tiny.bin"));
    candidates.push(base_data_dir.join("models").join("ggml-base.en.bin"));
    candidates.push(base_data_dir.join("models").join("ggml-tiny.en.bin"));

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("models").join("ggml-base.bin"));
        candidates.push(cwd.join("models").join("ggml-tiny.bin"));
        candidates.push(cwd.join("models").join("ggml-base.en.bin"));
        candidates.push(cwd.join("models").join("ggml-tiny.en.bin"));
        candidates.push(cwd.join("..").join("models").join("ggml-base.bin"));
        candidates.push(cwd.join("..").join("models").join("ggml-tiny.bin"));
        candidates.push(cwd.join("..").join("models").join("ggml-base.en.bin"));
        candidates.push(cwd.join("..").join("models").join("ggml-tiny.en.bin"));
    }

    for candidate in candidates {
        if validate_model(&candidate)? {
            return Ok(candidate);
        }
    }

    Err(
        "No valid whisper model found. Set WHISPER_MODEL_PATH or place ggml-base.bin / ggml-tiny.bin (or *.en variants) in ./models/ (install via `bash scripts/macos/install-whisper-model.sh`).".to_string(),
    )
}

fn parse_whisper_detected_language(stderr_text: &str) -> Option<String> {
    let marker = "auto-detected language:";
    for line in stderr_text.lines() {
        let lower = line.to_lowercase();
        let Some(pos) = lower.find(marker) else {
            continue;
        };
        let suffix = lower[(pos + marker.len())..].trim();
        let lang: String = suffix
            .chars()
            .take_while(|ch| ch.is_ascii_alphabetic() || *ch == '-')
            .collect();
        if (2..=8).contains(&lang.len()) {
            return Some(lang);
        }
    }
    None
}

fn call_ollama(model_name: &str, prompt: &str) -> Result<String, String> {
    let client = Client::new();
    let response = client
        .post("http://127.0.0.1:11434/api/generate")
        .json(&json!({
            "model": model_name,
            "prompt": prompt,
            "stream": false
        }))
        .send()
        .map_err(|e| {
            format!(
                "Failed to call Ollama at http://127.0.0.1:11434. Ensure Ollama is running locally. Error: {e}"
            )
        })?;

    if !response.status().is_success() {
        return Err(format!("Ollama request failed with status {}", response.status()));
    }

    let body: serde_json::Value = response
        .json()
        .map_err(|e| format!("Failed to parse Ollama response: {e}"))?;

    body.get("response")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| "Ollama response missing `response` text".to_string())
}

fn is_loopback_device_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    let loopback_markers = [
        "blackhole",
        "loopback",
        "soundflower",
        "vb-cable",
        "stereo mix",
        "monitor of",
    ];
    loopback_markers
        .iter()
        .any(|marker| lower.contains(marker))
}

fn parse_macos_recording_devices(joined_output: &str) -> Vec<RecordingDevice> {
    let mut devices = Vec::new();
    let mut in_audio_section = false;

    for line in joined_output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("AVFoundation audio devices") {
            in_audio_section = true;
            continue;
        }
        if trimmed.contains("AVFoundation video devices") {
            in_audio_section = false;
            continue;
        }
        if !in_audio_section {
            continue;
        }

        let Some(marker) = trimmed.rfind("] [") else {
            continue;
        };
        let rest = &trimmed[(marker + 3)..];
        let Some(end_index_marker) = rest.find("] ") else {
            continue;
        };

        let index = rest[..end_index_marker].trim();
        let name = rest[(end_index_marker + 2)..].trim();
        if index.is_empty() || name.is_empty() {
            continue;
        }

        devices.push(RecordingDevice {
            name: name.to_string(),
            format: "avfoundation".to_string(),
            input: format!(":{index}"),
            is_loopback: is_loopback_device_name(name),
        });
    }

    devices
}

fn parse_windows_recording_devices(joined_output: &str) -> Vec<RecordingDevice> {
    let mut devices = Vec::new();
    let mut in_audio_section = false;

    for line in joined_output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("DirectShow audio devices") {
            in_audio_section = true;
            continue;
        }
        if trimmed.contains("DirectShow video devices") {
            in_audio_section = false;
            continue;
        }
        if !in_audio_section || trimmed.contains("Alternative name") {
            continue;
        }

        let Some(first_quote) = trimmed.find('"') else {
            continue;
        };
        let remainder = &trimmed[(first_quote + 1)..];
        let Some(second_quote) = remainder.find('"') else {
            continue;
        };

        let name = remainder[..second_quote].trim();
        if name.is_empty() {
            continue;
        }

        let exists = devices
            .iter()
            .any(|item: &RecordingDevice| item.name.eq_ignore_ascii_case(name));
        if exists {
            continue;
        }

        devices.push(RecordingDevice {
            name: name.to_string(),
            format: "dshow".to_string(),
            input: format!("audio={name}"),
            is_loopback: is_loopback_device_name(name),
        });
    }

    devices
}

fn estimated_pcm_bytes_from_us(out_time_us: u64) -> u64 {
    // 16kHz * 1 channel * s16 (2 bytes)
    44 + (out_time_us.saturating_mul(32_000) / 1_000_000)
}

fn rms_db_to_level(db: f32) -> f32 {
    // Treat -55 dB as silence and -10 dB as strong signal.
    ((db + 55.0) / 45.0).clamp(0.0, 1.0)
}

#[tauri::command]
fn list_recording_devices() -> Result<Vec<RecordingDevice>, String> {
    if !find_executable("ffmpeg") {
        return Err("ffmpeg not found in PATH".to_string());
    }

    let output = if cfg!(target_os = "macos") {
        Command::new("ffmpeg")
            .arg("-f")
            .arg("avfoundation")
            .arg("-list_devices")
            .arg("true")
            .arg("-i")
            .arg("")
            .output()
            .map_err(|e| format!("Failed to query ffmpeg avfoundation devices: {e}"))?
    } else if cfg!(target_os = "windows") {
        Command::new("ffmpeg")
            .arg("-list_devices")
            .arg("true")
            .arg("-f")
            .arg("dshow")
            .arg("-i")
            .arg("dummy")
            .output()
            .map_err(|e| format!("Failed to query ffmpeg dshow devices: {e}"))?
    } else {
        Command::new("ffmpeg")
            .arg("-sources")
            .arg("pulse")
            .output()
            .map_err(|e| format!("Failed to query ffmpeg audio sources: {e}"))?
    };

    let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout_text = String::from_utf8_lossy(&output.stdout).to_string();
    let joined = format!("{stderr_text}\n{stdout_text}");

    let mut devices = if cfg!(target_os = "macos") {
        parse_macos_recording_devices(&joined)
    } else if cfg!(target_os = "windows") {
        parse_windows_recording_devices(&joined)
    } else {
        Vec::new()
    };

    if cfg!(target_os = "macos") && supports_native_system_audio_capture() {
        devices.insert(
            0,
            RecordingDevice {
                name: "System Audio (macOS Native)".to_string(),
                format: "screencapturekit".to_string(),
                input: "system".to_string(),
                is_loopback: true,
            },
        );
    }

    if devices.is_empty() && cfg!(target_os = "macos") {
        devices.push(RecordingDevice {
            name: "Default Microphone".to_string(),
            format: "avfoundation".to_string(),
            input: ":0".to_string(),
            is_loopback: false,
        });
    }

    Ok(devices)
}

#[tauri::command]
fn list_audio_device_hints() -> Result<Vec<String>, String> {
    if !find_executable("ffmpeg") {
        return Err("ffmpeg not found in PATH".to_string());
    }

    let output = if cfg!(target_os = "macos") {
        Command::new("ffmpeg")
            .arg("-f")
            .arg("avfoundation")
            .arg("-list_devices")
            .arg("true")
            .arg("-i")
            .arg("")
            .output()
            .map_err(|e| format!("Failed to query ffmpeg avfoundation devices: {e}"))?
    } else if cfg!(target_os = "windows") {
        Command::new("ffmpeg")
            .arg("-list_devices")
            .arg("true")
            .arg("-f")
            .arg("dshow")
            .arg("-i")
            .arg("dummy")
            .output()
            .map_err(|e| format!("Failed to query ffmpeg dshow devices: {e}"))?
    } else {
        Command::new("ffmpeg")
            .arg("-sources")
            .arg("pulse")
            .output()
            .map_err(|e| format!("Failed to query ffmpeg audio sources: {e}"))?
    };

    let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout_text = String::from_utf8_lossy(&output.stdout).to_string();
    let joined = format!("{stderr_text}\n{stdout_text}");

    let mut hints = Vec::new();
    for line in joined.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let is_macos_audio_index =
            cfg!(target_os = "macos") && trimmed.contains("AVFoundation indev") && trimmed.contains("] [");
        if trimmed.contains("AVFoundation audio devices")
            || trimmed.contains("AVFoundation input device")
            || trimmed.contains("DirectShow audio devices")
            || trimmed.contains("Alternative name")
            || is_macos_audio_index
            || (cfg!(target_os = "windows") && trimmed.contains("]  \""))
        {
            hints.push(trimmed.to_string());
        }
    }

    if hints.is_empty() {
        hints.push("No parsed devices found. Run `ffmpeg` device list manually for this platform.".to_string());
    }

    if cfg!(target_os = "macos") && supports_native_system_audio_capture() {
        hints.insert(
            0,
            "Native system source available: select \"System Audio (macOS Native)\" for ScreenCaptureKit-based capture."
                .to_string(),
        );
    }

    Ok(hints)
}

#[tauri::command]
fn recording_meter(session_id: String, state: State<'_, AppState>) -> Result<RecordingMeter, String> {
    let (output_path, telemetry) = {
        let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "Recording session not found".to_string())?;
        (session.output_path.clone(), Arc::clone(&session.telemetry))
    };

    let file_bytes = fs::metadata(&output_path).map(|meta| meta.len()).unwrap_or(0);
    let mut state = telemetry.lock().map_err(|e| e.to_string())?;
    if file_bytes > state.bytes_written {
        state.bytes_written = file_bytes;
    }

    Ok(RecordingMeter {
        bytes_written: state.bytes_written,
        level: state.level,
    })
}

#[tauri::command]
fn bootstrap_state(state: State<'_, AppState>) -> Result<BootstrapState, String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;

    let mut folders_stmt = conn
        .prepare("SELECT id, parent_id, name, created_at, updated_at, deleted_at FROM folders ORDER BY created_at ASC")
        .map_err(|e| format!("Failed to prepare folders query: {e}"))?;

    let folders_iter = folders_stmt
        .query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                name: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                deleted_at: row.get(5)?,
            })
        })
        .map_err(|e| format!("Failed to read folders: {e}"))?;

    let mut folders = Vec::new();
    for item in folders_iter {
        folders.push(item.map_err(|e| format!("Failed to parse folder row: {e}"))?);
    }

    let mut entries_stmt = conn
        .prepare(
            "SELECT id, folder_id, title, status, duration_sec, recording_path, created_at, updated_at, deleted_at
             FROM entries
             ORDER BY created_at DESC",
        )
        .map_err(|e| format!("Failed to prepare entries query: {e}"))?;

    let entries_iter = entries_stmt
        .query_map([], |row| {
            Ok(Entry {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                duration_sec: row.get(4)?,
                recording_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
            })
        })
        .map_err(|e| format!("Failed to read entries: {e}"))?;

    let mut entries = Vec::new();
    for item in entries_iter {
        entries.push(item.map_err(|e| format!("Failed to parse entry row: {e}"))?);
    }

    let mut prompts_stmt = conn
        .prepare("SELECT role, prompt_text, updated_at FROM prompt_templates ORDER BY role ASC")
        .map_err(|e| format!("Failed to prepare prompts query: {e}"))?;
    let prompts_iter = prompts_stmt
        .query_map([], |row| {
            Ok(PromptTemplate {
                role: row.get(0)?,
                prompt_text: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })
        .map_err(|e| format!("Failed to read prompts: {e}"))?;

    let mut prompts = Vec::new();
    for item in prompts_iter {
        prompts.push(item.map_err(|e| format!("Failed to parse prompt row: {e}"))?);
    }

    Ok(BootstrapState {
        folders,
        entries,
        prompt_templates: prompts,
        model_name: model_name(&conn)?,
    })
}

#[tauri::command]
fn get_entry_bundle(entry_id: String, state: State<'_, AppState>) -> Result<EntryBundle, String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_entry_exists(&conn, &entry_id)?;

    let mut transcript_stmt = conn
        .prepare(
            "SELECT id, entry_id, version, text, language, is_manual_edit, created_at
             FROM transcript_revisions
             WHERE entry_id = ?1
             ORDER BY version DESC",
        )
        .map_err(|e| format!("Failed to prepare transcript bundle query: {e}"))?;

    let transcript_iter = transcript_stmt
        .query_map(params![entry_id], |row| {
            Ok(TranscriptRevision {
                id: row.get(0)?,
                entry_id: row.get(1)?,
                version: row.get(2)?,
                text: row.get(3)?,
                language: row.get(4)?,
                is_manual_edit: row.get::<_, i64>(5)? == 1,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| format!("Failed to query transcript bundle: {e}"))?;

    let mut transcript_revisions = Vec::new();
    for item in transcript_iter {
        transcript_revisions.push(item.map_err(|e| format!("Failed to parse transcript row: {e}"))?);
    }

    let mut artifact_stmt = conn
        .prepare(
            "SELECT id, entry_id, artifact_type, version, text, source_transcript_version, is_stale, is_manual_edit, created_at
             FROM artifact_revisions
             WHERE entry_id = ?1
             ORDER BY artifact_type ASC, version DESC",
        )
        .map_err(|e| format!("Failed to prepare artifact bundle query: {e}"))?;

    let artifact_iter = artifact_stmt
        .query_map(params![entry_id], |row| {
            Ok(ArtifactRevision {
                id: row.get(0)?,
                entry_id: row.get(1)?,
                artifact_type: row.get(2)?,
                version: row.get(3)?,
                text: row.get(4)?,
                source_transcript_version: row.get(5)?,
                is_stale: row.get::<_, i64>(6)? == 1,
                is_manual_edit: row.get::<_, i64>(7)? == 1,
                created_at: row.get(8)?,
            })
        })
        .map_err(|e| format!("Failed to query artifact bundle: {e}"))?;

    let mut artifact_revisions = Vec::new();
    for item in artifact_iter {
        artifact_revisions.push(item.map_err(|e| format!("Failed to parse artifact row: {e}"))?);
    }

    Ok(EntryBundle {
        transcript_revisions,
        artifact_revisions,
    })
}

#[tauri::command]
fn create_folder(name: String, parent_id: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;

    if let Some(parent) = &parent_id {
        ensure_folder_exists(&conn, parent)?;
    }

    let now = now_ts();
    conn.execute(
        "INSERT INTO folders(id, parent_id, name, created_at, updated_at, deleted_at) VALUES(?1, ?2, ?3, ?4, ?4, NULL)",
        params![Uuid::new_v4().to_string(), parent_id, name.trim(), now],
    )
    .map_err(|e| format!("Failed to create folder: {e}"))?;

    Ok(())
}

#[tauri::command]
fn rename_folder(folder_id: String, name: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_folder_exists(&conn, &folder_id)?;

    conn.execute(
        "UPDATE folders SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name.trim(), now_ts(), folder_id],
    )
    .map_err(|e| format!("Failed to rename folder: {e}"))?;

    Ok(())
}

#[tauri::command]
fn create_entry(folder_id: String, title: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_folder_exists(&conn, &folder_id)?;

    let id = Uuid::new_v4().to_string();
    let now = now_ts();

    conn.execute(
        "INSERT INTO entries(id, folder_id, title, status, duration_sec, recording_path, created_at, updated_at, deleted_at)
         VALUES(?1, ?2, ?3, 'new', 0, NULL, ?4, ?4, NULL)",
        params![id, folder_id, title.trim(), now],
    )
    .map_err(|e| format!("Failed to create entry: {e}"))?;

    let base_data_dir = data_dir(&state)?;
    ensure_entry_dirs(&base_data_dir, &id)?;

    Ok(())
}

#[tauri::command]
fn rename_entry(entry_id: String, title: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_entry_exists(&conn, &entry_id)?;

    conn.execute(
        "UPDATE entries SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title.trim(), now_ts(), entry_id],
    )
    .map_err(|e| format!("Failed to rename entry: {e}"))?;

    Ok(())
}

#[tauri::command]
fn move_to_trash(entity_type: String, id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    let now = now_ts();

    match entity_type.as_str() {
        "entry" => {
            conn.execute(
                "UPDATE entries SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| format!("Failed to move entry to trash: {e}"))?;
        }
        "folder" => {
            let folder_ids = descendant_folder_ids(&conn, &id)?;
            for folder_id in &folder_ids {
                conn.execute(
                    "UPDATE folders SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
                    params![now, folder_id],
                )
                .map_err(|e| format!("Failed to trash folder: {e}"))?;
                conn.execute(
                    "UPDATE entries SET deleted_at = ?1, updated_at = ?1 WHERE folder_id = ?2",
                    params![now, folder_id],
                )
                .map_err(|e| format!("Failed to trash entries under folder: {e}"))?;
            }
        }
        _ => return Err("Unknown entity type".to_string()),
    }

    Ok(())
}

#[tauri::command]
fn restore_from_trash(entity_type: String, id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    let now = now_ts();

    match entity_type.as_str() {
        "entry" => {
            conn.execute(
                "UPDATE entries SET deleted_at = NULL, updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|e| format!("Failed to restore entry: {e}"))?;
        }
        "folder" => {
            let folder_ids = descendant_folder_ids(&conn, &id)?;
            for folder_id in &folder_ids {
                conn.execute(
                    "UPDATE folders SET deleted_at = NULL, updated_at = ?1 WHERE id = ?2",
                    params![now, folder_id],
                )
                .map_err(|e| format!("Failed to restore folder: {e}"))?;
                conn.execute(
                    "UPDATE entries SET deleted_at = NULL, updated_at = ?1 WHERE folder_id = ?2",
                    params![now, folder_id],
                )
                .map_err(|e| format!("Failed to restore folder entries: {e}"))?;
            }
        }
        _ => return Err("Unknown entity type".to_string()),
    }

    Ok(())
}

#[tauri::command]
fn purge_entity(entity_type: String, id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    let base_data_dir = data_dir(&state)?;

    match entity_type.as_str() {
        "entry" => {
            conn.execute("DELETE FROM transcript_revisions WHERE entry_id = ?1", params![id])
                .map_err(|e| format!("Failed to purge transcript revisions: {e}"))?;
            conn.execute("DELETE FROM artifact_revisions WHERE entry_id = ?1", params![id])
                .map_err(|e| format!("Failed to purge artifact revisions: {e}"))?;
            conn.execute("DELETE FROM entries WHERE id = ?1", params![id])
                .map_err(|e| format!("Failed to purge entry: {e}"))?;

            let path = entry_dir(&base_data_dir, &id);
            if path.exists() {
                let _ = fs::remove_dir_all(path);
            }
        }
        "folder" => {
            let folder_ids = descendant_folder_ids(&conn, &id)?;
            let entry_ids = entry_ids_for_folder_ids(&conn, &folder_ids)?;

            for entry_id in &entry_ids {
                conn.execute("DELETE FROM transcript_revisions WHERE entry_id = ?1", params![entry_id])
                    .map_err(|e| format!("Failed to purge transcript revisions: {e}"))?;
                conn.execute("DELETE FROM artifact_revisions WHERE entry_id = ?1", params![entry_id])
                    .map_err(|e| format!("Failed to purge artifact revisions: {e}"))?;
                conn.execute("DELETE FROM entries WHERE id = ?1", params![entry_id])
                    .map_err(|e| format!("Failed to purge entry row: {e}"))?;

                let path = entry_dir(&base_data_dir, entry_id);
                if path.exists() {
                    let _ = fs::remove_dir_all(path);
                }
            }

            for folder_id in folder_ids {
                conn.execute("DELETE FROM folders WHERE id = ?1", params![folder_id])
                    .map_err(|e| format!("Failed to purge folder row: {e}"))?;
            }
        }
        _ => return Err("Unknown entity type".to_string()),
    }

    Ok(())
}

#[tauri::command]
fn start_recording(entry_id: String, sources: Vec<RecordingSource>, state: State<'_, AppState>) -> Result<String, String> {
    if sources.is_empty() {
        return Err("At least one audio source is required".to_string());
    }

    let has_native_system_source = sources
        .iter()
        .any(|source| source.format.eq_ignore_ascii_case("screencapturekit"));
    if has_native_system_source && !cfg!(target_os = "macos") {
        return Err("Native system-audio source is currently available only on macOS".to_string());
    }
    if has_native_system_source && !supports_native_system_audio_capture() {
        return Err(
            "Native system-audio capture requires macOS 13 or newer. Use microphone/loopback sources on this version."
                .to_string(),
        );
    }
    if has_native_system_source && sources.len() > 1 {
        return Err(
            "System Audio (macOS Native) currently records as a dedicated source. Remove other sources and retry."
                .to_string(),
        );
    }

    if !has_native_system_source && !find_executable("ffmpeg") {
        return Err("ffmpeg not found in PATH. Install ffmpeg to enable recording.".to_string());
    }

    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_entry_exists(&conn, &entry_id)?;

    let base_data_dir = data_dir(&state)?;
    let entry_directory = ensure_entry_dirs(&base_data_dir, &entry_id)?;
    let existing_path: Option<PathBuf> = conn
        .query_row(
            "SELECT recording_path FROM entries WHERE id = ?1",
            params![entry_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(|e| format!("Failed to read existing recording path: {e}"))?
        .and_then(|path| {
            let parsed = PathBuf::from(path);
            if parsed.exists() {
                Some(parsed)
            } else {
                None
            }
        });

    let output_path = if existing_path.is_some() {
        entry_directory
            .join("audio")
            .join(format!("segment-{}.wav", unix_now()))
    } else {
        entry_directory.join("audio").join("original.wav")
    };

    let mut child = if has_native_system_source {
        #[cfg(target_os = "macos")]
        {
            let helper_binary = ensure_sck_recorder_binary(&base_data_dir)?;
            let mut command = Command::new(helper_binary);
            command.arg("--output");
            command.arg(output_path.to_string_lossy().to_string());
            command.stdin(Stdio::piped());
            command.stdout(Stdio::null());
            command.stderr(Stdio::piped());
            command
                .spawn()
                .map_err(|e| format!("Failed to start ScreenCaptureKit recorder: {e}"))?
        }
        #[cfg(not(target_os = "macos"))]
        {
            unreachable!("Native system source is only available on macOS");
        }
    } else {
        let mut command = Command::new("ffmpeg");
        command.arg("-y");
        command.arg("-nostats");
        command.arg("-progress");
        command.arg("pipe:2");

        for source in &sources {
            command.arg("-f");
            command.arg(&source.format);
            command.arg("-i");
            command.arg(&source.input);
        }

        let filter_graph = if sources.len() > 1 {
            let mut input_refs = String::new();
            for index in 0..sources.len() {
                input_refs.push_str(&format!("[{index}:a]"));
            }
            format!(
                "{input_refs}amix=inputs={}:duration=longest:dropout_transition=2[mix];\
[mix]astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level[mout]",
                sources.len()
            )
        } else {
            "[0:a]astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level[mout]"
                .to_string()
        };
        command.arg("-filter_complex");
        command.arg(filter_graph);
        command.arg("-map");
        command.arg("[mout]");

        command.arg("-ac");
        command.arg("1");
        command.arg("-ar");
        command.arg("16000");
        command.arg(output_path.to_string_lossy().to_string());
        command.stdin(Stdio::piped());
        command.stdout(Stdio::null());
        command.stderr(Stdio::piped());

        command
            .spawn()
            .map_err(|e| format!("Failed to start ffmpeg recording: {e}"))?
    };

    let telemetry = Arc::new(Mutex::new(RecordingTelemetry::default()));
    if let Some(stderr) = child.stderr.take() {
        spawn_recording_telemetry(stderr, Arc::clone(&telemetry));
    }

    // If the recorder exits immediately, surface a clear error instead of creating a dead session.
    thread::sleep(Duration::from_millis(350));
    if let Some(status) = child
        .try_wait()
        .map_err(|e| format!("Failed to inspect recorder process status: {e}"))?
    {
        if has_native_system_source {
            let details = telemetry
                .lock()
                .ok()
                .and_then(|state| state.last_error.clone())
                .unwrap_or_else(|| "no additional details".to_string());
            return Err(format!(
                "Native system recording failed to start (status {status}). \
Grant \"Screen & System Audio Recording\" permission to this app/terminal in macOS Privacy settings and retry. Details: {details}"
            ));
        }
        return Err(format!(
            "Recording failed to start (ffmpeg exited with status {status}). \
Check recording source format/input values and macOS microphone permissions."
        ));
    }

    conn.execute(
        "UPDATE entries SET status = 'recording', updated_at = ?1 WHERE id = ?2",
        params![now_ts(), entry_id],
    )
    .map_err(|e| format!("Failed to mark entry as recording: {e}"))?;

    let session_id = Uuid::new_v4().to_string();
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    sessions.insert(
        session_id.clone(),
        RecordingSession {
            entry_id,
            output_path,
            existing_path,
            child,
            telemetry,
            paused: false,
        },
    );

    Ok(session_id)
}

#[tauri::command]
fn stop_recording(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let mut session = sessions
        .remove(&session_id)
        .ok_or_else(|| "Recording session not found".to_string())?;
    let recorder_error = session
        .telemetry
        .lock()
        .ok()
        .and_then(|state| state.last_error.clone());

    if session.paused {
        let pid = session.child.id();
        set_process_paused(pid, false)?;
        session.paused = false;
    }

    if let Some(mut stdin) = session.child.stdin.take() {
        let _ = stdin.write_all(b"q\n");
    }

    wait_for_recorder_shutdown(&mut session.child);

    let db = db_path(&state)?;
    let conn = connection(&db)?;

    let final_path = if let Some(existing) = &session.existing_path {
        if session.output_path.exists() {
            if existing.exists() {
                let merged = existing
                    .parent()
                    .unwrap_or(existing.as_path())
                    .join(format!("merged-{}.wav", unix_now()));
                concat_recordings(existing, &session.output_path, &merged)?;
                let _ = fs::remove_file(existing);
                fs::rename(&merged, existing)
                    .map_err(|e| format!("Failed to finalize merged recording: {e}"))?;
                let _ = fs::remove_file(&session.output_path);
                existing.clone()
            } else {
                session.output_path.clone()
            }
        } else if existing.exists() {
            // No new segment was produced; preserve previously recorded audio.
            existing.clone()
        } else {
            if let Some(details) = recorder_error {
                return Err(format!("Recording file was not created. Native recorder error: {details}"));
            }
            return Err("Recording file was not created. Ensure system/audio permissions are granted and that audio is actively playing during capture.".to_string());
        }
    } else {
        if session.output_path.exists() {
            session.output_path.clone()
        } else {
            if let Some(details) = recorder_error {
                return Err(format!("Recording file was not created. Native recorder error: {details}"));
            }
            return Err("Recording file was not created. Ensure system/audio permissions are granted and that audio is actively playing during capture.".to_string());
        }
    };

    let file_size = fs::metadata(&final_path).map(|meta| meta.len()).unwrap_or(0);
    if file_size <= 64 {
        return Err(
            "Recording captured no audible data. Check source routing/permissions and try again while audio is playing."
                .to_string(),
        );
    }

    let recording_path = final_path.to_string_lossy().to_string();
    let duration_sec = probe_duration_seconds(&recording_path);

    conn.execute(
        "UPDATE entries
         SET status = 'recorded', recording_path = ?1, duration_sec = ?2, updated_at = ?3
         WHERE id = ?4",
        params![recording_path, duration_sec, now_ts(), session.entry_id],
    )
    .map_err(|e| format!("Failed to finalize recording entry state: {e}"))?;

    Ok(())
}

#[tauri::command]
fn set_recording_paused(session_id: String, paused: bool, state: State<'_, AppState>) -> Result<(), String> {
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Recording session not found".to_string())?;
    if session.paused == paused {
        return Ok(());
    }

    let pid = session.child.id();
    set_process_paused(pid, paused)?;
    session.paused = paused;
    Ok(())
}

#[tauri::command]
fn transcribe_entry(entry_id: String, language: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_entry_exists(&conn, &entry_id)?;

    let mut stmt = conn
        .prepare("SELECT recording_path FROM entries WHERE id = ?1")
        .map_err(|e| format!("Failed to prepare recording path query: {e}"))?;

    let recording_path: Option<String> = stmt
        .query_row(params![entry_id], |row| row.get(0))
        .map_err(|e| format!("Failed to read recording path: {e}"))?;

    let recording_path = recording_path.ok_or_else(|| "No recording found for this entry".to_string())?;

    if !Path::new(&recording_path).exists() {
        return Err("Recording path does not exist on disk".to_string());
    }

    let base_data_dir = data_dir(&state)?;
    let entry_directory = ensure_entry_dirs(&base_data_dir, &entry_id)?;
    let transcript_dir = entry_directory.join("transcript");
    let output_base = transcript_dir.join(format!("tmp_{}", unix_now()));

    let whisper_bin = if find_executable("whisper-cli") {
        "whisper-cli"
    } else if find_executable("whisper") {
        "whisper"
    } else {
        return Err("No Whisper executable found (`whisper-cli` or `whisper`) in PATH".to_string());
    };

    let mut command = Command::new(whisper_bin);
    if whisper_bin == "whisper-cli" {
        let model_path = resolve_whisper_model_path(&base_data_dir)?;
        let language_requested = language
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "auto".to_string());
        let english_only_model = model_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.ends_with(".en.bin"))
            .unwrap_or(false);
        if language_requested == "auto" && english_only_model {
            return Err(
                "Current Whisper model is English-only and cannot auto-detect/transcribe other languages. Install a multilingual model (ggml-tiny.bin or ggml-base.bin)."
                    .to_string(),
            );
        }
        // Use CPU mode for stability on some macOS setups where GPU backend crashes.
        command.arg("-ng");
        command.arg("-m").arg(model_path.to_string_lossy().to_string());
        command.arg("-f").arg(&recording_path);
        command.arg("-otxt");
        command.arg("-of").arg(output_base.to_string_lossy().to_string());
        command.arg("--language").arg(language_requested);
    } else {
        command.arg(&recording_path);
        command.arg("--output_format").arg("txt");
        command.arg("--output_dir").arg(transcript_dir.to_string_lossy().to_string());
        let lang_value = language
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "auto".to_string());
        command.arg("--language").arg(lang_value);
    }

    let output = command
        .output()
        .map_err(|e| format!("Failed to run Whisper command: {e}"))?;
    let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("Whisper transcription failed: {stderr_text}"));
    }

    let transcript_path = if whisper_bin == "whisper-cli" {
        output_base.with_extension("txt")
    } else {
        let mut candidate = None;
        if let Ok(read_dir) = fs::read_dir(&transcript_dir) {
            for item in read_dir.flatten() {
                let path = item.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("txt") {
                    candidate = Some(path);
                }
            }
        }
        candidate.ok_or_else(|| "Whisper did not produce a transcript file".to_string())?
    };

    let transcript_text = fs::read_to_string(&transcript_path)
        .map_err(|e| format!("Failed to read transcript output: {e}"))?;
    if transcript_text.trim().is_empty() {
        return Err(
            "Transcription returned empty text. Check that speech was audible in the recording and that the selected input devices are correct."
                .to_string(),
        );
    }

    let version = get_next_transcript_version(&conn, &entry_id)?;
    let mut language_value = language.unwrap_or_else(|| "auto".to_string());
    if language_value.eq_ignore_ascii_case("auto") {
        if let Some(detected) = parse_whisper_detected_language(&stderr_text) {
            language_value = detected;
        }
    }

    conn.execute(
        "INSERT INTO transcript_revisions(id, entry_id, version, text, language, is_manual_edit, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, 0, ?6)",
        params![Uuid::new_v4().to_string(), entry_id, version, transcript_text, language_value, now_ts()],
    )
    .map_err(|e| format!("Failed to save transcript revision: {e}"))?;

    conn.execute(
        "UPDATE artifact_revisions SET is_stale = 1 WHERE entry_id = ?1",
        params![entry_id],
    )
    .map_err(|e| format!("Failed to mark artifacts stale: {e}"))?;

    conn.execute(
        "UPDATE entries SET status = 'transcribed', updated_at = ?1 WHERE id = ?2",
        params![now_ts(), entry_id],
    )
    .map_err(|e| format!("Failed to update entry status after transcription: {e}"))?;

    Ok(())
}

#[tauri::command]
fn generate_artifact(entry_id: String, artifact_type: String, state: State<'_, AppState>) -> Result<(), String> {
    validate_artifact_type(&artifact_type)?;

    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_entry_exists(&conn, &entry_id)?;

    let transcript = latest_transcript(&conn, &entry_id)?
        .ok_or_else(|| "No transcript found. Run transcription first.".to_string())?;

    let prompt_template = prompt_for_role(&conn, &artifact_type)?;
    let model = model_name(&conn)?;

    let full_prompt = format!(
        "{}\n\nTranscript (language={}):\n{}\n\nReturn markdown only.",
        prompt_template, transcript.language, transcript.text
    );

    let response_text = call_ollama(&model, &full_prompt)?;
    let version = get_next_artifact_version(&conn, &entry_id, &artifact_type)?;

    conn.execute(
        "INSERT INTO artifact_revisions(id, entry_id, artifact_type, version, text, source_transcript_version, is_stale, is_manual_edit, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7)",
        params![
            Uuid::new_v4().to_string(),
            entry_id,
            artifact_type,
            version,
            response_text,
            transcript.version,
            now_ts()
        ],
    )
    .map_err(|e| format!("Failed to save artifact revision: {e}"))?;

    conn.execute(
        "UPDATE entries SET status = 'processed', updated_at = ?1 WHERE id = ?2",
        params![now_ts(), entry_id],
    )
    .map_err(|e| format!("Failed to update entry status after artifact generation: {e}"))?;

    Ok(())
}

#[tauri::command]
fn update_transcript(entry_id: String, text: String, language: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_entry_exists(&conn, &entry_id)?;

    let version = get_next_transcript_version(&conn, &entry_id)?;

    conn.execute(
        "INSERT INTO transcript_revisions(id, entry_id, version, text, language, is_manual_edit, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, 1, ?6)",
        params![Uuid::new_v4().to_string(), entry_id, version, text, language, now_ts()],
    )
    .map_err(|e| format!("Failed to save manual transcript revision: {e}"))?;

    conn.execute(
        "UPDATE artifact_revisions SET is_stale = 1 WHERE entry_id = ?1",
        params![entry_id],
    )
    .map_err(|e| format!("Failed to mark artifacts stale after transcript edit: {e}"))?;

    conn.execute(
        "UPDATE entries SET status = 'edited', updated_at = ?1 WHERE id = ?2",
        params![now_ts(), entry_id],
    )
    .map_err(|e| format!("Failed to update entry status after transcript edit: {e}"))?;

    Ok(())
}

#[tauri::command]
fn update_artifact(entry_id: String, artifact_type: String, text: String, state: State<'_, AppState>) -> Result<(), String> {
    validate_artifact_type(&artifact_type)?;

    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_entry_exists(&conn, &entry_id)?;

    let transcript = latest_transcript(&conn, &entry_id)?
        .ok_or_else(|| "No transcript exists for this entry yet".to_string())?;

    let version = get_next_artifact_version(&conn, &entry_id, &artifact_type)?;

    conn.execute(
        "INSERT INTO artifact_revisions(id, entry_id, artifact_type, version, text, source_transcript_version, is_stale, is_manual_edit, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, 0, 1, ?7)",
        params![
            Uuid::new_v4().to_string(),
            entry_id,
            artifact_type,
            version,
            text,
            transcript.version,
            now_ts()
        ],
    )
    .map_err(|e| format!("Failed to save manual artifact revision: {e}"))?;

    conn.execute(
        "UPDATE entries SET status = 'edited', updated_at = ?1 WHERE id = ?2",
        params![now_ts(), entry_id],
    )
    .map_err(|e| format!("Failed to update entry status after artifact edit: {e}"))?;

    Ok(())
}

#[tauri::command]
fn update_prompt_template(role: String, prompt_text: String, state: State<'_, AppState>) -> Result<(), String> {
    validate_prompt_role(&role)?;

    let db = db_path(&state)?;
    let conn = connection(&db)?;

    conn.execute(
        "INSERT INTO prompt_templates(role, prompt_text, updated_at) VALUES(?1, ?2, ?3)
         ON CONFLICT(role) DO UPDATE SET prompt_text = excluded.prompt_text, updated_at = excluded.updated_at",
        params![role, prompt_text, now_ts()],
    )
    .map_err(|e| format!("Failed to update prompt template: {e}"))?;

    Ok(())
}

#[tauri::command]
fn update_model_name(model_name: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;

    conn.execute(
        "INSERT INTO settings(key, value, updated_at) VALUES(?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![MODEL_NAME_KEY, model_name.trim(), now_ts()],
    )
    .map_err(|e| format!("Failed to update model name: {e}"))?;

    Ok(())
}

#[tauri::command]
fn export_entry_markdown(entry_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    ensure_entry_exists(&conn, &entry_id)?;

    let mut entry_stmt = conn
        .prepare("SELECT title, recording_path, created_at, updated_at FROM entries WHERE id = ?1")
        .map_err(|e| format!("Failed to prepare entry export query: {e}"))?;

    let (title, recording_path, created_at, updated_at): (String, Option<String>, String, String) = entry_stmt
        .query_row(params![entry_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| format!("Failed to load entry for export: {e}"))?;

    let transcript = latest_transcript(&conn, &entry_id)?;
    let summary = latest_artifact_by_type(&conn, &entry_id, "summary")?;
    let analysis = latest_artifact_by_type(&conn, &entry_id, "analysis")?;
    let critique_recruitment = latest_artifact_by_type(&conn, &entry_id, "critique_recruitment")?;
    let critique_sales = latest_artifact_by_type(&conn, &entry_id, "critique_sales")?;
    let critique_cs = latest_artifact_by_type(&conn, &entry_id, "critique_cs")?;

    let mut markdown = String::new();
    markdown.push_str(&format!("# {}\n\n", title));
    markdown.push_str(&format!("- Entry ID: `{}`\n", entry_id));
    markdown.push_str(&format!("- Created: {}\n", created_at));
    markdown.push_str(&format!("- Updated: {}\n", updated_at));
    if let Some(ref t) = transcript {
        markdown.push_str(&format!("- Transcript Version: {}\n", t.version));
    }
    markdown.push('\n');

    markdown.push_str("## Transcript\n\n");
    markdown.push_str(transcript.as_ref().map(|item| item.text.as_str()).unwrap_or("(none)"));
    markdown.push_str("\n\n");

    markdown.push_str("## Summary\n\n");
    markdown.push_str(summary.as_ref().map(|item| item.text.as_str()).unwrap_or("(none)"));
    markdown.push_str("\n\n");

    markdown.push_str("## Analysis\n\n");
    markdown.push_str(analysis.as_ref().map(|item| item.text.as_str()).unwrap_or("(none)"));
    markdown.push_str("\n\n");

    markdown.push_str("## Critique (Recruitment Head)\n\n");
    markdown.push_str(
        critique_recruitment
            .as_ref()
            .map(|item| item.text.as_str())
            .unwrap_or("(none)"),
    );
    markdown.push_str("\n\n");

    markdown.push_str("## Critique (Sales Head)\n\n");
    markdown.push_str(
        critique_sales
            .as_ref()
            .map(|item| item.text.as_str())
            .unwrap_or("(none)"),
    );
    markdown.push_str("\n\n");

    markdown.push_str("## Critique (Customer Success Lead)\n\n");
    markdown.push_str(critique_cs.as_ref().map(|item| item.text.as_str()).unwrap_or("(none)"));
    markdown.push_str("\n");

    let base_data_dir = data_dir(&state)?;
    let entry_directory = ensure_entry_dirs(&base_data_dir, &entry_id)?;
    let exports_dir = entry_directory.join("exports");
    fs::create_dir_all(&exports_dir).map_err(|e| format!("Failed to create export directory: {e}"))?;

    let zip_path = exports_dir.join(format!("export-{}.zip", unix_now()));
    let zip_file = File::create(&zip_path).map_err(|e| format!("Failed to create export zip file: {e}"))?;
    let mut zip_writer = zip::ZipWriter::new(zip_file);
    let options = FileOptions::default();

    zip_writer
        .start_file("entry.md", options)
        .map_err(|e| format!("Failed to create markdown entry in zip: {e}"))?;
    zip_writer
        .write_all(markdown.as_bytes())
        .map_err(|e| format!("Failed to write markdown in zip: {e}"))?;

    if let Some(path) = recording_path {
        let source_path = PathBuf::from(path);
        if source_path.exists() {
            let extension = source_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("wav");
            let mut audio_data = Vec::new();
            let mut file = File::open(&source_path)
                .map_err(|e| format!("Failed to open source audio for export: {e}"))?;
            file.read_to_end(&mut audio_data)
                .map_err(|e| format!("Failed to read source audio for export: {e}"))?;
            zip_writer
                .start_file(format!("audio/original.{extension}"), options)
                .map_err(|e| format!("Failed to create audio entry in zip: {e}"))?;
            zip_writer
                .write_all(&audio_data)
                .map_err(|e| format!("Failed to write audio entry in zip: {e}"))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("Failed to finalize zip export: {e}"))?;

    Ok(zip_path.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data = app
                .path()
                .app_data_dir()?
                .join("ai-transcribe-local");

            fs::create_dir_all(&app_data)?;
            fs::create_dir_all(app_data.join("entries"))?;

            let db_path = app_data.join("app.db");
            if let Err(err) = init_database(&db_path) {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, err).into());
            }

            app.manage(AppState {
                sessions: Mutex::new(HashMap::new()),
                data_dir: app_data,
                db_path,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_recording_devices,
            list_audio_device_hints,
            recording_meter,
            bootstrap_state,
            get_entry_bundle,
            create_folder,
            rename_folder,
            create_entry,
            rename_entry,
            move_to_trash,
            restore_from_trash,
            purge_entity,
            start_recording,
            set_recording_paused,
            stop_recording,
            transcribe_entry,
            generate_artifact,
            update_transcript,
            update_artifact,
            update_prompt_template,
            update_model_name,
            export_entry_markdown
        ])
        .run(tauri::generate_context!())
        .expect("error while running AI Transcribe Local");
}
