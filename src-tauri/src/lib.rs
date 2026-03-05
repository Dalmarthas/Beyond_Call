use chrono::Utc;
use reqwest::blocking::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{Manager, State};
use uuid::Uuid;
use zip::write::FileOptions;

const MODEL_NAME_KEY: &str = "model_name";
const DEFAULT_MODEL_NAME: &str = "qwen3:8b";
const WHISPER_MODEL_KEY: &str = "whisper_model";
const DEFAULT_WHISPER_MODEL: &str = "turbo";
const OPENAI_WHISPER_MODELS: &[&str] = &[
    "tiny",
    "tiny.en",
    "base",
    "base.en",
    "small",
    "small.en",
    "medium",
    "medium.en",
    "large",
    "large-v2",
    "large-v3",
    "turbo",
];
const WINDOWS_LOOPBACK_FALLBACK_IDLE_POLLS: u32 = 20;
const WINDOWS_LOOPBACK_FALLBACK_SILENT_POLLS: u32 = 75;
#[cfg(target_os = "macos")]
const SCK_RECORDER_SWIFT: &str = include_str!("../macos/screen_capture_audio.swift");

struct AppState {
    sessions: Mutex<HashMap<String, RecordingSession>>,
    data_dir: PathBuf,
    db_path: PathBuf,
}

struct RecordingProcess {
    output_path: PathBuf,
    native_microphone_path: Option<PathBuf>,
    windows_loopback_raw_path: Option<PathBuf>,
    windows_loopback_format: Option<WindowsLoopbackPcmFormat>,
    child: Option<Child>,
    windows_wasapi_capture: Option<WindowsWasapiCapture>,
    telemetry: Arc<Mutex<RecordingTelemetry>>,
}

struct WindowsWasapiCapture {
    stop_tx: mpsc::Sender<()>,
    join_handle: Option<thread::JoinHandle<Result<(), String>>>,
}

#[derive(Clone, Copy)]
enum WindowsLoopbackLevelKind {
    Float32,
}

#[derive(Clone, Copy)]
struct WindowsLoopbackPcmFormat {
    ffmpeg_input_format: &'static str,
    sample_rate: u32,
    channels: u16,
    bytes_per_frame: usize,
    level_kind: WindowsLoopbackLevelKind,
}

struct RecordingSession {
    entry_id: String,
    sources: Vec<RecordingSource>,
    source_analysis: RecordingSourceAnalysis,
    existing_path: Option<PathBuf>,
    process: Option<RecordingProcess>,
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
    whisper_model: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecordingDevicesWithHints {
    devices: Vec<RecordingDevice>,
    hints: Vec<String>,
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

    conn.execute(
        "INSERT OR IGNORE INTO settings(key, value, updated_at) VALUES(?1, ?2, ?3)",
        params![WHISPER_MODEL_KEY, DEFAULT_WHISPER_MODEL, now],
    )
    .map_err(|e| format!("Failed to seed whisper model setting: {e}"))?;

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

fn setting_value(conn: &Connection, key: &str, fallback: &str) -> Result<String, String> {
    let mut stmt = conn
        .prepare("SELECT value FROM settings WHERE key = ?1")
        .map_err(|e| format!("Failed to prepare settings query: {e}"))?;

    let result: Result<String, _> = stmt.query_row(params![key], |row| row.get(0));
    Ok(result.unwrap_or_else(|_| fallback.to_string()))
}

fn model_name(conn: &Connection) -> Result<String, String> {
    setting_value(conn, MODEL_NAME_KEY, DEFAULT_MODEL_NAME)
}

fn whisper_model_name(conn: &Connection) -> Result<String, String> {
    setting_value(conn, WHISPER_MODEL_KEY, DEFAULT_WHISPER_MODEL)
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

#[cfg(target_os = "macos")]
fn supports_native_system_audio_plus_microphone() -> bool {
    macos_version_major().map(|major| major >= 15).unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn supports_native_system_audio_plus_microphone() -> bool {
    false
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
    let source_changed = match fs::read_to_string(&source_path) {
        Ok(existing) => existing != SCK_RECORDER_SWIFT,
        Err(_) => true,
    };
    if source_changed {
        fs::write(&source_path, SCK_RECORDER_SWIFT)
            .map_err(|e| format!("Failed to write ScreenCaptureKit helper source: {e}"))?;
    }

    let binary_path = bin_dir.join("screen_capture_audio");
    let should_compile = source_changed || !binary_path.exists();

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

fn native_system_recording_device() -> Option<RecordingDevice> {
    #[cfg(target_os = "macos")]
    {
        if supports_native_system_audio_capture() {
            return Some(RecordingDevice {
                name: "System Audio (macOS Native)".to_string(),
                format: "screencapturekit".to_string(),
                input: "system".to_string(),
                is_loopback: true,
            });
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct RecordingSourceAnalysis {
    has_native_system_source: bool,
    native_with_microphone: bool,
}

impl RecordingSourceAnalysis {
    fn requires_ffmpeg(self, has_existing_path: bool) -> bool {
        !self.has_native_system_source || has_existing_path || self.native_with_microphone
    }
}

fn is_native_system_source(source: &RecordingSource) -> bool {
    source.format.eq_ignore_ascii_case("screencapturekit")
}

fn is_windows_wasapi_loopback_source(source: &RecordingSource) -> bool {
    source.format.eq_ignore_ascii_case("wasapi_loopback")
}

fn has_windows_wasapi_loopback_source(sources: &[RecordingSource]) -> bool {
    cfg!(target_os = "windows") && sources.iter().any(is_windows_wasapi_loopback_source)
}

fn ffmpeg_required_for_recording_sources(
    sources: &[RecordingSource],
    source_analysis: RecordingSourceAnalysis,
    has_existing_path: bool,
) -> bool {
    if has_windows_wasapi_loopback_source(sources) {
        // Current WASAPI path records float loopback data and finalizes via ffmpeg conversion/mix.
        return true;
    }
    source_analysis.requires_ffmpeg(has_existing_path)
}

fn analyze_recording_sources(
    sources: &[RecordingSource],
    is_macos_target: bool,
    native_system_supported: bool,
    native_plus_microphone_supported: bool,
) -> Result<RecordingSourceAnalysis, String> {
    if sources.is_empty() {
        return Err("At least one audio source is required".to_string());
    }

    let has_native_system_source = sources.iter().any(is_native_system_source);
    let non_native_source_count = sources.iter().filter(|source| !is_native_system_source(source)).count();
    let native_with_microphone = has_native_system_source && non_native_source_count > 0;

    if has_native_system_source && !is_macos_target {
        return Err("Native system-audio source is currently available only on macOS".to_string());
    }
    if has_native_system_source && !native_system_supported {
        return Err(
            "Native system-audio capture requires macOS 13 or newer. Use microphone/loopback sources on this version."
                .to_string(),
        );
    }
    if native_with_microphone && !native_plus_microphone_supported {
        return Err(
            "Native system + microphone capture requires macOS 15 or newer. On older versions, use loopback + microphone sources."
                .to_string(),
        );
    }
    if has_native_system_source && non_native_source_count > 1 {
        return Err(
            "With System Audio (macOS Native), select at most one additional microphone source."
                .to_string(),
        );
    }

    Ok(RecordingSourceAnalysis {
        has_native_system_source,
        native_with_microphone,
    })
}

fn recording_output_paths(
    entry_directory: &Path,
    has_existing_path: bool,
    native_with_microphone: bool,
    segment_stamp: u64,
) -> (PathBuf, Option<PathBuf>) {
    let output_path = if has_existing_path {
        entry_directory
            .join("audio")
            .join(format!("segment-{segment_stamp}.wav"))
    } else {
        entry_directory.join("audio").join("original.wav")
    };

    let native_microphone_path = if native_with_microphone {
        if has_existing_path {
            Some(
                entry_directory
                    .join("audio")
                    .join(format!("segment-{segment_stamp}-microphone.wav")),
            )
        } else {
            Some(entry_directory.join("audio").join("original-microphone.wav"))
        }
    } else {
        None
    };

    (output_path, native_microphone_path)
}

fn ffmpeg_recording_filter_graph(source_count: usize) -> String {
    if source_count > 1 {
        let mut input_refs = String::new();
        for index in 0..source_count {
            input_refs.push_str(&format!("[{index}:a]"));
        }
        format!(
            "{input_refs}amix=inputs={source_count}:duration=longest:dropout_transition=2[mix];\
[mix]astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level[mout]"
        )
    } else {
        "[0:a]astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level[mout]"
            .to_string()
    }
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
                        state.level = state.level.max(level.clamp(0.0, 1.0));
                    }
                }
                continue;
            }

            let lower = line.to_ascii_lowercase();
            if lower.contains("error") || lower.contains("failed") {
                if let Ok(mut state) = telemetry.lock() {
                    state.last_error = Some(line.trim().to_string());
                }
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
                    state.level = state.level.max(mapped.clamp(0.0, 1.0));
                }
            }
        }
    });
}

fn recording_start_failure_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "Check recording source format/input values and macOS microphone/screen-audio permissions."
    } else if cfg!(target_os = "windows") {
        "Check source format/input values and Windows microphone privacy settings plus loopback routing (VB-CABLE/Stereo Mix)."
    } else {
        "Check recording source format/input values and audio permissions for this platform."
    }
}

fn recording_runtime_failure_hint() -> &'static str {
    if cfg!(target_os = "macos") {
        "Ensure microphone/screen-audio permissions are granted and audio is actively playing during capture."
    } else if cfg!(target_os = "windows") {
        "Ensure microphone privacy access is allowed and loopback routing (VB-CABLE/Stereo Mix) is configured with active playback."
    } else {
        "Ensure audio permissions and source routing are configured and audio is actively playing during capture."
    }
}

fn windows_level_from_float32(buffer: &[u8]) -> f32 {
    let mut peak = 0.0_f32;
    for chunk in buffer.chunks_exact(4) {
        let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]).abs();
        if value.is_finite() && value > peak {
            peak = value;
        }
    }
    peak.clamp(0.0, 1.0)
}

fn windows_pcm_peak_level(buffer: &[u8], _level_kind: WindowsLoopbackLevelKind) -> f32 {
    windows_level_from_float32(buffer)
}


