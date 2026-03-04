import { useEffect, useMemo, useState } from "react";
import { api } from "./lib/api";
import beyondLogo from "./assets/beyond-logo.png";
import type {
  ArtifactRevision,
  ArtifactType,
  BootstrapState,
  Entry,
  EntryBundle,
  Folder,
  PromptRole,
  RecordingDevice,
  RecordingSource
} from "./lib/types";
import "./styles/app.css";

const CRITIQUE_ROLES: { role: PromptRole; label: string }[] = [
  { role: "critique_recruitment", label: "Recruitment Critique Prompt" },
  { role: "critique_sales", label: "Sales Critique Prompt" },
  { role: "critique_cs", label: "Customer Success Critique Prompt" }
];
const SUMMARY_PROMPT: { role: PromptRole; label: string } = {
  role: "summary",
  label: "Summary Prompt"
};

const ARTIFACT_TYPES: { type: ArtifactType; label: string }[] = [
  { type: "summary", label: "Summarize" },
  { type: "analysis", label: "Analyze" },
  { type: "critique_recruitment", label: "Critique: Recruitment" },
  { type: "critique_sales", label: "Critique: Sales" },
  { type: "critique_cs", label: "Critique: Customer Success" }
];

const TRANSCRIPTION_LANGUAGES: { value: string; label: string }[] = [
  { value: "auto", label: "Auto detect" },
  { value: "ru", label: "Russian" },
  { value: "en", label: "English" },
  { value: "uk", label: "Ukrainian" },
  { value: "es", label: "Spanish" },
  { value: "de", label: "German" },
  { value: "fr", label: "French" }
];

const WHISPER_MODEL_PRESETS: string[] = [
  "turbo",
  "large-v3",
  "large-v2",
  "large",
  "medium",
  "medium.en",
  "small",
  "small.en",
  "base",
  "base.en",
  "tiny",
  "tiny.en",
  "ggml-base.bin",
  "ggml-tiny.bin",
  "ggml-base.en.bin",
  "ggml-tiny.en.bin"
];

const CRITIQUE_ACTIONS: { type: ArtifactType; label: string }[] = [
  { type: "critique_recruitment", label: "Recruitment" },
  { type: "critique_sales", label: "Sales" },
  { type: "critique_cs", label: "Customer Success" }
];

const RU_TRANSLATIONS: Record<string, string> = {
  "AI Call Recorder Local": "AI Запись Звонков Локально",
  "Local Model & Prompt Settings": "Настройки Моделей и Промптов",
  Settings: "Настройки",
  Trash: "Корзина",
  "Close settings": "Закрыть настройки",
  "Close trash": "Закрыть корзину",
  "Interface Language": "Язык интерфейса",
  "Ollama Model Name": "Название модели Ollama",
  "Save Model": "Сохранить модель",
  "Model name updated": "Название модели обновлено",
  "Whisper Model": "Модель Whisper",
  "Custom Whisper Model (optional)": "Пользовательская модель Whisper (необязательно)",
  "Save Whisper Model": "Сохранить модель Whisper",
  "Refresh Whisper Models": "Обновить модели Whisper",
  "Whisper model updated": "Модель Whisper обновлена",
  "Whisper models refreshed": "Список моделей Whisper обновлен",
  "Use turbo/large-v3 with OpenAI Whisper CLI (whisper), or use local ggml-*.bin models with whisper-cli.":
    "Используйте turbo/large-v3 с OpenAI Whisper CLI (whisper) или локальные модели ggml-*.bin с whisper-cli.",
  "Summary Prompt": "Промпт для Саммари",
  "Recruitment Critique Prompt": "Промпт критики: Рекрутинг",
  "Sales Critique Prompt": "Промпт критики: Продажи",
  "Customer Success Critique Prompt": "Промпт критики: Customer Success",
  "Save Prompt": "Сохранить промпт",
  "Folders": "Папки",
  Entries: "Записи",
  Restore: "Восстановить",
  Purge: "Удалить навсегда",
  Workspace: "Рабочее пространство",
  "Entry Detail": "Детали записи",
  "Select an entry to work on recording and AI tasks.": "Выберите запись для работы с записью звонка и AI задачами.",
  Status: "Статус",
  recording: "запись",
  recorded: "записано",
  transcribed: "транскрибировано",
  processed: "обработано",
  edited: "изменено",
  paused: "пауза",
  transcribing: "транскрибация",
  Duration: "Длительность",
  "Refresh devices": "Обновить устройства",
  "Audio devices refreshed": "Аудиоустройства обновлены",
  "Add source": "Добавить источник",
  "Remove source": "Удалить источник",
  "Start recording": "Начать запись",
  "Recording started": "Запись начата",
  Pause: "Пауза",
  "Recording paused": "Запись на паузе",
  Resume: "Продолжить",
  "Recording resumed": "Запись продолжена",
  "Stop Recording": "Остановить запись",
  "Recording stopped. Transcribing...": "Запись остановлена. Выполняется транскрибация...",
  "Recording stopped and transcribed": "Запись остановлена и транскрибирована",
  Transcribe: "Транскрибировать",
  Summarize: "Саммари",
  Analyze: "Анализ",
  Critique: "Критика",
  Export: "Экспорт",
  "Transcription ready": "Транскрибация готова",
  "Summarize completed": "Саммари готово",
  "Analyze completed": "Анализ готов",
  "Recording in progress": "Идет запись",
  "Transcribing latest recording": "Транскрибация последней записи",
  "Signal level": "Уровень сигнала",
  Captured: "Записано",
  "Transcription Language": "Язык транскрибации",
  "Auto detect": "Автоопределение",
  Russian: "Русский",
  English: "Английский",
  Ukrainian: "Украинский",
  Spanish: "Испанский",
  German: "Немецкий",
  French: "Французский",
  Version: "Версия",
  Language: "Язык",
  Updated: "Обновлено",
  Transcript: "Транскрипт",
  "Transcript text": "Текст транскрипта",
  "Transcript saved": "Транскрипт сохранен",
  Artifacts: "Артефакты",
  stale: "устарело",
  Save: "Сохранить",
  Recruitment: "Рекрутинг",
  Sales: "Продажи",
  "Customer Success": "Customer Success",
  "Critique: Recruitment": "Критика: Рекрутинг",
  "Critique: Sales": "Критика: Продажи",
  "Critique: Customer Success": "Критика: Customer Success",
  "Add subfolder": "Добавить подпапку",
  "Add folder": "Добавить папку",
  "Add entry": "Добавить запись",
  "Rename selected": "Переименовать выбранное",
  "Delete selected": "Удалить выбранное",
  "Entry name": "Название записи",
  "Subfolder, entry, or rename": "Подпапка, запись или новое имя",
  "Workspace item name": "Название элемента",
  Subfolder: "Подпапка",
  Folder: "Папка",
  Entry: "Запись",
  "Folder created": "Папка создана",
  "Entry created": "Запись создана",
  "Type a new name first, then press rename.": "Сначала введите новое имя, затем нажмите переименовать.",
  "Entry renamed": "Запись переименована",
  "Folder renamed": "Папка переименована",
  "Select a folder first": "Сначала выберите папку",
  "Select a folder or entry first.": "Сначала выберите папку или запись.",
  "Entry moved to trash": "Запись перемещена в корзину",
  "Folder moved to trash": "Папка перемещена в корзину",
  "Audio device detection timed out. You can still use the app and retry refresh.":
    "Истекло время ожидания определения аудиоустройств. Вы можете продолжить работу и обновить позже.",
  "AI backend is not ready yet:": "AI-бэкенд пока не готов:",
  "AI Prompts": "AI Промпты",
  "Export created at": "Экспорт создан в",
  "updated": "обновлено",
  "Save Summary Prompt": "Сохранить промпт для Саммари",
  "Summary Prompt updated": "Промпт для Саммари обновлен",
  "Recording signal meter": "Индикатор сигнала записи",
  "Save Transcript": "Сохранить транскрипт",
  completed: "завершено",
  saved: "сохранено",
  Summary: "Саммари",
  Analysis: "Анализ",
  Ready: "Готово",
  Home: "Главная",
  "Model Settings": "Настройки моделей",
  "Prompt Templates": "Шаблоны промптов",
  "No deleted folders": "Удаленных папок нет",
  "No deleted entries": "Удаленных записей нет",
  "turbo | large-v3 | ggml-base.bin | /path/to/model.bin":
    "turbo | large-v3 | ggml-base.bin | /путь/к/модели.bin",
  new: "новая"
};