fn spawn_ffmpeg_capture_child(sources: &[RecordingSource], output_path: &Path) -> Result<Child, String> {
    let mut command = Command::new("ffmpeg");
    command.arg("-y");
    command.arg("-nostats");
    command.arg("-progress");
    command.arg("pipe:2");

    for source in sources {
        command.arg("-f");
        command.arg(&source.format);
        command.arg("-i");
        command.arg(&source.input);
    }

    let filter_graph = ffmpeg_recording_filter_graph(sources.len());
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
        .map_err(|e| format!("Failed to start ffmpeg recording: {e}"))
}

#[cfg(target_os = "windows")]
fn spawn_windows_wasapi_loopback_capture(
    device_id: &str,
    output_path: &Path,
    telemetry: Arc<Mutex<RecordingTelemetry>>,
) -> Result<(WindowsWasapiCapture, PathBuf, WindowsLoopbackPcmFormat), String> {
    use wasapi::{initialize_mta, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

    let raw_path = output_path.with_extension("loopback.raw");
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (startup_tx, startup_rx) = mpsc::channel::<Result<WindowsLoopbackPcmFormat, String>>();
    let selected_device_id = device_id.to_string();
    let raw_path_for_thread = raw_path.clone();

    let join_handle = thread::spawn(move || -> Result<(), String> {
        let _ = initialize_mta();

        let init_result = (|| -> Result<_, String> {
            let enumerator = DeviceEnumerator::new()
                .map_err(|e| format!("Failed to create WASAPI device enumerator: {e}"))?;
            let default_device_id = enumerator
                .get_default_device(&Direction::Render)
                .ok()
                .and_then(|device| device.get_id().ok());

            let mut candidate_device_ids: Vec<String> = vec![selected_device_id.clone()];
            if let Some(default_id) = default_device_id {
                if !candidate_device_ids
                    .iter()
                    .any(|candidate| candidate == &default_id)
                {
                    candidate_device_ids.push(default_id);
                }
            }

            if let Ok(collection) = enumerator.get_device_collection(&Direction::Render) {
                if let Ok(count) = collection.get_nbr_devices() {
                    for index in 0..count {
                        if let Ok(device) = collection.get_device_at_index(index) {
                            if let Ok(id) = device.get_id() {
                                if !candidate_device_ids
                                    .iter()
                                    .any(|candidate| candidate == &id)
                                {
                                    candidate_device_ids.push(id);
                                }
                            }
                        }
                    }
                }
            }

            let open_stream = |target_device_id: &str| -> Result<
                (
                    wasapi::AudioClient,
                    wasapi::AudioCaptureClient,
                    WindowsLoopbackPcmFormat,
                    Vec<u8>,
                ),
                String,
            > {
                let device = enumerator
                    .get_device(target_device_id)
                    .map_err(|e| format!("Failed to open playback device `{target_device_id}`: {e}"))?;
                let mut audio_client = device
                    .get_iaudioclient()
                    .map_err(|e| format!("Failed to create WASAPI audio client: {e}"))?;

                let desired_format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
                let stream_mode = StreamMode::PollingShared {
                    autoconvert: true,
                    buffer_duration_hns: 0,
                };

                audio_client
                    .initialize_client(&desired_format, &Direction::Capture, &stream_mode)
                    .map_err(|e| {
                        format!(
                            "Failed to initialize WASAPI loopback stream for `{target_device_id}`: {e}"
                        )
                    })?;

                let capture_client = audio_client
                    .get_audiocaptureclient()
                    .map_err(|e| format!("Failed to create WASAPI capture client: {e}"))?;
                audio_client
                    .start_stream()
                    .map_err(|e| format!("Failed to start WASAPI loopback stream: {e}"))?;

                let loopback_format = WindowsLoopbackPcmFormat {
                    ffmpeg_input_format: "f32le",
                    sample_rate: 48_000,
                    channels: 2,
                    bytes_per_frame: desired_format.get_blockalign() as usize,
                    level_kind: WindowsLoopbackLevelKind::Float32,
                };

                Ok((audio_client, capture_client, loopback_format, Vec::<u8>::new()))
            };

            let mut active_device_id = selected_device_id.clone();
            let (audio_client, capture_client, loopback_format, sample_buffer) =
                match open_stream(&selected_device_id) {
                    Ok(stream) => stream,
                    Err(selected_error) => {
                        let mut last_error = selected_error;
                        let mut fallback_stream = None;
                        for candidate_id in candidate_device_ids.iter().skip(1) {
                            match open_stream(candidate_id) {
                                Ok(stream) => {
                                    fallback_stream = Some((candidate_id.clone(), stream));
                                    break;
                                }
                                Err(error) => {
                                    last_error = error;
                                }
                            }
                        }

                        let Some((fallback_id, stream)) = fallback_stream else {
                            return Err(format!(
                                "Failed to initialize WASAPI loopback capture for selected endpoint `{selected_device_id}` and all fallback playback endpoints. Last error: {last_error}"
                            ));
                        };

                        active_device_id = fallback_id;
                        stream
                    }
                };

            let raw_file = File::create(&raw_path_for_thread)
                .map_err(|e| format!("Failed to create loopback temp file: {e}"))?;

            Ok((
                audio_client,
                capture_client,
                raw_file,
                loopback_format,
                sample_buffer,
                active_device_id,
                candidate_device_ids,
                0_usize,
            ))
        })();

        let (
            mut audio_client,
            mut capture_client,
            mut raw_file,
            loopback_format,
            mut sample_buffer,
            mut active_device_id,
            candidate_device_ids,
            mut fallback_index,
        ) = match init_result {
            Ok(value) => value,
            Err(error) => {
                if let Ok(mut state) = telemetry.lock() {
                    state.last_error = Some(error.clone());
                }
                let _ = startup_tx.send(Err(error.clone()));
                return Err(error);
            }
        };

        let _ = startup_tx.send(Ok(loopback_format));

        let mut captured_bytes: u64 = 0;
        let mut idle_polls = 0_u32;
        let mut silent_polls = 0_u32;

        loop {
            match stop_rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => break,
                Err(TryRecvError::Empty) => {}
            }

            thread::sleep(Duration::from_millis(20));
            let mut saw_packet = false;
            let mut saw_non_silent_packet = false;

            loop {
                let Some(frame_count) = capture_client
                    .get_next_packet_size()
                    .map_err(|e| format!("Failed to poll WASAPI packet size: {e}"))?
                else {
                    break;
                };

                if frame_count == 0 {
                    break;
                }

                saw_packet = true;

                let required_bytes = frame_count as usize * loopback_format.bytes_per_frame;
                if sample_buffer.len() < required_bytes {
                    sample_buffer.resize(required_bytes, 0);
                }

                let (captured_frames, info) = capture_client
                    .read_from_device(&mut sample_buffer[..required_bytes])
                    .map_err(|e| format!("Failed to read WASAPI loopback buffer: {e}"))?;
                if captured_frames == 0 {
                    break;
                }

                let captured_chunk_bytes = captured_frames as usize * loopback_format.bytes_per_frame;
                if info.flags.silent {
                    sample_buffer[..captured_chunk_bytes].fill(0);
                }

                raw_file
                    .write_all(&sample_buffer[..captured_chunk_bytes])
                    .map_err(|e| format!("Failed to write WASAPI loopback data: {e}"))?;
                captured_bytes = captured_bytes.saturating_add(captured_chunk_bytes as u64);

                if let Ok(mut state) = telemetry.lock() {
                    state.bytes_written = captured_bytes;
                    let level = if info.flags.silent {
                        0.0
                    } else {
                        windows_pcm_peak_level(
                            &sample_buffer[..captured_chunk_bytes],
                            loopback_format.level_kind,
                        )
                    };
                    if level > 0.0005 {
                        saw_non_silent_packet = true;
                    }
                    state.level = state.level.max(level.clamp(0.0, 1.0));
                }
            }

            if saw_non_silent_packet {
                idle_polls = 0;
                silent_polls = 0;
                continue;
            }

            if saw_packet {
                idle_polls = 0;
                silent_polls = silent_polls.saturating_add(1);
            } else {
                idle_polls = idle_polls.saturating_add(1);
                silent_polls = silent_polls.saturating_add(1);
            }

            let should_try_fallback = if captured_bytes == 0 {
                idle_polls >= WINDOWS_LOOPBACK_FALLBACK_IDLE_POLLS
            } else {
                silent_polls >= WINDOWS_LOOPBACK_FALLBACK_SILENT_POLLS
            };
            if !should_try_fallback {
                continue;
            }

            idle_polls = 0;
            silent_polls = 0;
            let fallback_reason = if saw_packet {
                "only silent packets were captured from the active playback endpoint"
            } else {
                "no packets were captured from the active playback endpoint"
            };

            let mut switched = false;
            let mut last_switch_error: Option<String> = None;

            if candidate_device_ids.len() > 1 {
                for _ in 0..candidate_device_ids.len() {
                    let candidate_id = candidate_device_ids
                        .get(fallback_index % candidate_device_ids.len())
                        .cloned()
                        .unwrap_or_else(|| active_device_id.clone());
                    fallback_index = (fallback_index + 1) % candidate_device_ids.len();

                    if candidate_id == active_device_id {
                        continue;
                    }

                    let enumerator = match DeviceEnumerator::new() {
                        Ok(value) => value,
                        Err(error) => {
                            last_switch_error =
                                Some(format!("Failed to create WASAPI enumerator for fallback: {error}"));
                            break;
                        }
                    };
                    let device = match enumerator.get_device(&candidate_id) {
                        Ok(value) => value,
                        Err(error) => {
                            last_switch_error = Some(format!(
                                "Failed to open fallback playback endpoint `{candidate_id}`: {error}"
                            ));
                            continue;
                        }
                    };
                    let mut candidate_client = match device.get_iaudioclient() {
                        Ok(value) => value,
                        Err(error) => {
                            last_switch_error =
                                Some(format!("Failed to create fallback audio client: {error}"));
                            continue;
                        }
                    };

                    let desired_format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
                    let stream_mode = StreamMode::PollingShared {
                        autoconvert: true,
                        buffer_duration_hns: 0,
                    };
                    if let Err(error) =
                        candidate_client.initialize_client(&desired_format, &Direction::Capture, &stream_mode)
                    {
                        last_switch_error = Some(format!(
                            "Failed to initialize fallback loopback stream for `{candidate_id}`: {error}"
                        ));
                        continue;
                    }

                    let candidate_capture_client = match candidate_client.get_audiocaptureclient() {
                        Ok(value) => value,
                        Err(error) => {
                            last_switch_error =
                                Some(format!("Failed to create fallback capture client: {error}"));
                            continue;
                        }
                    };
                    if let Err(error) = candidate_client.start_stream() {
                        last_switch_error =
                            Some(format!("Failed to start fallback loopback stream: {error}"));
                        continue;
                    }

                    let _ = audio_client.stop_stream();
                    audio_client = candidate_client;
                    capture_client = candidate_capture_client;
                    active_device_id = candidate_id.clone();
                    switched = true;

                    if let Ok(mut state) = telemetry.lock() {
                        state.last_error = Some(format!(
                            "Selected playback endpoint {fallback_reason}. Switched loopback capture to `{active_device_id}`."
                        ));
                    }
                    break;
                }
            }

            if !switched {
                if let Ok(mut state) = telemetry.lock() {
                    state.last_error = Some(match last_switch_error {
                        Some(error) => format!(
                            "WASAPI fallback failed after endpoint rotation: {fallback_reason}. {error}"
                        ),
                        None => format!(
                            "WASAPI fallback failed: {fallback_reason}. Ensure audio is playing on the selected output device, then refresh sources."
                        ),
                    });
                }
            }
        }

        let _ = audio_client.stop_stream();
        raw_file
            .flush()
            .map_err(|e| format!("Failed to flush WASAPI loopback data: {e}"))?;
        Ok(())
    });

    match startup_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(loopback_format)) => Ok((
            WindowsWasapiCapture {
                stop_tx,
                join_handle: Some(join_handle),
            },
            raw_path,
            loopback_format,
        )),
        Ok(Err(error)) => {
            let _ = join_handle.join();
            Err(error)
        }
        Err(_) => {
            let _ = stop_tx.send(());
            let _ = join_handle.join();
            Err("Timed out while starting WASAPI loopback capture.".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn spawn_windows_wasapi_loopback_capture(
    _device_id: &str,
    _output_path: &Path,
    _telemetry: Arc<Mutex<RecordingTelemetry>>,
) -> Result<(WindowsWasapiCapture, PathBuf, WindowsLoopbackPcmFormat), String> {
    Err("WASAPI loopback capture is only available on Windows.".to_string())
}
#[cfg(target_os = "windows")]
fn finalize_windows_loopback_raw_capture(
    raw_path: &Path,
    output_path: &Path,
    loopback_format: WindowsLoopbackPcmFormat,
) -> Result<(), String> {
    let out = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg(loopback_format.ffmpeg_input_format)
        .arg("-ar")
        .arg(loopback_format.sample_rate.to_string())
        .arg("-ac")
        .arg(loopback_format.channels.to_string())
        .arg("-i")
        .arg(raw_path)
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg(output_path)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg WASAPI conversion: {e}"))?;

    if !out.status.success() {
        let stderr_text = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "Failed to convert WASAPI loopback data into wav: {stderr_text}"
        ));
    }

    let _ = fs::remove_file(raw_path);
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn finalize_windows_loopback_raw_capture(
    _raw_path: &Path,
    _output_path: &Path,
    _loopback_format: WindowsLoopbackPcmFormat,
) -> Result<(), String> {
    Ok(())
}

fn spawn_recording_process(
    helper_data_dir: &Path,
    entry_directory: &Path,
    sources: &[RecordingSource],
    source_analysis: RecordingSourceAnalysis,
    has_existing_path: bool,
) -> Result<RecordingProcess, String> {
    #[cfg(not(target_os = "macos"))]
    let _ = helper_data_dir;

    let segment_stamp = unix_now();
    let (output_path, mut native_microphone_path) = recording_output_paths(
        entry_directory,
        has_existing_path,
        source_analysis.native_with_microphone,
        segment_stamp,
    );

    let telemetry = Arc::new(Mutex::new(RecordingTelemetry::default()));
    let mut child: Option<Child> = None;
    let mut windows_wasapi_capture: Option<WindowsWasapiCapture> = None;
    let mut windows_loopback_raw_path: Option<PathBuf> = None;
    let mut windows_loopback_format: Option<WindowsLoopbackPcmFormat> = None;

    if source_analysis.has_native_system_source {
        #[cfg(target_os = "macos")]
        {
            let helper_binary = ensure_sck_recorder_binary(helper_data_dir)?;
            let mut command = Command::new(helper_binary);
            command.arg("--output");
            command.arg(output_path.to_string_lossy().to_string());
            if let Some(path) = &native_microphone_path {
                command.arg("--with-microphone");
                command.arg("--microphone-output");
                command.arg(path.to_string_lossy().to_string());
            }
            command.stdin(Stdio::piped());
            command.stdout(Stdio::null());
            command.stderr(Stdio::piped());

            let mut native_child = command
                .spawn()
                .map_err(|e| format!("Failed to start ScreenCaptureKit recorder: {e}"))?;
            if let Some(stderr) = native_child.stderr.take() {
                spawn_recording_telemetry(stderr, Arc::clone(&telemetry));
            }
            child = Some(native_child);
        }
        #[cfg(not(target_os = "macos"))]
        {
            unreachable!("Native system source is only available on macOS");
        }
    } else if has_windows_wasapi_loopback_source(sources) {
        let mut loopback_sources = sources
            .iter()
            .filter(|source| is_windows_wasapi_loopback_source(source));
        let loopback_source = loopback_sources
            .next()
            .ok_or_else(|| "WASAPI loopback source was selected but no source payload was provided.".to_string())?;
        if loopback_sources.next().is_some() {
            return Err("Select only one playback (WASAPI) source at a time.".to_string());
        }

        let non_loopback_sources: Vec<RecordingSource> = sources
            .iter()
            .filter(|source| !is_windows_wasapi_loopback_source(source))
            .cloned()
            .collect();

        if !non_loopback_sources.is_empty() {
            let microphone_output_path = if has_existing_path {
                entry_directory
                    .join("audio")
                    .join(format!("segment-{segment_stamp}-microphone.wav"))
            } else {
                entry_directory.join("audio").join("original-microphone.wav")
            };

            let mut microphone_child =
                spawn_ffmpeg_capture_child(&non_loopback_sources, &microphone_output_path)?;
            if let Some(stderr) = microphone_child.stderr.take() {
                spawn_recording_telemetry(stderr, Arc::clone(&telemetry));
            }
            child = Some(microphone_child);
            native_microphone_path = Some(microphone_output_path);
        } else {
            native_microphone_path = None;
        }

        let (capture, raw_path, loopback_format) = spawn_windows_wasapi_loopback_capture(
            &loopback_source.input,
            &output_path,
            Arc::clone(&telemetry),
        )?;
        windows_wasapi_capture = Some(capture);
        windows_loopback_raw_path = Some(raw_path);
        windows_loopback_format = Some(loopback_format);
    } else {
        let mut ffmpeg_child = spawn_ffmpeg_capture_child(sources, &output_path)?;
        if let Some(stderr) = ffmpeg_child.stderr.take() {
            spawn_recording_telemetry(stderr, Arc::clone(&telemetry));
        }
        child = Some(ffmpeg_child);
    }

    // If a subprocess exits immediately, surface a clear error instead of creating a dead session.
    let has_windows_loopback_worker = windows_wasapi_capture.is_some();
    let exited_status = if let Some(active_child) = child.as_mut() {
        thread::sleep(Duration::from_millis(350));
        active_child
            .try_wait()
            .map_err(|e| format!("Failed to inspect recorder process status: {e}"))?
    } else {
        None
    };

    if let Some(status) = exited_status {
        if source_analysis.has_native_system_source {
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

        let details = telemetry
            .lock()
            .ok()
            .and_then(|state| state.last_error.clone());

        if cfg!(target_os = "windows")
            && has_windows_loopback_worker
            && has_windows_wasapi_loopback_source(sources)
        {
            let retry_started = if let Some(microphone_output_path) = native_microphone_path.clone() {
                let microphone_sources: Vec<RecordingSource> = sources
                    .iter()
                    .filter(|source| !is_windows_wasapi_loopback_source(source))
                    .cloned()
                    .collect();

                let (resolved_retry_sources, resolved_changed, retry_notes) =
                    resolve_windows_dshow_sources_for_retry(&microphone_sources);

                let retry_sources = if resolved_changed {
                    resolved_retry_sources
                } else if let Some(rewritten) =
                    rewrite_windows_dshow_sources_to_friendly_names(&microphone_sources)
                {
                    rewritten
                } else {
                    microphone_sources.clone()
                };

                let mut retry_child =
                    spawn_ffmpeg_capture_child(&retry_sources, &microphone_output_path)?;
                if let Some(stderr) = retry_child.stderr.take() {
                    spawn_recording_telemetry(stderr, Arc::clone(&telemetry));
                }

                thread::sleep(Duration::from_millis(350));
                if retry_child
                    .try_wait()
                    .map_err(|e| {
                        format!(
                            "Failed to inspect fallback microphone capture process status: {e}"
                        )
                    })?
                    .is_none()
                {
                    child = Some(retry_child);
                    if let Ok(mut state) = telemetry.lock() {
                        if !retry_notes.is_empty() {
                            state.last_error = Some(retry_notes.join(" "));
                        } else if resolved_changed {
                            state.last_error = Some(
                                "Primary DirectShow microphone path failed; switched to validated fallback input."
                                    .to_string(),
                            );
                        }
                    }
                    true
                } else {
                    if let Ok(mut state) = telemetry.lock() {
                        if !retry_notes.is_empty() {
                            state.last_error = Some(retry_notes.join(" "));
                        }
                    }
                    false
                }
            } else {
                false
            };

            if !retry_started {
                let detail_text = telemetry
                    .lock()
                    .ok()
                    .and_then(|state| state.last_error.clone())
                    .or_else(|| details.clone())
                    .unwrap_or_else(|| "no additional details".to_string());
                return Err(format!(
                    "Microphone capture failed to start for the selected source (status {status}) while WASAPI playback capture was active. Pick a different microphone source or verify Windows microphone privacy permissions. Details: {detail_text}"
                ));
            }
        } else if let Some(details) = details {
            return Err(format!(
                "Recording failed to start (ffmpeg exited with status {status}). {} Details: {details}",
                recording_start_failure_hint()
            ));
        } else {
            return Err(format!(
                "Recording failed to start (ffmpeg exited with status {status}). {}",
                recording_start_failure_hint()
            ));
        }
    }

    Ok(RecordingProcess {
        output_path,
        native_microphone_path,
        windows_loopback_raw_path,
        windows_loopback_format,
        child,
        windows_wasapi_capture,
        telemetry,
    })
}

fn finalize_active_recording_process(session: &mut RecordingSession) -> Result<(), String> {
    let Some(mut process) = session.process.take() else {
        return Ok(());
    };

    if let Some(child) = process.child.as_mut() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(b"q\n");
        }
        wait_for_recorder_shutdown(child);
    }

    let mut recorder_error = process
        .telemetry
        .lock()
        .ok()
        .and_then(|state| state.last_error.clone());

    if let Some(mut capture) = process.windows_wasapi_capture.take() {
        let _ = capture.stop_tx.send(());
        if let Some(join_handle) = capture.join_handle.take() {
            match join_handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    recorder_error = Some(error.clone());
                    if let Ok(mut state) = process.telemetry.lock() {
                        state.last_error = Some(error);
                    }
                }
                Err(_) => {
                    let error = "WASAPI loopback worker thread terminated unexpectedly.".to_string();
                    recorder_error = Some(error.clone());
                    if let Ok(mut state) = process.telemetry.lock() {
                        state.last_error = Some(error);
                    }
                }
            }
        }
    }

    let run_output_path = process.output_path.clone();

    if let Some(raw_path) = &process.windows_loopback_raw_path {
        if raw_path.exists() {
            let loopback_format = process
                .windows_loopback_format
                .ok_or_else(|| "Missing WASAPI loopback format metadata for conversion.".to_string())?;
            finalize_windows_loopback_raw_capture(raw_path, &run_output_path, loopback_format)?;
        }
    }

    if let Some(mic_path) = &process.native_microphone_path {
        let loopback_exists = run_output_path.exists();
        let microphone_exists = mic_path.exists();

        if loopback_exists && microphone_exists {
            let loopback_has_signal = recording_contains_audible_signal(&run_output_path);
            let microphone_has_signal = recording_contains_audible_signal(mic_path);

            match (loopback_has_signal, microphone_has_signal) {
                (true, true) => {
                    let mixed_path = run_output_path
                        .parent()
                        .unwrap_or(run_output_path.as_path())
                        .join(format!("mixed-{}.wav", unix_now()));
                    mix_audio_tracks(&run_output_path, mic_path, &mixed_path)?;
                    let _ = fs::remove_file(&run_output_path);
                    fs::rename(&mixed_path, &run_output_path)
                        .map_err(|e| format!("Failed to finalize mixed native recording: {e}"))?;
                    let _ = fs::remove_file(mic_path);
                }
                (true, false) => {
                    let _ = fs::remove_file(mic_path);
                }
                (false, true) => {
                    let _ = fs::remove_file(&run_output_path);
                    fs::rename(mic_path, &run_output_path).map_err(|e| {
                        format!("Failed to promote microphone recording as final output: {e}")
                    })?;
                }
                (false, false) => {
                    let _ = fs::remove_file(mic_path);
                }
            }
        } else if microphone_exists && !loopback_exists {
            fs::rename(mic_path, &run_output_path)
                .map_err(|e| format!("Failed to finalize microphone-only recording: {e}"))?;
        }
    }

    let run_has_audio_payload = run_output_path.exists()
        && recording_contains_audio_payload(&run_output_path)
        && recording_contains_audible_signal(&run_output_path);
    if run_has_audio_payload {
        let existing_valid = session
            .existing_path
            .as_ref()
            .filter(|path| path.exists())
            .cloned();

        if let Some(existing) = existing_valid {
            if existing != run_output_path {
                let merged = existing
                    .parent()
                    .unwrap_or(existing.as_path())
                    .join(format!("merged-{}.wav", unix_now()));
                concat_recordings(&existing, &run_output_path, &merged)?;
                let _ = fs::remove_file(&existing);
                fs::rename(&merged, &existing)
                    .map_err(|e| format!("Failed to finalize merged recording: {e}"))?;
                let _ = fs::remove_file(&run_output_path);
            }
            session.existing_path = Some(existing);
        } else {
            session.existing_path = Some(run_output_path.clone());
        }
        return Ok(());
    }

    if run_output_path.exists() {
        // Empty/near-empty segment: drop it and preserve previous recording when available.
        let _ = fs::remove_file(&run_output_path);
    }

    let has_existing = session
        .existing_path
        .as_ref()
        .map(|path| path.exists())
        .unwrap_or(false);
    if has_existing {
        return Ok(());
    }

    if let Some(details) = recorder_error {
        return Err(format!("Recording file was not created. Native recorder error: {details}"));
    }

    // No completed segment yet (for example, immediate pause). Keep session alive.
    Ok(())
}

fn recording_contains_audio_payload(path: &Path) -> bool {
    let file_size = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    if file_size <= 128 {
        return false;
    }

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return file_size > 1024,
    };

    let mut riff_header = [0_u8; 12];
    if file.read_exact(&mut riff_header).is_err() {
        return file_size > 1024;
    }

    if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
        return file_size > 1024;
    }

    loop {
        let mut chunk_header = [0_u8; 8];
        if file.read_exact(&mut chunk_header).is_err() {
            break;
        }

        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]) as u64;

        if &chunk_header[0..4] == b"data" {
            return chunk_size > 0;
        }

        let aligned_size = chunk_size + (chunk_size % 2);
        if file.seek(SeekFrom::Current(aligned_size as i64)).is_err() {
            break;
        }
    }

    file_size > 1024
}

fn recording_contains_audible_signal(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let mut riff_header = [0_u8; 12];
    if file.read_exact(&mut riff_header).is_err() {
        return false;
    }

    if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
        return false;
    }

    let mut audio_format: Option<u16> = None;
    let mut bits_per_sample: Option<u16> = None;

    loop {
        let mut chunk_header = [0_u8; 8];
        if file.read_exact(&mut chunk_header).is_err() {
            break;
        }

        let chunk_size = u32::from_le_bytes([
            chunk_header[4],
            chunk_header[5],
            chunk_header[6],
            chunk_header[7],
        ]) as u64;

        if &chunk_header[0..4] == b"fmt " {
            if chunk_size < 16 {
                return false;
            }
            let mut fmt_prefix = [0_u8; 16];
            if file.read_exact(&mut fmt_prefix).is_err() {
                return false;
            }
            audio_format = Some(u16::from_le_bytes([fmt_prefix[0], fmt_prefix[1]]));
            bits_per_sample = Some(u16::from_le_bytes([fmt_prefix[14], fmt_prefix[15]]));

            let trailing_fmt_bytes = chunk_size.saturating_sub(16);
            let aligned_trailing = trailing_fmt_bytes + (chunk_size % 2);
            if aligned_trailing > 0
                && file
                    .seek(SeekFrom::Current(aligned_trailing as i64))
                    .is_err()
            {
                return false;
            }
            continue;
        }

        if &chunk_header[0..4] == b"data" {
            let mut remaining = chunk_size;
            let mut buffer = [0_u8; 8192];
            while remaining > 0 {
                let read_len = remaining.min(buffer.len() as u64) as usize;
                if file.read_exact(&mut buffer[..read_len]).is_err() {
                    return false;
                }

                let has_signal = match (audio_format, bits_per_sample) {
                    (Some(1), Some(16)) => buffer[..read_len]
                        .chunks_exact(2)
                        .any(|sample| i16::from_le_bytes([sample[0], sample[1]]) != 0),
                    (Some(3), Some(32)) => buffer[..read_len].chunks_exact(4).any(|sample| {
                        f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]).abs()
                            > 1e-6
                    }),
                    _ => buffer[..read_len].iter().any(|value| *value != 0),
                };

                if has_signal {
                    return true;
                }
                remaining -= read_len as u64;
            }

            return false;
        }

        let aligned_size = chunk_size + (chunk_size % 2);
        if file.seek(SeekFrom::Current(aligned_size as i64)).is_err() {
            break;
        }
    }

    false
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