type IconName =
  | "folder-plus"
  | "entry-plus"
  | "edit"
  | "trash"
  | "settings"
  | "refresh"
  | "remove"
  | "folder"
  | "entry"
  | "mic"
  | "plus"
  | "chevron-right"
  | "arrow-right"
  | "arrow-left"
  | "play"
  | "save"
  | "more"
  | "file-text"
  | "sparkles"
  | "brain"
  | "target";

function Icon({ name }: { name: IconName }) {
  switch (name) {
    case "folder-plus":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M3 7h7l2 2h9v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" fill="none" stroke="currentColor" strokeWidth="1.8" />
          <path d="M12 12v5M9.5 14.5h5" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      );
    case "entry-plus":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 3h7l4 4v14H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" fill="none" stroke="currentColor" strokeWidth="1.8" />
          <path d="M14 3v4h4" fill="none" stroke="currentColor" strokeWidth="1.8" />
          <path d="M12 11v6M9 14h6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      );
    case "folder":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M3 7h7l2 2h9v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" fill="none" stroke="currentColor" strokeWidth="1.8" />
        </svg>
      );
    case "entry":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 3h7l4 4v14H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" fill="none" stroke="currentColor" strokeWidth="1.8" />
          <path d="M14 3v4h4" fill="none" stroke="currentColor" strokeWidth="1.8" />
        </svg>
      );
    case "edit":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M4 20l4.5-1 10-10a1.8 1.8 0 0 0 0-2.5l-1-1a1.8 1.8 0 0 0-2.5 0l-10 10L4 20z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
          <path d="M13.5 7.5l3 3" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      );
    case "trash":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M4 7h16M9 7V5h6v2M7 7l1 13h8l1-13" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          <path d="M10 11v6M14 11v6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      );
    case "settings":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M10.3 3.8h3.4l.5 2.1c.4.1.8.3 1.2.5l1.9-1.1 2.4 2.4-1.1 1.9c.2.4.4.8.5 1.2l2.1.5v3.4l-2.1.5c-.1.4-.3.8-.5 1.2l1.1 1.9-2.4 2.4-1.9-1.1c-.4.2-.8.4-1.2.5l-.5 2.1h-3.4l-.5-2.1c-.4-.1-.8-.3-1.2-.5l-1.9 1.1-2.4-2.4 1.1-1.9c-.2-.4-.4-.8-.5-1.2l-2.1-.5v-3.4l2.1-.5c.1-.4.3-.8.5-1.2l-1.1-1.9 2.4-2.4 1.9 1.1c.4-.2.8-.4 1.2-.5z" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
          <circle cx="12" cy="12" r="2.8" fill="none" stroke="currentColor" strokeWidth="1.7" />
        </svg>
      );
    case "refresh":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M19 8a8 8 0 1 0 2 5.3" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          <path d="M19 3v5h-5" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case "mic":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <rect x="9" y="3.5" width="6" height="11" rx="3" fill="none" stroke="currentColor" strokeWidth="1.8" />
          <path d="M6.5 11.5a5.5 5.5 0 1 0 11 0M12 17v3.5M9 20.5h6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      );
    case "plus":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 5v14M5 12h14" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      );
    case "chevron-right":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 6l6 6-6 6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case "arrow-right":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M5 12h14M13 6l6 6-6 6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case "arrow-left":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M19 12H5M11 6l-6 6 6 6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case "play":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 7.8v8.4a.7.7 0 0 0 1.06.6l6.8-4.2a.7.7 0 0 0 0-1.2l-6.8-4.2a.7.7 0 0 0-1.06.6z" fill="currentColor" />
        </svg>
      );
    case "save":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M5 4h11l3 3v12a1.8 1.8 0 0 1-1.8 1.8H6.8A1.8 1.8 0 0 1 5 19V4z" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
          <path d="M8 4v6h8V5.2M8.5 20v-5h7v5" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      );
    case "more":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="6" r="1.5" fill="currentColor" />
          <circle cx="12" cy="12" r="1.5" fill="currentColor" />
          <circle cx="12" cy="18" r="1.5" fill="currentColor" />
        </svg>
      );
    case "file-text":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 3h7l4 4v14H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" fill="none" stroke="currentColor" strokeWidth="1.8" />
          <path d="M14 3v4h4M9 12h6M9 16h6" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      );
    case "sparkles":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 4l1.7 4.3L18 10l-4.3 1.7L12 16l-1.7-4.3L6 10l4.3-1.7L12 4zM18.5 15l.9 2.2 2.1.9-2.1.9-.9 2.2-.9-2.2-2.1-.9 2.1-.9.9-2.2zM5.5 14l.7 1.6 1.6.7-1.6.7-.7 1.6-.7-1.6-1.6-.7 1.6-.7.7-1.6z" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
        </svg>
      );
    case "brain":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M8.3 5a3.3 3.3 0 0 0-3.3 3.3c0 .8.3 1.6.8 2.2a3.8 3.8 0 0 0 1 7.4h.5a3 3 0 0 0 4.8 2.4V10.1A5.1 5.1 0 0 0 8.3 5zm7.4 0a3.3 3.3 0 0 1 3.3 3.3c0 .8-.3 1.6-.8 2.2a3.8 3.8 0 0 1-1 7.4h-.5a3 3 0 0 1-4.8 2.4V10.1A5.1 5.1 0 0 1 15.7 5z" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
        </svg>
      );
    case "target":
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="8.3" fill="none" stroke="currentColor" strokeWidth="1.6" />
          <circle cx="12" cy="12" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.6" />
          <circle cx="12" cy="12" r="1.6" fill="currentColor" />
        </svg>
      );
    default:
      return (
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M7 7l10 10M17 7L7 17" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      );
  }
}