fn mix_audio_tracks(first: &Path, second: &Path, output: &Path) -> Result<(), String> {
    let out = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(first)
        .arg("-i")
        .arg(second)
        .arg("-filter_complex")
        .arg("[0:a][1:a]amix=inputs=2:duration=longest:dropout_transition=2[a]")
        .arg("-map")
        .arg("[a]")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg(output)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg audio mix: {e}"))?;

    if !out.status.success() {
        let stderr_text = String::from_utf8_lossy(&out.stderr);
        return Err(format!("Failed to mix system + microphone audio: {stderr_text}"));
    }

    Ok(())
}

fn resolve_whisper_model_path(base_data_dir: &Path, preferred_model: Option<&str>) -> Result<PathBuf, String> {
    let min_model_bytes = 10 * 1024 * 1024_u64;
    let cwd = std::env::current_dir().ok();

    let validate_model = |path: &Path| -> Result<bool, String> {
        if !path.exists() {
            return Ok(false);
        }
        let metadata = fs::metadata(path)
            .map_err(|e| format!("Failed to inspect whisper model at {}: {e}", path.display()))?;
        if metadata.len() < min_model_bytes {
            return Err(format!(
                "Whisper model at {} looks invalid ({} bytes). Install a valid ggml model file and retry.",
                path.display(),
                metadata.len()
            ));
        }
        Ok(true)
    };

    let add_named_candidate = |candidates: &mut Vec<PathBuf>, model_name: &str| {
        let trimmed = model_name.trim();
        if trimmed.is_empty() {
            return;
        }
        let direct = PathBuf::from(trimmed);
        if direct.is_absolute() || trimmed.contains('/') || trimmed.contains('\\') {
            candidates.push(direct);
            return;
        }

        candidates.push(base_data_dir.join("models").join(trimmed));
        if let Some(cwd) = &cwd {
            candidates.push(cwd.join("models").join(trimmed));
            candidates.push(cwd.join("..").join("models").join(trimmed));
        }
    };

    if let Ok(explicit) = std::env::var("WHISPER_MODEL_PATH") {
        let candidate = PathBuf::from(explicit);
        if validate_model(&candidate)? {
            return Ok(candidate);
        }
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(model_name) = preferred_model {
        add_named_candidate(&mut candidates, model_name);
    }
    // Prefer multilingual models for language auto-detection.
    add_named_candidate(&mut candidates, "ggml-base.bin");
    add_named_candidate(&mut candidates, "ggml-tiny.bin");
    add_named_candidate(&mut candidates, "ggml-base.en.bin");
    add_named_candidate(&mut candidates, "ggml-tiny.en.bin");

    for candidate in candidates {
        if validate_model(&candidate)? {
            return Ok(candidate);
        }
    }

    Err(
        "No valid whisper model found. Set WHISPER_MODEL_PATH or place ggml-base.bin / ggml-tiny.bin (or *.en variants) in ./models/.".to_string(),
    )
}

fn whisper_model_looks_like_cpp(model_name: &str) -> bool {
    let trimmed = model_name.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.ends_with(".bin")
        || lower.starts_with("ggml-")
        || trimmed.contains('/')
        || trimmed.contains('\\')
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

fn parse_openai_whisper_detected_language(output_text: &str) -> Option<String> {
    let marker = "Detected language:";
    for line in output_text.lines() {
        let Some(pos) = line.find(marker) else {
            continue;
        };
        let suffix = line[(pos + marker.len())..].trim();
        let lang = suffix
            .split(|ch: char| ch == ',' || ch == '(' || ch == '[')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(|ch: char| !ch.is_ascii_alphabetic() && ch != '-')
            .to_ascii_lowercase();
        if (2..=16).contains(&lang.len()) {
            return Some(lang);
        }
    }
    None
}

fn normalize_transcription_language(raw_language: &str) -> String {
    let trimmed = raw_language.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return "auto".to_string();
    }

    let lower = trimmed.to_ascii_lowercase();
    let mapped_code = match lower.as_str() {
        "english" => Some("en"),
        "russian" => Some("ru"),
        "ukrainian" => Some("uk"),
        "spanish" | "castilian" | "valencian" => Some("es"),
        "german" => Some("de"),
        "french" => Some("fr"),
        _ => None,
    };
    if let Some(code) = mapped_code {
        return code.to_string();
    }

    let looks_like_code = lower.len() <= 3 && lower.chars().all(|ch| ch.is_ascii_alphabetic() || ch == '-');
    if looks_like_code {
        return lower;
    }

    // OpenAI Whisper CLI accepts title-cased language names.
    lower
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut normalized = first.to_ascii_uppercase().to_string();
                    normalized.push_str(chars.as_str());
                    normalized
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn ollama_client(timeout_seconds: u64) -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()
        .map_err(|e| format!("Failed to initialize Ollama HTTP client: {e}"))
}

fn ollama_reachable(timeout_seconds: u64) -> bool {
    let Ok(client) = ollama_client(timeout_seconds) else {
        return false;
    };
    let Ok(response) = client.get("http://127.0.0.1:11434/api/tags").send() else {
        return false;
    };
    response.status().is_success()
}

fn start_ollama_server() -> Result<(), String> {
    if !find_executable("ollama") {
        return Err("Ollama executable not found in PATH. Install Ollama first.".to_string());
    }

    Command::new("ollama")
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start Ollama automatically: {e}"))?;

    for _ in 0..24 {
        if ollama_reachable(1) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }

    Err("Ollama did not become ready on http://127.0.0.1:11434.".to_string())
}

fn ollama_tags() -> Result<serde_json::Value, String> {
    let client = ollama_client(8)?;
    let response = client
        .get("http://127.0.0.1:11434/api/tags")
        .send()
        .map_err(|e| format!("Failed to query Ollama models: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Ollama tags request failed with status {}", response.status()));
    }

    response
        .json()
        .map_err(|e| format!("Failed to parse Ollama tags response: {e}"))
}

fn ollama_model_exists(target_model: &str) -> Result<bool, String> {
    let body = ollama_tags()?;
    let normalized_target = target_model.trim();
    if normalized_target.is_empty() {
        return Ok(false);
    }

    let models = body
        .get("models")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    for model in models {
        let Some(name) = model.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        if name == normalized_target {
            return Ok(true);
        }
        if let Some((base, _)) = name.split_once(':') {
            if base == normalized_target {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn warmup_ollama_model(model_name: &str) -> Result<(), String> {
    let client = ollama_client(120)?;
    let response = client
        .post("http://127.0.0.1:11434/api/generate")
        .json(&json!({
            "model": model_name,
            "prompt": "Reply only with OK",
            "stream": false,
            "think": false,
            "options": { "num_predict": 2 }
        }))
        .send()
        .map_err(|e| format!("Failed to warm up Ollama model `{model_name}`: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Warm-up call failed for model `{model_name}` with status {}",
            response.status()
        ));
    }

    Ok(())
}

fn ensure_ollama_ready(model_name: &str, warmup: bool) -> Result<String, String> {
    if !ollama_reachable(2) {
        start_ollama_server()?;
    }

    if !ollama_model_exists(model_name)? {
        Command::new("ollama")
            .arg("pull")
            .arg(model_name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start background model download for `{model_name}`: {e}"))?;
        return Ok(format!(
            "Model `{model_name}` is downloading in background. Summarize/Analyze/Critique will work when download completes."
        ));
    }

    if warmup {
        let model = model_name.to_string();
        thread::spawn(move || {
            let _ = warmup_ollama_model(&model);
        });
    }

    Ok("ready".to_string())
}

fn call_ollama(model_name: &str, prompt: &str) -> Result<String, String> {
    let readiness = ensure_ollama_ready(model_name, false)?;
    if readiness != "ready" {
        return Err(readiness);
    }

    let client = ollama_client(240)?;
    let response = client
        .post("http://127.0.0.1:11434/api/generate")
        .json(&json!({
            "model": model_name,
            "prompt": prompt,
            "stream": false,
            "think": false
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
        "monitor mix",
        "wave out mix",
        "what u hear",
        "wave link stream",
        "wave link system",
        "wave link monitor",
    ];
    loopback_markers.iter().any(|marker| lower.contains(marker))
        || (lower.contains("wave link")
            && (lower.contains("stream")
                || lower.contains("system")
                || lower.contains("monitor")))
}

fn windows_dshow_audio_input(name: &str) -> String {
    let escaped = name.replace('"', "\\\"");
    format!("audio=\"{escaped}\"")
}

fn parse_windows_dshow_audio_value(input: &str) -> Option<&str> {
    let trimmed = input.trim();
    let (left, right) = trimmed.split_once('=')?;
    if !left.trim().eq_ignore_ascii_case("audio") {
        return None;
    }
    let value = right.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn unquote_windows_dshow_audio_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].replace("\\\"", "\"")
    } else {
        trimmed.to_string()
    }
}

fn windows_dshow_input_candidates(source: &RecordingSource) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_candidate = |candidate: String| {
        let normalized = candidate.trim();
        if normalized.is_empty() {
            return;
        }
        if seen.insert(normalized.to_string()) {
            candidates.push(normalized.to_string());
        }
    };

    push_candidate(source.input.clone());

    if source.format.eq_ignore_ascii_case("dshow") {
        if let Some(value) = parse_windows_dshow_audio_value(&source.input) {
            let raw_value = unquote_windows_dshow_audio_value(value);
            if !raw_value.is_empty() {
                push_candidate(windows_dshow_audio_input(&raw_value));
                push_candidate(format!("audio={raw_value}"));
            }
        }

        let label = source.label.trim();
        if !label.is_empty() {
            push_candidate(windows_dshow_audio_input(label));
            push_candidate(format!("audio={label}"));
        }
    }

    candidates
}
#[cfg(target_os = "windows")]
fn rewrite_windows_dshow_sources_to_friendly_names(
    sources: &[RecordingSource],
) -> Option<Vec<RecordingSource>> {
    let mut changed = false;
    let mut rewritten: Vec<RecordingSource> = Vec::with_capacity(sources.len());

    for source in sources {
        if source.format.eq_ignore_ascii_case("dshow")
            && source.input.to_ascii_lowercase().contains("@device_")
        {
            rewritten.push(RecordingSource {
                label: source.label.clone(),
                format: source.format.clone(),
                input: windows_dshow_audio_input(&source.label),
            });
            changed = true;
        } else {
            rewritten.push(source.clone());
        }
    }

    changed.then_some(rewritten)
}

#[cfg(not(target_os = "windows"))]
fn rewrite_windows_dshow_sources_to_friendly_names(
    _sources: &[RecordingSource],
) -> Option<Vec<RecordingSource>> {
    None
}

#[cfg(target_os = "windows")]
fn probe_windows_dshow_input(candidate_input: &str) -> Result<(), String> {
    let mut child = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostats")
        .arg("-f")
        .arg("dshow")
        .arg("-i")
        .arg(candidate_input)
        .arg("-t")
        .arg("0.30")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg("-f")
        .arg("null")
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("probe spawn failed: {e}"))?;

    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(1_500);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("probe status check failed: {e}"))?
        {
            let mut stderr_text = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut stderr_text);
            }

            if status.success() {
                return Ok(());
            }

            let detail = stderr_text
                .lines()
                .rev()
                .find(|line| {
                    let lower = line.to_ascii_lowercase();
                    lower.contains("error")
                        || lower.contains("failed")
                        || lower.contains("could not find")
                })
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .unwrap_or_else(|| {
                    if stderr_text.trim().is_empty() {
                        "no additional details".to_string()
                    } else {
                        stderr_text.trim().to_string()
                    }
                });

            return Err(format!("probe failed with status {status}: {detail}"));
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }

        thread::sleep(Duration::from_millis(75));
    }
}