function latestByType(revisions: ArtifactRevision[], type: ArtifactType) {
  return revisions
    .filter((item) => item.artifact_type === type)
    .sort((a, b) => b.version - a.version)[0];
}

function buildTree(folders: Folder[]) {
  const byParent = new Map<string | null, Folder[]>();
  for (const folder of folders) {
    const key = folder.parent_id;
    const current = byParent.get(key) ?? [];
    current.push(folder);
    byParent.set(key, current);
  }

  for (const list of byParent.values()) {
    list.sort((a, b) => a.name.localeCompare(b.name));
  }

  return byParent;
}

function formatDate(ts: string) {
  return new Date(ts).toLocaleString();
}

function formatShortDate(ts: string) {
  return new Date(ts).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric"
  });
}

function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new Error(message)), timeoutMs);
    promise
      .then((value) => {
        window.clearTimeout(timer);
        resolve(value);
      })
      .catch((error) => {
        window.clearTimeout(timer);
        reject(error);
      });
  });
}

export default function App() {
  const [uiLanguage, setUiLanguage] = useState<"en" | "ru">(() => {
    if (typeof window === "undefined") {
      return "en";
    }
    const saved = window.localStorage.getItem("ui_language");
    return saved === "ru" ? "ru" : "en";
  });
  const [bootstrap, setBootstrap] = useState<BootstrapState | null>(null);
  const [selectedFolderId, setSelectedFolderId] = useState<string | null>(null);
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null);
  const [entryBundle, setEntryBundle] = useState<EntryBundle | null>(null);
  const [recordingSessionId, setRecordingSessionId] = useState<string | null>(null);
  const [sources, setSources] = useState<RecordingSource[]>([]);
  const [transcriptDraft, setTranscriptDraft] = useState<string>("");
  const [recordingDevices, setRecordingDevices] = useState<RecordingDevice[]>([]);
  const [audioDeviceHints, setAudioDeviceHints] = useState<string[]>([]);
  const [artifactDrafts, setArtifactDrafts] = useState<Record<ArtifactType, string>>({
    summary: "",
    analysis: "",
    critique_recruitment: "",
    critique_sales: "",
    critique_cs: ""
  });
  const [promptDrafts, setPromptDrafts] = useState<Record<PromptRole, string>>({
    summary: "",
    analysis: "",
    critique_recruitment: "",
    critique_sales: "",
    critique_cs: ""
  });
  const [modelName, setModelName] = useState<string>("qwen3:8b");
  const [whisperModel, setWhisperModel] = useState<string>("turbo");
  const [whisperModelOptions, setWhisperModelOptions] = useState<string[]>(WHISPER_MODEL_PRESETS);
  const [showSettings, setShowSettings] = useState(false);
  const [showTrash, setShowTrash] = useState(false);
  const [critiqueType, setCritiqueType] = useState<ArtifactType>("critique_recruitment");
  const [workspaceNameDraft, setWorkspaceNameDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [transcribingAfterStop, setTranscribingAfterStop] = useState(false);
  const [recordingPaused, setRecordingPaused] = useState(false);
  const [recordingLevel, setRecordingLevel] = useState(0);
  const [recordingBytes, setRecordingBytes] = useState(0);
  const [meterBars, setMeterBars] = useState<number[]>(() => Array.from({ length: 24 }, () => 0.02));
  const [transcriptionLanguage, setTranscriptionLanguage] = useState<string>("auto");
  const [detailTab, setDetailTab] = useState<"transcript" | "summary" | "analysis" | "critique">("transcript");
  const tt = (text: string) => (uiLanguage === "ru" ? RU_TRANSLATIONS[text] ?? text : text);

  const activeEntry = useMemo(() => {
    if (!bootstrap || !selectedEntryId) {
      return null;
    }
    return bootstrap.entries.find((entry) => entry.id === selectedEntryId) ?? null;
  }, [bootstrap, selectedEntryId]);

  const entriesByFolder = useMemo(() => {
    const map = new Map<string, Entry[]>();
    const entries = (bootstrap?.entries ?? [])
      .filter((entry) => !entry.deleted_at)
      .sort((a, b) => b.created_at.localeCompare(a.created_at));
    for (const entry of entries) {
      const current = map.get(entry.folder_id) ?? [];
      current.push(entry);
      map.set(entry.folder_id, current);
    }
    return map;
  }, [bootstrap?.entries]);

  const visibleFolders = useMemo(
    () =>
      (bootstrap?.folders ?? [])
        .filter((folder) => !folder.deleted_at)
        .sort((a, b) => a.name.localeCompare(b.name)),
    [bootstrap?.folders]
  );
  const selectedFolder = useMemo(
    () => visibleFolders.find((folder) => folder.id === selectedFolderId) ?? null,
    [visibleFolders, selectedFolderId]
  );
  const selectedFolderEntries = useMemo(
    () => (selectedFolderId ? entriesByFolder.get(selectedFolderId) ?? [] : []),
    [entriesByFolder, selectedFolderId]
  );

  const trashedFolders = useMemo(
    () => bootstrap?.folders.filter((folder) => folder.deleted_at) ?? [],
    [bootstrap]
  );
  const trashedEntries = useMemo(
    () => bootstrap?.entries.filter((entry) => entry.deleted_at) ?? [],
    [bootstrap]
  );
  const canRunPostRecordingActions = useMemo(
    () => Boolean(activeEntry?.recording_path) && !recordingSessionId && !transcribingAfterStop,
    [activeEntry?.recording_path, recordingSessionId, transcribingAfterStop]
  );
  const visibleArtifactTypes = useMemo(
    () =>
      ARTIFACT_TYPES.filter((item) => {
        const latestArtifact = entryBundle ? latestByType(entryBundle.artifact_revisions, item.type) : undefined;
        return Boolean(latestArtifact || artifactDrafts[item.type].trim().length > 0);
      }),
    [artifactDrafts, entryBundle]
  );
  const whisperModelChoices = useMemo(
    () =>
      Array.from(
        new Set(
          [...WHISPER_MODEL_PRESETS, ...whisperModelOptions, whisperModel].filter(
            (value) => value.trim().length > 0
          )
        )
      ),
    [whisperModel, whisperModelOptions]
  );

  async function reloadBootstrap(keepSelection = true) {
    const data = await api.bootstrapState();
    setBootstrap(data);
    setModelName(data.model_name);
    setWhisperModel(data.whisper_model);
    try {
      const models = await api.listWhisperModels();
      const merged = Array.from(new Set([data.whisper_model, ...models]));
      setWhisperModelOptions(merged);
    } catch {
      setWhisperModelOptions((current) =>
        current.includes(data.whisper_model) ? current : [data.whisper_model, ...current]
      );
    }
    const nextPrompts = { ...promptDrafts };
    for (const template of data.prompt_templates) {
      nextPrompts[template.role] = template.prompt_text;
    }
    setPromptDrafts(nextPrompts);

    const firstFolder = data.folders.find((folder) => !folder.deleted_at);

    if (!keepSelection) {
      setSelectedFolderId(firstFolder?.id ?? null);
      setSelectedEntryId(null);
      setEntryBundle(null);
    }

    if (keepSelection) {
      if (selectedFolderId) {
        const exists = data.folders.some((folder) => folder.id === selectedFolderId && !folder.deleted_at);
        if (!exists) {
          setSelectedFolderId(firstFolder?.id ?? null);
        }
      } else {
        setSelectedFolderId(firstFolder?.id ?? null);
      }
    }

    if (keepSelection && selectedEntryId) {
      const exists = data.entries.some((entry) => entry.id === selectedEntryId && !entry.deleted_at);
      if (!exists) {
        setSelectedEntryId(null);
        setEntryBundle(null);
      }
    }
  }

  async function loadEntryBundle(entryId: string) {
    const bundle = await api.getEntryBundle(entryId);
    setEntryBundle(bundle);

    const latestTranscript = bundle.transcript_revisions.sort((a, b) => b.version - a.version)[0];
    setTranscriptDraft(latestTranscript?.text ?? "");
    setTranscriptionLanguage(
      latestTranscript?.language && latestTranscript.language.trim().length > 0
        ? latestTranscript.language
        : "auto"
    );

    const nextDrafts: Record<ArtifactType, string> = {
      summary: "",
      analysis: "",
      critique_recruitment: "",
      critique_sales: "",
      critique_cs: ""
    };

    for (const type of Object.keys(nextDrafts) as ArtifactType[]) {
      nextDrafts[type] = latestByType(bundle.artifact_revisions, type)?.text ?? "";
    }

    setArtifactDrafts(nextDrafts);
  }

  async function runTask(task: () => Promise<void>, successNotice?: string) {
    setBusy(true);
    setError(null);
    setNotice(null);
    const currentEntryId = selectedEntryId;
    try {
      await task();
      await reloadBootstrap(true);
      if (currentEntryId) {
        try {
          await loadEntryBundle(currentEntryId);
        } catch {
          setSelectedEntryId(null);
          setEntryBundle(null);
        }
      }
      if (successNotice) {
        setNotice(successNotice);
      }
    } catch (taskError) {
      const message = taskError instanceof Error ? taskError.message : String(taskError);
      setError(message);
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem("ui_language", uiLanguage);
    }
  }, [uiLanguage]);

  useEffect(() => {
    const bootstrap = async () => {
      setBusy(true);
      setError(null);
      try {
        await reloadBootstrap(false);
        await loadRecordingDevices(true);
        void api
          .prepareAiBackend()
          .then((message) => {
            if (
              message &&
              !message.toLowerCase().startsWith("ai backend ready")
            ) {
              setNotice(message);
            }
          })
          .catch((taskError) => {
            const message = taskError instanceof Error ? taskError.message : String(taskError);
            setNotice(`${tt("AI backend is not ready yet:")} ${message}`);
          });
      } catch (taskError) {
        const message = taskError instanceof Error ? taskError.message : String(taskError);
        setError(message);
      } finally {
        setBusy(false);
      }
    };
    bootstrap();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!recordingSessionId) {
      setRecordingLevel(0);
      setRecordingBytes(0);
      setMeterBars(Array.from({ length: 24 }, () => 0.02));
      return;
    }

    let cancelled = false;
    const pollMeter = async () => {
      try {
        const meter = await api.getRecordingMeter(recordingSessionId);
        if (cancelled) {
          return;
        }

        const normalizedLevel = Math.max(0, Math.min(1, meter.level));
        setRecordingLevel(normalizedLevel);
        setRecordingBytes(meter.bytes_written);
        setMeterBars((previous) => {
          const next = [...previous.slice(1)];
          const bar = normalizedLevel < 0.02
            ? 0.02
            : Math.min(1, normalizedLevel * 0.95 + 0.03);
          next.push(bar);
          return next;
        });
      } catch {
        // keep last known meter values while polling retries
      }
    };

    void pollMeter();
    const timer = window.setInterval(() => {
      void pollMeter();
    }, 450);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [recordingSessionId]);

  useEffect(() => {
    setDetailTab("transcript");
  }, [selectedEntryId]);

  const folderTree = useMemo(() => {
    if (!bootstrap) {
      return new Map<string | null, Folder[]>();
    }
    const activeFolders = bootstrap.folders.filter((folder) => !folder.deleted_at);
    return buildTree(activeFolders);
  }, [bootstrap]);

  const latestTranscript = useMemo(() => {
    if (!entryBundle) {
      return null;
    }
    return entryBundle.transcript_revisions.sort((a, b) => b.version - a.version)[0] ?? null;
  }, [entryBundle]);

  async function onSelectEntry(entryId: string) {
    const entry = bootstrap?.entries.find((item) => item.id === entryId);
    if (entry) {
      setSelectedFolderId(entry.folder_id);
    }
    setSelectedEntryId(entryId);
    setBusy(true);
    setError(null);
    try {
      await loadEntryBundle(entryId);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setBusy(false);
    }
  }

  function renderFolderNodes(parentId: string | null): JSX.Element[] {
    const nodes = folderTree.get(parentId) ?? [];
    return nodes.map((folder) => (
      <li key={folder.id}>
        <button
          className={selectedFolderId === folder.id ? "tree-node active" : "tree-node"}
          onClick={() => {
            setSelectedFolderId(folder.id);
            setSelectedEntryId(null);
            setEntryBundle(null);
          }}
        >
          <span className="node-icon">
            <Icon name="folder" />
          </span>
          <span>{folder.name}</span>
        </button>
        <ul className="tree-list">
          {(entriesByFolder.get(folder.id) ?? []).map((entry) => (
            <li key={entry.id}>
              <button
                className={selectedEntryId === entry.id ? "tree-node entry active" : "tree-node entry"}
                onClick={() => void onSelectEntry(entry.id)}
              >
                <span className="node-icon">
                  <Icon name="entry" />
                </span>
                <span>{entry.title}</span>
              </button>
            </li>
          ))}
          {renderFolderNodes(folder.id)}
        </ul>
      </li>
    ));
  }

  function defaultLabel(prefix: string) {
    const datePart = new Date().toISOString().slice(0, 16).replace("T", " ");
    return `${prefix} ${datePart}`;
  }

  function formatBytes(value: number) {
    if (value < 1024) {
      return `${value} B`;
    }
    if (value < 1024 * 1024) {
      return `${Math.round(value / 1024)} KB`;
    }
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  }

  function sourceKey(source: RecordingSource) {
    return `${source.format}::${source.input}`;
  }

  function deviceKey(device: RecordingDevice) {
    return `${device.format}::${device.input}`;
  }

  function sourceFromDevice(device: RecordingDevice): RecordingSource {
    return {
      label: device.name,
      format: device.format,
      input: device.input
    };
  }

  function defaultSourcesFromDevices(devices: RecordingDevice[]): RecordingSource[] {
    const isWindowsDeviceList = devices.some((device) => device.format === "dshow");
    if (isWindowsDeviceList) {
      const preferredLoopback = devices.find((device) => device.is_loopback);
      const preferredMicLike = devices.find((device) => !device.is_loopback) ?? devices[0];
      if (
        preferredLoopback &&
        preferredMicLike &&
        deviceKey(preferredLoopback) !== deviceKey(preferredMicLike)
      ) {
        return [sourceFromDevice(preferredLoopback), sourceFromDevice(preferredMicLike)];
      }
      if (preferredLoopback) {
        return [sourceFromDevice(preferredLoopback)];
      }
      if (preferredMicLike) {
        return [sourceFromDevice(preferredMicLike)];
      }
      return [];
    }

    const nativeSystem = devices.find(
      (device) => device.format === "screencapturekit" && device.input === "system"
    );
    const preferredMicLike =
      devices.find(
        (device) =>
          !device.is_loopback && !(device.format === "screencapturekit" && device.input === "system")
      ) ??
      devices.find((device) => !device.is_loopback) ??
      devices[0];

    if (nativeSystem && preferredMicLike && deviceKey(nativeSystem) !== deviceKey(preferredMicLike)) {
      return [sourceFromDevice(nativeSystem), sourceFromDevice(preferredMicLike)];
    }
    if (nativeSystem) {
      return [sourceFromDevice(nativeSystem)];
    }
    if (preferredMicLike) {
      return [sourceFromDevice(preferredMicLike)];
    }
    return [];
  }

  async function loadRecordingDevices(applyStartupDefaults = false) {
    const [devices, hints] = await Promise.all([
      withTimeout(
        api.listRecordingDevices(),
        10000,
        tt("Audio device detection timed out. You can still use the app and retry refresh.")
      ),
      api.listAudioDeviceHints().catch(() => [] as string[])
    ]);

    setRecordingDevices(devices);
    setAudioDeviceHints(hints.slice(0, 8));

    if (devices.length === 0) {
      setSources([]);
      return;
    }
    const startupDefaults = defaultSourcesFromDevices(devices);
    const preferredMicLike = devices.find((device) => !device.is_loopback) ?? devices[0];

    setSources((current) => {
      if (applyStartupDefaults && startupDefaults.length > 0) {
        return startupDefaults;
      }
      if (current.length === 0) {
        return startupDefaults.length > 0 ? startupDefaults : [sourceFromDevice(preferredMicLike)];
      }

      return current.map((source, index) => {
        const exact = devices.find((device) => deviceKey(device) === sourceKey(source));
        if (exact) {
          return sourceFromDevice(exact);
        }
        const fallback =
          source.label.toLowerCase().includes("mic")
            ? preferredMicLike
            : devices[Math.min(index, devices.length - 1)] ?? devices[0];
        return sourceFromDevice(fallback);
      });
    });
  }

  function artifactLabel(type: ArtifactType) {
    return tt(ARTIFACT_TYPES.find((item) => item.type === type)?.label ?? type);
  }

  function createFolderFromCurrentSelection() {
    const fallback = selectedFolderId ? defaultLabel(tt("Subfolder")) : defaultLabel(tt("Folder"));
    const name = workspaceNameDraft.trim() || fallback;
    runTask(async () => {
      await api.createFolder(name, selectedFolderId);
      setWorkspaceNameDraft("");
    }, tt("Folder created"));
  }

  function createEntryForSelectedFolder() {
    if (!selectedFolderId) {
      setError(tt("Select a folder first"));
      return;
    }
    const title = workspaceNameDraft.trim() || defaultLabel(tt("Entry"));
    runTask(async () => {
      await api.createEntry(selectedFolderId, title);
      setWorkspaceNameDraft("");
    }, tt("Entry created"));
  }

  function renameSelectedEntity() {
    const name = workspaceNameDraft.trim();
    if (!name) {
      setError(tt("Type a new name first, then press rename."));
      return;
    }
    if (selectedEntryId) {
      runTask(async () => {
        await api.renameEntry(selectedEntryId, name);
        setWorkspaceNameDraft("");
      }, tt("Entry renamed"));
      return;
    }
    if (selectedFolderId) {
      runTask(async () => {
        await api.renameFolder(selectedFolderId, name);
        setWorkspaceNameDraft("");
      }, tt("Folder renamed"));
      return;
    }
    setError(tt("Select a folder or entry first."));
  }

  function moveSelectedEntityToTrash() {
    if (selectedEntryId) {
      runTask(async () => {
        await api.moveToTrash("entry", selectedEntryId);
        setSelectedEntryId(null);
        setEntryBundle(null);
      }, tt("Entry moved to trash"));
      return;
    }
    if (selectedFolderId) {
      runTask(async () => {
        await api.moveToTrash("folder", selectedFolderId);
        setSelectedFolderId(null);
      }, tt("Folder moved to trash"));
      return;
    }
    setError(tt("Select a folder or entry first."));
  }

  const activeArtifactType: ArtifactType =
    detailTab === "summary" ? "summary" : detailTab === "analysis" ? "analysis" : critiqueType;
  const sourceControlsDisabled = busy || Boolean(recordingSessionId) || transcribingAfterStop;

  return (
    <div className="redesign-shell">
      <aside className="redesign-sidebar">
        <div className="brand-block">
          <div className="brand-mic" aria-hidden="true">
            <img className="brand-logo-image" src={beyondLogo} alt="" />
          </div>
          <div>
            <p className="brand-title">Beyond</p>
            <p className="brand-subtitle">AI SCRIBE</p>
          </div>
        </div>

        <div className="sidebar-content">
          <div className="workspace-heading">
            <span>{tt("Workspace").toUpperCase()}</span>
            <button
              className="ghost-icon"
              disabled={busy}
              title={tt("Add folder")}
              aria-label={tt("Add folder")}
              onClick={createFolderFromCurrentSelection}
            >
              <Icon name="plus" />
            </button>
          </div>

          <ul className="workspace-list">
            {visibleFolders.map((folder) => (
              <li key={folder.id}>
                <button
                  className={selectedFolderId === folder.id ? "workspace-item active" : "workspace-item"}
                  onClick={() => {
                    setSelectedFolderId(folder.id);
                    setSelectedEntryId(null);
                    setEntryBundle(null);
                  }}
                >
                  <span className="workspace-icon">
                    <Icon name="folder" />
                  </span>
                  <span>{folder.name}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>

        <div className="sidebar-footer-stack">
          <button
            className="sidebar-footer-btn"
            onClick={() => {
              setShowTrash((current) => !current);
              setShowSettings(false);
            }}
          >
            <span className="gear-symbol" aria-hidden="true">
              <Icon name="trash" />
            </span>
            <span>{tt("Trash")}</span>
          </button>
          <button
            className="sidebar-footer-btn"
            onClick={() => {
              setShowSettings((current) => !current);
              setShowTrash(false);
            }}
          >
            <span className="gear-symbol" aria-hidden="true">
              <Icon name="settings" />
            </span>
            <span>{tt("AI Prompts")}</span>
          </button>
        </div>
      </aside>

      <main className="redesign-main">
        <header className="top-chrome">
          {!activeEntry ? (
            <div className="crumb-row">
              <span>{tt("Home")}</span>
              <span className="crumb-sep" aria-hidden="true">
                <Icon name="chevron-right" />
              </span>
              <span>{selectedFolder?.name ?? tt("Workspace")}</span>
            </div>
          ) : (
            <div className="detail-head-left">
              <button
                className="ghost-icon"
                onClick={() => {
                  setSelectedEntryId(null);
                  setEntryBundle(null);
                }}
              >
                <Icon name="arrow-left" />
              </button>
              <strong>{activeEntry.title}</strong>
            </div>
          )}

          {!activeEntry ? (
            <button
              className="outline-btn record-call-btn"
              disabled={busy || !selectedFolderId}
              onClick={createEntryForSelectedFolder}
            >
              <Icon name="mic" />
              {tt("Record Call")}
            </button>
          ) : (
            <div className="top-actions">
              <button
                className="outline-btn"
                disabled={busy}
                onClick={() => {
                  if (!activeEntry) {
                    return;
                  }
                  if (detailTab === "transcript") {
                    const language = transcriptionLanguage || latestTranscript?.language || "auto";
                    runTask(
                      async () => api.updateTranscript(activeEntry.id, transcriptDraft, language),
                      tt("Transcript saved")
                    );
                    return;
                  }
                  runTask(
                    async () =>
                      api.updateArtifact(activeEntry.id, activeArtifactType, artifactDrafts[activeArtifactType]),
                    `${artifactLabel(activeArtifactType)} ${tt("saved")}`
                  );
                }}
              >
                <Icon name="save" />
                {tt("Save")}
              </button>
              <button
                className="ghost-icon"
                title={tt("Settings")}
                aria-label={tt("Settings")}
                onClick={() => {
                  setShowSettings((current) => !current);
                  setShowTrash(false);
                }}
              >
                <Icon name="more" />
              </button>
            </div>
          )}
        </header>

        {error && <p className="status error">{error}</p>}
        {notice && <p className="status success">{notice}</p>}

        {!activeEntry ? (
          <section className="workspace-view">
            <div className="workspace-page-header">
              <div>
                <h1>{selectedFolder?.name ?? tt("Workspace")}</h1>
                <p>{tt("Manage your recordings and analysis.")}</p>
              </div>
              <div className="workspace-actions">
                <button className="outline-btn" disabled={busy} onClick={createFolderFromCurrentSelection}>
                  <span className="workspace-icon">
                    <Icon name="folder" />
                  </span>
                  {tt("New Folder")}
                </button>
                <button className="solid-btn" disabled={busy || !selectedFolderId} onClick={createEntryForSelectedFolder}>
                  <Icon name="plus" />
                  {tt("New Entry")}
                </button>
              </div>
            </div>

            <p className="section-label">{tt("Recordings & Entries").toUpperCase()}</p>
            <div className="entry-list">
              {selectedFolderEntries.length === 0 ? (
                <div className="entry-card empty">{tt("Select an entry to work on recording and AI tasks.")}</div>
              ) : (
                selectedFolderEntries.map((entry) => (
                  <button key={entry.id} className="entry-card" onClick={() => void onSelectEntry(entry.id)}>
                    <span className="play-badge" aria-hidden="true">
                      <Icon name="play" />
                    </span>
                    <span className="entry-meta">
                      <strong>{entry.title}</strong>
                      <small>{formatShortDate(entry.created_at)}</small>
                    </span>
                    <span className="entry-arrow" aria-hidden="true">
                      <Icon name="arrow-right" />
                    </span>
                  </button>
                ))
              )}
            </div>
          </section>
        ) : (
          <section className="entry-view">
            <div className="recording-card">
              <button
                className="start-circle"
                disabled={busy || Boolean(recordingSessionId) || transcribingAfterStop || sources.length === 0}
                onClick={() => {
                  runTask(async () => {
                    const sessionId = await api.startRecording(activeEntry.id, sources);
                    setRecordingSessionId(sessionId);
                    setRecordingPaused(false);
                  }, tt("Recording started"));
                }}
              >
                <Icon name="mic" />
              </button>
              <div className="recording-copy">
                <h2>{tt("Start Recording")}</h2>
                <p>{tt("Record browser/app audio using screen share.")}</p>
              </div>
              <div className="recording-inline-actions">
                {!recordingSessionId && (
                  <div className="source-inline-controls">
                    <div className="source-inline-toolbar">
                      <button
                        className="ghost-icon source-mini-btn"
                        title={tt("Refresh devices")}
                        aria-label={tt("Refresh devices")}
                        disabled={sourceControlsDisabled}
                        onClick={() =>
                          runTask(async () => {
                            await loadRecordingDevices();
                          }, tt("Audio devices refreshed"))
                        }
                      >
                        <Icon name="refresh" />
                      </button>
                      <button
                        className="ghost-icon source-mini-btn"
                        title={tt("Add source")}
                        aria-label={tt("Add source")}
                        disabled={sourceControlsDisabled || recordingDevices.length === 0}
                        onClick={() => {
                          const used = new Set(sources.map((source) => sourceKey(source)));
                          const candidate =
                            recordingDevices.find(
                              (device) => device.is_loopback && !used.has(deviceKey(device))
                            ) ??
                            recordingDevices.find((device) => !used.has(deviceKey(device))) ??
                            recordingDevices[0];
                          if (!candidate) {
                            return;
                          }
                          setSources([...sources, sourceFromDevice(candidate)]);
                        }}
                      >
                        <Icon name="plus" />
                      </button>
                    </div>
                    <div className="source-inline-list">
                      {sources.map((source, index) => (
                        <div className="source-inline-row" key={`${source.format}-${source.input}-${index}`}>
                          <select
                            className="source-inline-select"
                            value={sourceKey(source)}
                            onChange={(event) => {
                              const selected = recordingDevices.find(
                                (device) => deviceKey(device) === event.target.value
                              );
                              if (!selected) {
                                return;
                              }
                              const next = [...sources];
                              next[index] = sourceFromDevice(selected);
                              setSources(next);
                            }}
                            disabled={sourceControlsDisabled || recordingDevices.length === 0}
                          >
                            {recordingDevices.length > 0 ? (
                              recordingDevices.map((device) => (
                                <option key={deviceKey(device)} value={deviceKey(device)}>
                                  {device.name}
                                </option>
                              ))
                            ) : (
                              <option value={sourceKey(source)}>{source.label}</option>
                            )}
                          </select>
                          <button
                            className="ghost-icon source-mini-btn"
                            title={tt("Remove source")}
                            aria-label={tt("Remove source")}
                            disabled={sourceControlsDisabled || sources.length <= 1}
                            onClick={() => setSources(sources.filter((_, i) => i !== index))}
                          >
                            <Icon name="remove" />
                          </button>
                        </div>
                      ))}
                    </div>
                    {audioDeviceHints.length > 0 && (
                      <div className="source-hints">
                        {audioDeviceHints.map((hint, index) => (
                          <p className="help-text" key={`hint-${index}`}>
                            {hint}
                          </p>
                        ))}
                      </div>
                    )}
                  </div>
                )}
                {recordingSessionId ? (
                  <>
                    <button
                      className="outline-btn"
                      disabled={busy || transcribingAfterStop}
                      onClick={() => {
                        if (!recordingSessionId) {
                          return;
                        }
                        runTask(
                          async () => {
                            await api.setRecordingPaused(recordingSessionId, !recordingPaused);
                            setRecordingPaused((value) => !value);
                          },
                          recordingPaused ? tt("Recording resumed") : tt("Recording paused")
                        );
                      }}
                    >
                      {recordingPaused ? tt("Resume") : tt("Pause")}
                    </button>
                    <button
                      className="solid-btn"
                      disabled={busy || transcribingAfterStop}
                      onClick={async () => {
                        if (!recordingSessionId) {
                          return;
                        }
                        const sessionId = recordingSessionId;
                        setBusy(true);
                        setError(null);
                        setNotice(null);
                        try {
                          await api.stopRecording(sessionId);
                          setRecordingSessionId(null);
                          setRecordingPaused(false);
                          await reloadBootstrap(true);
                          setNotice(tt("Recording stopped. Transcribing..."));
                        } catch (taskError) {
                          const message = taskError instanceof Error ? taskError.message : String(taskError);
                          setError(message);
                          setBusy(false);
                          return;
                        } finally {
                          setBusy(false);
                        }

                        setTranscribingAfterStop(true);
                        try {
                          await api.transcribeEntry(activeEntry.id, transcriptionLanguage);
                          await reloadBootstrap(true);
                          await loadEntryBundle(activeEntry.id);
                          setNotice(tt("Recording stopped and transcribed"));
                        } catch (taskError) {
                          const message = taskError instanceof Error ? taskError.message : String(taskError);
                          setNotice(null);
                          setError(message);
                        } finally {
                          setTranscribingAfterStop(false);
                        }
                      }}
                    >
                      {tt("Stop Recording")}
                    </button>
                  </>
                ) : (
                  <span className="idle-pill">{tt("Ready")}</span>
                )}
              </div>
            </div>

            {(recordingSessionId || transcribingAfterStop) && (
              <div className="recording-monitor">
                <p className="recording-live">
                  {recordingSessionId ? tt("Recording in progress") : tt("Transcribing latest recording")}
                </p>
                {recordingSessionId && (
                  <>
                    <div className="meter-strip" aria-label={tt("Recording signal meter")}>
                      {meterBars.map((bar, index) => (
                        <span
                          key={`bar-${index}`}
                          className="meter-bar"
                          style={{ height: `${Math.round(8 + bar * 34)}px` }}
                        />
                      ))}
                    </div>
                    <p className="help-text">
                      {tt("Signal level")}: {Math.round(recordingLevel * 100)}% | {tt("Captured")}:{" "}
                      {formatBytes(recordingBytes)}
                    </p>
                  </>
                )}
              </div>
            )}

            <div className="tab-card">
              <div className="tab-strip">
                <button
                  className={detailTab === "transcript" ? "tab-btn active" : "tab-btn"}
                  onClick={() => setDetailTab("transcript")}
                >
                  <Icon name="file-text" />
                  {tt("Transcript")}
                </button>
                <button
                  className={detailTab === "summary" ? "tab-btn active" : "tab-btn"}
                  onClick={() => setDetailTab("summary")}
                >
                  <Icon name="sparkles" />
                  {tt("Summary")}
                </button>
                <button
                  className={detailTab === "analysis" ? "tab-btn active" : "tab-btn"}
                  onClick={() => setDetailTab("analysis")}
                >
                  <Icon name="brain" />
                  {tt("Analysis")}
                </button>
                <button
                  className={detailTab === "critique" ? "tab-btn active" : "tab-btn"}
                  onClick={() => setDetailTab("critique")}
                >
                  <Icon name="target" />
                  {tt("Critique")}
                </button>
              </div>

              <div className="tab-toolbar">
                {detailTab === "transcript" ? (
                  <>
                    <p>{tt("Live transcription will appear here after recording.")}</p>
                    <div className="toolbar-actions">
                      <select
                        value={transcriptionLanguage}
                        disabled={busy || Boolean(recordingSessionId)}
                        onChange={(event) => setTranscriptionLanguage(event.target.value)}
                      >
                        {TRANSCRIPTION_LANGUAGES.map((language) => (
                          <option key={language.value} value={language.value}>
                            {tt(language.label)}
                          </option>
                        ))}
                      </select>
                      <button
                        className="outline-btn"
                        disabled={!canRunPostRecordingActions || busy}
                        onClick={() =>
                          runTask(
                            async () => api.transcribeEntry(activeEntry.id, transcriptionLanguage),
                            tt("Transcription ready")
                          )
                        }
                      >
                        <Icon name="mic" />
                        {tt("Re-Transcribe")}
                      </button>
                    </div>
                  </>
                ) : (
                  <>
                    <p>{tt("Generate artifact from latest transcript and refine manually if needed.")}</p>
                    <div className="toolbar-actions">
                      {detailTab === "critique" && (
                        <select
                          value={critiqueType}
                          onChange={(event) => setCritiqueType(event.target.value as ArtifactType)}
                          disabled={!canRunPostRecordingActions || busy}
                        >
                          {CRITIQUE_ACTIONS.map((item) => (
                            <option key={item.type} value={item.type}>
                              {tt(item.label)}
                            </option>
                          ))}
                        </select>
                      )}
                      <button
                        className="outline-btn"
                        disabled={!canRunPostRecordingActions || busy}
                        onClick={() =>
                          runTask(
                            async () => api.generateArtifact(activeEntry.id, activeArtifactType),
                            `${artifactLabel(activeArtifactType)} ${tt("completed")}`
                          )
                        }
                      >
                        {detailTab === "summary" ? (
                          <Icon name="sparkles" />
                        ) : detailTab === "analysis" ? (
                          <Icon name="brain" />
                        ) : (
                          <Icon name="target" />
                        )}
                        {tt("Generate")}
                      </button>
                    </div>
                  </>
                )}
              </div>

              <div className="tab-editor">
                {detailTab === "transcript" ? (
                  <textarea
                    className="large-text"
                    value={transcriptDraft}
                    onChange={(event) => setTranscriptDraft(event.target.value)}
                    placeholder={tt("Transcript goes here... You can paste it manually or use auto-transcription.")}
                  />
                ) : (
                  <textarea
                    className="large-text"
                    value={artifactDrafts[activeArtifactType]}
                    onChange={(event) =>
                      setArtifactDrafts({ ...artifactDrafts, [activeArtifactType]: event.target.value })
                    }
                    placeholder={`${tt("Draft")} ${artifactLabel(activeArtifactType).toLowerCase()}...`}
                  />
                )}
              </div>
            </div>
          </section>
        )}

        {showSettings && (
          <section className="settings-drawer">
            <div className="panel-heading">
              <h2>{tt("Local Model & Prompt Settings")}</h2>
              <button className="ghost-icon" onClick={() => setShowSettings(false)}>
                <Icon name="remove" />
              </button>
            </div>
            <div className="settings-section">
              <h3>{tt("Model Settings")}</h3>
              <label className="settings-field">
                <span>{tt("Interface Language")}</span>
                <select value={uiLanguage} onChange={(event) => setUiLanguage(event.target.value as "en" | "ru")}>
                  <option value="en">{tt("English")}</option>
                  <option value="ru">{tt("Russian")}</option>
                </select>
              </label>
              <label className="settings-field">
                <span>{tt("Ollama Model Name")}</span>
                <input value={modelName} onChange={(event) => setModelName(event.target.value)} />
              </label>
              <button
                className="outline-btn settings-action-btn"
                disabled={busy}
                onClick={() => runTask(async () => api.updateModelName(modelName), tt("Model name updated"))}
              >
                {tt("Save Model")}
              </button>
              <label className="settings-field">
                <span>{tt("Whisper Model")}</span>
                <select value={whisperModel} onChange={(event) => setWhisperModel(event.target.value)}>
                  {whisperModelChoices.map((model) => (
                    <option key={model} value={model}>
                      {model}
                    </option>
                  ))}
                </select>
              </label>
              <div className="settings-action-row">
                <button
                  className="outline-btn settings-action-btn"
                  disabled={busy}
                  onClick={() =>
                    runTask(async () => api.updateWhisperModel(whisperModel), tt("Whisper model updated"))
                  }
                >
                  {tt("Save Whisper Model")}
                </button>
                <button
                  className="outline-btn settings-action-btn"
                  disabled={busy}
                  onClick={() =>
                    runTask(async () => {
                      const models = await api.listWhisperModels();
                      setWhisperModelOptions(Array.from(new Set([whisperModel, ...models])));
                    }, tt("Whisper models refreshed"))
                  }
                >
                  {tt("Refresh Whisper Models")}
                </button>
              </div>
            </div>

            <div className="settings-section">
              <h3>{tt("Prompt Templates")}</h3>
              <div className="settings-block">
                <p>{tt(SUMMARY_PROMPT.label)}</p>
                <textarea
                  className="settings-textarea"
                  value={promptDrafts[SUMMARY_PROMPT.role]}
                  onChange={(event) =>
                    setPromptDrafts({ ...promptDrafts, [SUMMARY_PROMPT.role]: event.target.value })
                  }
                />
                <button
                  className="outline-btn settings-action-btn"
                  disabled={busy}
                  onClick={() =>
                    runTask(
                      async () => api.updatePrompt(SUMMARY_PROMPT.role, promptDrafts[SUMMARY_PROMPT.role]),
                      tt("Summary Prompt updated")
                    )
                  }
                >
                  {tt("Save Summary Prompt")}
                </button>
              </div>

              {CRITIQUE_ROLES.map((item) => (
                <div className="settings-block" key={item.role}>
                  <p>{tt(item.label)}</p>
                  <textarea
                    className="settings-textarea"
                    value={promptDrafts[item.role]}
                    onChange={(event) => setPromptDrafts({ ...promptDrafts, [item.role]: event.target.value })}
                  />
                  <button
                    className="outline-btn settings-action-btn"
                    disabled={busy}
                    onClick={() =>
                      runTask(
                        async () => api.updatePrompt(item.role, promptDrafts[item.role]),
                        `${tt(item.label)} ${tt("updated")}`
                      )
                    }
                  >
                    {tt("Save Prompt")}
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}

        {showTrash && (
          <section className="settings-drawer trash-drawer">
            <div className="panel-heading">
              <h2>{tt("Trash")}</h2>
              <button className="ghost-icon" onClick={() => setShowTrash(false)}>
                <Icon name="remove" />
              </button>
            </div>
            <div className="trash-grid">
              <div className="settings-block">
                <p>{tt("Folders")}</p>
                {trashedFolders.length === 0 ? (
                  <small>{tt("No deleted folders")}</small>
                ) : (
                  trashedFolders.map((folder) => (
                    <div className="trash-row" key={folder.id}>
                      <span>{folder.name}</span>
                      <div className="trash-row-actions">
                        <button
                          className="outline-btn settings-action-btn"
                          disabled={busy}
                          onClick={() => runTask(async () => api.restoreFromTrash("folder", folder.id))}
                        >
                          {tt("Restore")}
                        </button>
                        <button
                          className="outline-btn settings-action-btn danger-outline"
                          disabled={busy}
                          onClick={() => runTask(async () => api.purgeEntity("folder", folder.id))}
                        >
                          {tt("Purge")}
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
              <div className="settings-block">
                <p>{tt("Entries")}</p>
                {trashedEntries.length === 0 ? (
                  <small>{tt("No deleted entries")}</small>
                ) : (
                  trashedEntries.map((entry) => (
                    <div className="trash-row" key={entry.id}>
                      <span>{entry.title}</span>
                      <div className="trash-row-actions">
                        <button
                          className="outline-btn settings-action-btn"
                          disabled={busy}
                          onClick={() => runTask(async () => api.restoreFromTrash("entry", entry.id))}
                        >
                          {tt("Restore")}
                        </button>
                        <button
                          className="outline-btn settings-action-btn danger-outline"
                          disabled={busy}
                          onClick={() => runTask(async () => api.purgeEntity("entry", entry.id))}
                        >
                          {tt("Purge")}
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          </section>
        )}
      </main>
    </div>
  );
}