#[cfg(not(target_os = "windows"))]
fn probe_windows_dshow_input(_candidate_input: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn resolve_windows_dshow_sources_for_retry(
    sources: &[RecordingSource],
) -> (Vec<RecordingSource>, bool, Vec<String>) {
    let mut resolved_sources = Vec::with_capacity(sources.len());
    let mut changed = false;
    let mut notes: Vec<String> = Vec::new();

    for source in sources {
        if !source.format.eq_ignore_ascii_case("dshow") {
            resolved_sources.push(source.clone());
            continue;
        }

        let candidates = windows_dshow_input_candidates(source);
        let mut chosen_input: Option<String> = None;
        let mut last_probe_error: Option<String> = None;

        for candidate in &candidates {
            match probe_windows_dshow_input(candidate) {
                Ok(()) => {
                    chosen_input = Some(candidate.clone());
                    break;
                }
                Err(error) => {
                    last_probe_error = Some(error);
                }
            }
        }

        if let Some(chosen) = chosen_input {
            if chosen != source.input {
                changed = true;
                notes.push(format!(
                    "Resolved DirectShow source `{}` via fallback input form.",
                    source.label
                ));
            }
            let mut rewritten = source.clone();
            rewritten.input = chosen;
            resolved_sources.push(rewritten);
        } else {
            if let Some(error) = last_probe_error {
                notes.push(format!(
                    "DirectShow probe failed for `{}`: {error}",
                    source.label
                ));
            }
            resolved_sources.push(source.clone());
        }
    }

    (resolved_sources, changed, notes)
}

#[cfg(not(target_os = "windows"))]
fn resolve_windows_dshow_sources_for_retry(
    sources: &[RecordingSource],
) -> (Vec<RecordingSource>, bool, Vec<String>) {
    (sources.to_vec(), false, Vec::new())
}
fn parse_quoted_value(line: &str) -> Option<&str> {
    let first_quote = line.find('"')?;
    let remainder = &line[(first_quote + 1)..];
    let second_quote = remainder.find('"')?;
    Some(remainder[..second_quote].trim())
}

fn parse_macos_recording_devices(joined_output: &str) -> Vec<RecordingDevice> {
    let mut devices: Vec<RecordingDevice> = Vec::new();
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
    let mut devices: Vec<RecordingDevice> = Vec::new();
    let mut in_audio_section = false;
    let mut last_audio_device_index: Option<usize> = None;

    for line in joined_output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("DirectShow audio devices") {
            in_audio_section = true;
            continue;
        }
        if trimmed.contains("DirectShow video devices") {
            in_audio_section = false;
            last_audio_device_index = None;
            continue;
        }

        if trimmed.contains("Alternative name") {
            if let Some(index) = last_audio_device_index {
                if let Some(alternative_name) = parse_quoted_value(trimmed) {
                    if !alternative_name.is_empty() {
                        devices[index].input = windows_dshow_audio_input(alternative_name);
                    }
                }
            }
            continue;
        }

        let Some(name) = parse_quoted_value(trimmed) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }

        let remainder = trimmed
            .split_once('"')
            .and_then(|(_, rest)| rest.split_once('"').map(|(_, suffix)| suffix))
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let has_audio_tag = remainder.contains("(audio)");
        let has_video_tag = remainder.contains("(video)");
        let has_none_tag = remainder.contains("(none)");
        let likely_audio_name = {
            let lower = name.to_ascii_lowercase();
            lower.contains("audio")
                || lower.contains("microphone")
                || lower.contains("mic")
                || is_loopback_device_name(name)
        };

        let is_audio_candidate = if in_audio_section {
            true
        } else if has_audio_tag {
            true
        } else if has_video_tag {
            false
        } else if has_none_tag {
            likely_audio_name
        } else if likely_audio_name {
            // Fallback for ffmpeg builds that omit section/type markers.
            true
        } else {
            false
        };

        if !is_audio_candidate {
            continue;
        }

        let exists = devices
            .iter()
            .any(|item: &RecordingDevice| item.name.eq_ignore_ascii_case(name));
        if exists {
            last_audio_device_index = None;
            continue;
        }

        devices.push(RecordingDevice {
            name: name.to_string(),
            format: "dshow".to_string(),
            input: windows_dshow_audio_input(name),
            is_loopback: is_loopback_device_name(name),
        });
        last_audio_device_index = Some(devices.len() - 1);
    }

    devices
}

#[cfg(target_os = "windows")]
fn list_windows_wasapi_loopback_devices() -> Result<Vec<RecordingDevice>, String> {
    use wasapi::{initialize_mta, DeviceEnumerator, Direction};

    let _ = initialize_mta();
    let enumerator = DeviceEnumerator::new()
        .map_err(|e| format!("Failed to create WASAPI device enumerator: {e}"))?;

    let default_id = enumerator
        .get_default_device(&Direction::Render)
        .ok()
        .and_then(|device| device.get_id().ok());

    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|e| format!("Failed to query playback endpoints: {e}"))?;
    let count = collection
        .get_nbr_devices()
        .map_err(|e| format!("Failed to count playback endpoints: {e}"))?;

    let mut devices: Vec<RecordingDevice> = Vec::new();
    for index in 0..count {
        let device = collection
            .get_device_at_index(index)
            .map_err(|e| format!("Failed to open playback endpoint #{index}: {e}"))?;

        let id = device
            .get_id()
            .map_err(|e| format!("Failed to read playback endpoint id #{index}: {e}"))?;
        let name = device
            .get_friendlyname()
            .unwrap_or_else(|_| format!("Playback Device {}", index + 1));
        let is_default = default_id
            .as_ref()
            .map(|default_id| default_id == &id)
            .unwrap_or(false);

        devices.push(RecordingDevice {
            name: if is_default {
                format!("{name} (Default Playback)")
            } else {
                name
            },
            format: "wasapi_loopback".to_string(),
            input: id,
            is_loopback: true,
        });
    }

    Ok(devices)
}

#[cfg(not(target_os = "windows"))]
fn list_windows_wasapi_loopback_devices() -> Result<Vec<RecordingDevice>, String> {
    Ok(Vec::new())
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
    let ffmpeg_available = find_executable("ffmpeg");

    if !ffmpeg_available && !cfg!(target_os = "windows") {
        if let Some(native) = native_system_recording_device() {
            return Ok(vec![native]);
        }
        return Err("ffmpeg not found in PATH".to_string());
    }

    let mut devices = if cfg!(target_os = "macos") {
        let output = Command::new("ffmpeg")
            .arg("-f")
            .arg("avfoundation")
            .arg("-list_devices")
            .arg("true")
            .arg("-i")
            .arg("")
            .output()
            .map_err(|e| format!("Failed to query ffmpeg avfoundation devices: {e}"))?;

        let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout_text = String::from_utf8_lossy(&output.stdout).to_string();
        let joined = format!("{stderr_text}\n{stdout_text}");
        parse_macos_recording_devices(&joined)
    } else if cfg!(target_os = "windows") {
        let mut devices = list_windows_wasapi_loopback_devices().unwrap_or_else(|error| {
            eprintln!("WASAPI playback enumeration failed: {error}");
            Vec::new()
        });

        if ffmpeg_available {
            let output = Command::new("ffmpeg")
                .arg("-list_devices")
                .arg("true")
                .arg("-f")
                .arg("dshow")
                .arg("-i")
                .arg("dummy")
                .output()
                .map_err(|e| format!("Failed to query ffmpeg dshow devices: {e}"))?;

            let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout_text = String::from_utf8_lossy(&output.stdout).to_string();
            let joined = format!("{stderr_text}\n{stdout_text}");

            let lower = joined.to_ascii_lowercase();
            if lower.contains("unknown input format: 'dshow'")
                || lower.contains("unknown input format: \"dshow\"")
            {
                return Err(
                    "ffmpeg build is missing DirectShow (dshow) support. Install a Windows ffmpeg build with DirectShow enabled."
                        .to_string(),
                );
            }

            let mut dshow_devices = parse_windows_recording_devices(&joined);
            devices.append(&mut dshow_devices);
        }

        if !ffmpeg_available && devices.is_empty() {
            return Err(
                "ffmpeg not found in PATH and no WASAPI playback devices were detected.".to_string(),
            );
        }

        devices
    } else {
        let output = Command::new("ffmpeg")
            .arg("-sources")
            .arg("pulse")
            .output()
            .map_err(|e| format!("Failed to query ffmpeg audio sources: {e}"))?;

        let _stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
        let _stdout_text = String::from_utf8_lossy(&output.stdout).to_string();
        Vec::new()
    };

    if let Some(native) = native_system_recording_device() {
        devices.insert(0, native);
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

fn build_audio_device_hints(
    devices: &[RecordingDevice],
    ffmpeg_available: bool,
    has_native_source: bool,
) -> Vec<String> {
    let mut hints = Vec::new();

    if has_native_source {
        hints.push(
            "Native system source available: select \"System Audio (macOS Native)\" for ScreenCaptureKit-based capture."
                .to_string(),
        );
    }

    let has_wasapi_playback = cfg!(target_os = "windows")
        && devices
            .iter()
            .any(|device| device.format.eq_ignore_ascii_case("wasapi_loopback"));

    if !ffmpeg_available {
        if cfg!(target_os = "windows") {
            if has_wasapi_playback {
                hints.push(
                    "Playback devices detected via WASAPI loopback. You can record system audio without VB-CABLE/Stereo Mix."
                        .to_string(),
                );
            } else {
                hints.push(
                    "No playback loopback device detected via WASAPI. Check Windows sound devices and restart audio services, then refresh."
                        .to_string(),
                );
            }
            hints.push(
                "ffmpeg not found in PATH. Install ffmpeg to enable microphone capture and multi-source audio mixing on Windows."
                    .to_string(),
            );
        } else {
            hints.push("ffmpeg not found in PATH. Install ffmpeg and restart the app.".to_string());
        }
        if devices.is_empty() {
            hints.push(
                "No recording devices detected. Verify microphone/loopback devices in system settings, then refresh."
                    .to_string(),
            );
        }
        return hints;
    }

    if devices.is_empty() {
        hints.push(
            "No recording devices detected. Verify microphone/loopback devices in system settings, then refresh."
                .to_string(),
        );
        return hints;
    }

    let mut seen_names = BTreeSet::new();
    let mut ordered_names = Vec::new();
    for device in devices {
        let trimmed = device.name.trim();
        if trimmed.is_empty() {
            continue;
        }

        let normalized = trimmed.to_ascii_lowercase();
        if seen_names.insert(normalized) {
            ordered_names.push(trimmed.to_string());
        }
    }

    let loopback_count = devices.iter().filter(|device| device.is_loopback).count();
    if cfg!(target_os = "windows") {
        if has_wasapi_playback {
            hints.push(
                "WASAPI playback capture is available. Pick your speakers/headphones source to capture app/system audio."
                    .to_string(),
            );
        }

        if loopback_count == 0 {
            hints.push(
                "No loopback source detected. Enable Stereo Mix or install VB-CABLE to capture app/system audio on Windows."
                    .to_string(),
            );
            hints.push("You can still record microphone-only by selecting a non-loopback source.".to_string());
            hints.push(
                "DirectShow does not expose speaker/headphone endpoints. Use WASAPI playback sources above, or route output through Stereo Mix/VB-CABLE/Wave Link Stream."
                    .to_string(),
            );
        } else {
            hints.push(
                "Tip: use one loopback source for system audio and one microphone source for full-call capture."
                    .to_string(),
            );
        }
    } else if cfg!(target_os = "macos") && has_native_source {
        hints.push(
            "Tip: select \"System Audio (macOS Native)\" plus one microphone source for full-call capture."
                .to_string(),
        );
    }

    if !ordered_names.is_empty() {
        let preview: Vec<String> = ordered_names.iter().take(4).cloned().collect();
        hints.push(format!("Detected devices: {}", preview.join(", ")));
        let hidden_count = ordered_names.len().saturating_sub(preview.len());
        if hidden_count > 0 {
            hints.push(format!("{hidden_count} additional device(s) available in the source selector."));
        }
    }

    hints
}

#[tauri::command]
fn list_audio_device_hints() -> Result<Vec<String>, String> {
    let ffmpeg_available = find_executable("ffmpeg");
    let has_native_source = native_system_recording_device().is_some();

    match list_recording_devices() {
        Ok(devices) => Ok(build_audio_device_hints(
            &devices,
            ffmpeg_available,
            has_native_source,
        )),
        Err(error) => {
            let mut hints = build_audio_device_hints(&[], ffmpeg_available, has_native_source);
            hints.push(format!("Device query failed: {error}"));
            Ok(hints)
        }
    }
}

#[tauri::command]
fn list_recording_devices_with_hints() -> Result<RecordingDevicesWithHints, String> {
    let ffmpeg_available = find_executable("ffmpeg");
    let has_native_source = native_system_recording_device().is_some();
    let devices = list_recording_devices()?;
    let hints = build_audio_device_hints(&devices, ffmpeg_available, has_native_source);
    Ok(RecordingDevicesWithHints { devices, hints })
}

#[tauri::command]
fn recording_meter(session_id: String, state: State<'_, AppState>) -> Result<RecordingMeter, String> {
    let (active_output_path, existing_path, telemetry) = {
        let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        let session = sessions
            .get(&session_id)
            .ok_or_else(|| "Recording session not found".to_string())?;
        (
            session.process.as_ref().map(|process| process.output_path.clone()),
            session.existing_path.clone(),
            session
                .process
                .as_ref()
                .map(|process| Arc::clone(&process.telemetry)),
        )
    };

    let file_bytes = active_output_path
        .as_ref()
        .and_then(|path| fs::metadata(path).ok().map(|meta| meta.len()))
        .or_else(|| {
            existing_path
                .as_ref()
                .and_then(|path| fs::metadata(path).ok().map(|meta| meta.len()))
        })
        .unwrap_or(0);

    if let Some(telemetry) = telemetry {
        let mut state = telemetry.lock().map_err(|e| e.to_string())?;
        if file_bytes > state.bytes_written {
            state.bytes_written = file_bytes;
        }
        let reported_level = state.level.clamp(0.0, 1.0);
        state.level = (state.level * 0.72).clamp(0.0, 1.0);
        if state.level < 0.001 {
            state.level = 0.0;
        }
        return Ok(RecordingMeter {
            bytes_written: state.bytes_written,
            level: reported_level,
        });
    }

    Ok(RecordingMeter {
        bytes_written: file_bytes,
        level: 0.0,
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
        whisper_model: whisper_model_name(&conn)?,
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
    let source_analysis = analyze_recording_sources(
        &sources,
        cfg!(target_os = "macos"),
        supports_native_system_audio_capture(),
        supports_native_system_audio_plus_microphone(),
    )?;

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

    // ffmpeg is required for the non-native capture path, for append concatenation,
    // and for native system+microphone final mixing.
    let has_existing_path = existing_path.as_ref().map(|path| path.exists()).unwrap_or(false);
    let requires_ffmpeg = ffmpeg_required_for_recording_sources(&sources, source_analysis, has_existing_path);
    if requires_ffmpeg && !find_executable("ffmpeg") {
        return Err("ffmpeg not found in PATH. Install ffmpeg to enable this recording mode.".to_string());
    }

    let process = spawn_recording_process(
        &base_data_dir,
        &entry_directory,
        &sources,
        source_analysis,
        has_existing_path,
    )?;

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
            sources,
            source_analysis,
            existing_path,
            process: Some(process),
            paused: false,
        },
    );

    Ok(session_id)
}

#[tauri::command]
fn stop_recording(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let (entry_id, recording_path, duration_sec) = {
        let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| "Recording session not found".to_string())?;

        if session.process.is_some() {
            finalize_active_recording_process(session)?;
        }

        let final_path = session
            .existing_path
            .as_ref()
            .filter(|path| path.exists())
            .cloned()
            .ok_or_else(|| format!("Recording file was not created. {}", recording_runtime_failure_hint()))?;

        if !recording_contains_audio_payload(&final_path)
            || !recording_contains_audible_signal(&final_path)
        {
            return Err(
                "Recording captured no audible data. Check source routing/permissions and try again while audio is playing."
                    .to_string(),
            );
        }

        let recording_path = final_path.to_string_lossy().to_string();
        let duration_sec = probe_duration_seconds(&recording_path);
        (session.entry_id.clone(), recording_path, duration_sec)
    };

    let db = db_path(&state)?;
    let conn = connection(&db)?;
    conn.execute(
        "UPDATE entries
         SET status = 'recorded', recording_path = ?1, duration_sec = ?2, updated_at = ?3
         WHERE id = ?4",
        params![recording_path, duration_sec, now_ts(), entry_id],
    )
    .map_err(|e| format!("Failed to finalize recording entry state: {e}"))?;

    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    sessions.remove(&session_id);

    Ok(())
}
#[tauri::command]
fn set_recording_paused(session_id: String, paused: bool, state: State<'_, AppState>) -> Result<(), String> {
    let base_data_dir = data_dir(&state)?;

    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Recording session not found".to_string())?;

    if session.paused == paused {
        return Ok(());
    }

    if paused {
        finalize_active_recording_process(session)?;
        session.paused = true;
        return Ok(());
    }

    if session.process.is_some() {
        session.paused = false;
        return Ok(());
    }

    let entry_directory = ensure_entry_dirs(&base_data_dir, &session.entry_id)?;
    let has_existing_path = session
        .existing_path
        .as_ref()
        .map(|path| path.exists())
        .unwrap_or(false);
    let requires_ffmpeg = ffmpeg_required_for_recording_sources(&session.sources, session.source_analysis, has_existing_path);
    if requires_ffmpeg && !find_executable("ffmpeg") {
        return Err("ffmpeg not found in PATH. Install ffmpeg to enable this recording mode.".to_string());
    }

    let process = spawn_recording_process(
        &base_data_dir,
        &entry_directory,
        &session.sources,
        session.source_analysis,
        has_existing_path,
    )?;

    session.process = Some(process);
    session.paused = false;
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

    if !recording_contains_audio_payload(Path::new(&recording_path))
        || !recording_contains_audible_signal(Path::new(&recording_path))
    {
        return Err("Recording contains no audible signal. Re-record with active audio and verify your selected sources.".to_string());
    }

    let base_data_dir = data_dir(&state)?;
    let entry_directory = ensure_entry_dirs(&base_data_dir, &entry_id)?;
    let transcript_dir = entry_directory.join("transcript");
    let output_base = transcript_dir.join(format!("tmp_{}", unix_now()));
    let preferred_model = whisper_model_name(&conn)?;
    let use_whisper_cpp = whisper_model_looks_like_cpp(&preferred_model);
    let language_requested_raw = language
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let language_requested = normalize_transcription_language(&language_requested_raw);

    let mut command = if use_whisper_cpp {
        if !find_executable("whisper-cli") {
            return Err(
                "Selected Whisper model is a whisper.cpp model (*.bin), but `whisper-cli` is not available in PATH."
                    .to_string(),
            );
        }
        Command::new("whisper-cli")
    } else {
        if !find_executable("whisper") {
            return Err(
                "Selected Whisper model requires OpenAI Whisper CLI (`whisper`). Install it (for example `pipx install openai-whisper`) and try again."
                    .to_string(),
            );
        }
        Command::new("whisper")
    };

    if use_whisper_cpp {
        let model_path = resolve_whisper_model_path(&base_data_dir, Some(&preferred_model))?;
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
        command.arg("--language").arg(&language_requested);
    } else {
        command.arg(&recording_path);
        command.arg("--model").arg(preferred_model.trim());
        command.arg("--task").arg("transcribe");
        command.arg("--output_format").arg("txt");
        command.arg("--output_dir").arg(transcript_dir.to_string_lossy().to_string());
        if !language_requested.eq_ignore_ascii_case("auto") {
            command.arg("--language").arg(&language_requested);
        }
    }

    let output = command
        .output()
        .map_err(|e| format!("Failed to run Whisper command: {e}"))?;
    let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout_text = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        return Err(format!("Whisper transcription failed: {stderr_text}"));
    }

    let transcript_path = if use_whisper_cpp {
        output_base.with_extension("txt")
    } else {
        let expected = transcript_dir.join(
            Path::new(&recording_path)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("recording")
                .to_string()
                + ".txt",
        );
        if expected.exists() {
            expected
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
        }
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
    let mut language_value = normalize_transcription_language(
        &language.unwrap_or_else(|| "auto".to_string()),
    );
    if language_value.eq_ignore_ascii_case("auto") {
        if let Some(detected) = parse_whisper_detected_language(&stderr_text)
            .or_else(|| parse_openai_whisper_detected_language(&stderr_text))
            .or_else(|| parse_openai_whisper_detected_language(&stdout_text))
        {
            language_value = normalize_transcription_language(&detected);
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
    let artifact_name = match artifact_type.as_str() {
        "summary" => "summary",
        "analysis" => "analysis",
        "critique_recruitment" => "recruitment critique",
        "critique_sales" => "sales critique",
        "critique_cs" => "customer success critique",
        _ => "artifact",
    };

    let full_prompt = format!(
        "You are generating a {artifact_name} from a call transcript.\n\
INSTRUCTIONS (internal, do not repeat or quote):\n{prompt_template}\n\n\
OUTPUT RULES:\n\
- Return markdown only.\n\
- Do not include meta text about your instructions.\n\
- Do not copy instruction headings or labels unless they appear in the transcript itself.\n\
- Base the result only on transcript content.\n\n\
Transcript (language={}):\n{}\n",
        transcript.language, transcript.text
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
fn prepare_ai_backend(state: State<'_, AppState>) -> Result<String, String> {
    let db = db_path(&state)?;
    let conn = connection(&db)?;
    let model = model_name(&conn)?;
    let readiness = ensure_ollama_ready(&model, true)?;
    if readiness == "ready" {
        Ok(format!("AI backend ready ({model})"))
    } else {
        Ok(readiness)
    }
}

#[tauri::command]
fn list_whisper_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let mut models = BTreeSet::new();
    for model in OPENAI_WHISPER_MODELS {
        models.insert((*model).to_string());
    }
    let base_data_dir = data_dir(&state)?;
    let mut roots = vec![base_data_dir.join("models")];

    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("models"));
        roots.push(cwd.join("..").join("models"));
    }

    for root in roots {
        if !root.exists() {
            continue;
        }
        let Ok(read_dir) = fs::read_dir(&root) else {
            continue;
        };
        for item in read_dir.flatten() {
            let path = item.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !file_name.starts_with("ggml-") || !file_name.ends_with(".bin") {
                continue;
            }
            models.insert(file_name.to_string());
        }
    }

    if models.is_empty() {
        models.insert(DEFAULT_WHISPER_MODEL.to_string());
    }
    Ok(models.into_iter().collect())
}

#[tauri::command]
fn update_whisper_model(model_name: String, state: State<'_, AppState>) -> Result<(), String> {
    let trimmed = model_name.trim();
    if trimmed.is_empty() {
        return Err("Whisper model name cannot be empty".to_string());
    }

    let db = db_path(&state)?;
    let conn = connection(&db)?;

    conn.execute(
        "INSERT INTO settings(key, value, updated_at) VALUES(?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![WHISPER_MODEL_KEY, trimmed, now_ts()],
    )
    .map_err(|e| format!("Failed to update whisper model: {e}"))?;

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
            list_recording_devices_with_hints,
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
            prepare_ai_backend,
            list_whisper_models,
            update_whisper_model,
            export_entry_markdown
        ])
        .run(tauri::generate_context!())
        .expect("error while running AI Transcribe Local");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn source(format: &str, input: &str) -> RecordingSource {
        RecordingSource {
            label: format!("{format}:{input}"),
            format: format.to_string(),
            input: input.to_string(),
        }
    }

    #[test]
    fn analyze_recording_sources_requires_sources() {
        let error = analyze_recording_sources(&[], true, true, true).unwrap_err();
        assert_eq!(error, "At least one audio source is required");
    }

    #[test]
    fn analyze_recording_sources_rejects_native_on_non_macos() {
        let sources = vec![source("screencapturekit", "system")];
        let error = analyze_recording_sources(&sources, false, false, false).unwrap_err();
        assert_eq!(
            error,
            "Native system-audio source is currently available only on macOS"
        );
    }

    #[test]
    fn analyze_recording_sources_rejects_native_plus_multiple_non_native() {
        let sources = vec![
            source("screencapturekit", "system"),
            source("avfoundation", ":0"),
            source("avfoundation", ":1"),
        ];
        let error = analyze_recording_sources(&sources, true, true, true).unwrap_err();
        assert_eq!(
            error,
            "With System Audio (macOS Native), select at most one additional microphone source."
        );
    }

    #[test]
    fn analyze_recording_sources_calculates_ffmpeg_requirement() {
        let native_only = vec![source("screencapturekit", "system")];
        let native = analyze_recording_sources(&native_only, true, true, true).unwrap();
        assert!(native.has_native_system_source);
        assert!(!native.native_with_microphone);
        assert!(!native.requires_ffmpeg(false));
        assert!(native.requires_ffmpeg(true));

        let mic_only = vec![source("avfoundation", ":0")];
        let non_native = analyze_recording_sources(&mic_only, true, true, true).unwrap();
        assert!(!non_native.has_native_system_source);
        assert!(non_native.requires_ffmpeg(false));
    }

    #[test]
    fn recording_output_paths_new_file_with_native_mic() {
        let entry_dir = Path::new("/tmp/entry-under-test");
        let (output, native_mic) = recording_output_paths(entry_dir, false, true, 42);
        assert_eq!(output, entry_dir.join("audio").join("original.wav"));
        assert_eq!(
            native_mic,
            Some(entry_dir.join("audio").join("original-microphone.wav"))
        );
    }

    #[test]
    fn recording_output_paths_segment_file_with_native_mic() {
        let entry_dir = Path::new("/tmp/entry-under-test");
        let (output, native_mic) = recording_output_paths(entry_dir, true, true, 77);
        assert_eq!(output, entry_dir.join("audio").join("segment-77.wav"));
        assert_eq!(
            native_mic,
            Some(entry_dir.join("audio").join("segment-77-microphone.wav"))
        );
    }

    #[test]
    fn ffmpeg_recording_filter_graph_single_and_multi_source() {
        let single = ffmpeg_recording_filter_graph(1);
        assert_eq!(
            single,
            "[0:a]astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level[mout]"
        );

        let multi = ffmpeg_recording_filter_graph(2);
        assert!(multi.contains("[0:a][1:a]amix=inputs=2"));
        assert!(multi.contains("[mix]astats=metadata=1:reset=1"));
        assert!(multi.ends_with("[mout]"));
    }

    #[test]
    fn normalize_transcription_language_handles_detected_russian() {
        assert_eq!(normalize_transcription_language("russian"), "ru");
        assert_eq!(normalize_transcription_language("Russian"), "ru");
        assert_eq!(normalize_transcription_language("ru"), "ru");
    }

    #[test]
    fn normalize_transcription_language_title_cases_unknown_names() {
        assert_eq!(
            normalize_transcription_language("haitian creole"),
            "Haitian Creole"
        );
    }

    #[test]
    fn parse_openai_whisper_detected_language_supports_multi_word_names() {
        let log = "Detected language: Haitian Creole (0.99)";
        assert_eq!(
            parse_openai_whisper_detected_language(log),
            Some("haitian creole".to_string())
        );
    }

    #[test]
    fn parse_windows_recording_devices_extracts_unique_audio_inputs() {
        let output = r#"
[dshow @ 000001] DirectShow audio devices
[dshow @ 000001]  "Microphone (USB Audio Device)"
[dshow @ 000001]  "Stereo Mix (Realtek Audio)"
[dshow @ 000001]     Alternative name "@device_cm_{123}"
[dshow @ 000001]  "Microphone (USB Audio Device)"
"#;

        let devices = parse_windows_recording_devices(output);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Microphone (USB Audio Device)");
        assert_eq!(devices[0].format, "dshow");
        assert_eq!(devices[0].input, "audio=\"Microphone (USB Audio Device)\"");
        assert!(!devices[0].is_loopback);

        assert_eq!(devices[1].name, "Stereo Mix (Realtek Audio)");
        assert_eq!(devices[1].input, "audio=\"@device_cm_{123}\"");
        assert!(devices[1].is_loopback);
    }

    #[test]
    fn parse_windows_recording_devices_prefers_alternative_name_for_dshow_input() {
        let output = r#"
[dshow @ 000001] DirectShow audio devices
[dshow @ 000001]  "Microphone (USB Audio Device)"
[dshow @ 000001]     Alternative name "@device_cm_{abc}"
"#;

        let devices = parse_windows_recording_devices(output);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Microphone (USB Audio Device)");
        assert_eq!(devices[0].input, "audio=\"@device_cm_{abc}\"");
    }

    #[test]
    fn parse_windows_recording_devices_supports_inline_type_markers() {
        let output = r#"
[dshow @ 000001] "OBS Virtual Camera" (video)
[dshow @ 000001] "Game Capture HD60 S Audio" (audio)
[dshow @ 000001] "USB Microphone" (none)
[dshow @ 000001] "Integrated Camera" (none)
"#;

        let devices = parse_windows_recording_devices(output);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Game Capture HD60 S Audio");
        assert_eq!(devices[1].name, "USB Microphone");
        assert!(devices.iter().all(|device| !device.name.contains("Camera")));
    }

    #[test]
    fn parse_windows_recording_devices_supports_markerless_audio_names() {
        let output = r#"
[dshow @ 000001] "Wave Link Stream (Elgato Wave:3)"
[dshow @ 000001] "USB Microphone"
[dshow @ 000001] "OBS Virtual Camera"
"#;

        let devices = parse_windows_recording_devices(output);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Wave Link Stream (Elgato Wave:3)");
        assert_eq!(devices[1].name, "USB Microphone");
    }

    #[test]
    fn parse_windows_recording_devices_quotes_colon_names() {
        let output = r#"
[dshow @ 000001] "Wave Link Stream (Elgato Wave:3)" (none)
"#;

        let devices = parse_windows_recording_devices(output);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Wave Link Stream (Elgato Wave:3)");
        assert_eq!(devices[0].input, "audio=\"Wave Link Stream (Elgato Wave:3)\"");
        assert!(devices[0].is_loopback);
    }

    #[test]
    fn windows_dshow_input_candidates_include_friendly_and_unquoted_variants() {
        let source = RecordingSource {
            label: "Wave Link Stream (Elgato Wave:3)".to_string(),
            format: "dshow".to_string(),
            input: "audio=\"@device_cm_{abc}\"".to_string(),
        };

        let candidates = windows_dshow_input_candidates(&source);
        assert_eq!(candidates[0], "audio=\"@device_cm_{abc}\"");
        assert!(candidates
            .iter()
            .any(|candidate| candidate == "audio=@device_cm_{abc}"));
        assert!(candidates
            .iter()
            .any(|candidate| candidate == "audio=\"Wave Link Stream (Elgato Wave:3)\""));
        assert!(candidates
            .iter()
            .any(|candidate| candidate == "audio=Wave Link Stream (Elgato Wave:3)"));
    }

    #[test]
    fn parse_windows_dshow_audio_value_requires_audio_prefix() {
        assert_eq!(parse_windows_dshow_audio_value("video=Camera"), None);
        assert_eq!(parse_windows_dshow_audio_value("audio=   "), None);
        assert_eq!(
            parse_windows_dshow_audio_value("audio=\"Microphone\""),
            Some("\"Microphone\"")
        );
    }

    #[test]
    fn build_audio_device_hints_warns_when_windows_loopback_missing() {
        let devices = vec![RecordingDevice {
            name: "Microphone Array".to_string(),
            format: "dshow".to_string(),
            input: "audio=Microphone Array".to_string(),
            is_loopback: false,
        }];

        let hints = build_audio_device_hints(&devices, true, false);
        if cfg!(target_os = "windows") {
            assert!(
                hints
                    .iter()
                    .any(|hint| hint.contains("No loopback source detected"))
            );
        } else {
            assert!(
                hints
                    .iter()
                    .all(|hint| !hint.contains("No loopback source detected"))
            );
        }
    }

}















