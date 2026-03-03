import { useEffect, useMemo, useState } from "react";
import { api } from "./lib/api";
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
  "Vocalize": "Vocalize",
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
  "Manage your recordings and analysis.": "Управляйте записями и аналитикой.",
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
  "Start Recording": "Начать запись",
  "Record browser/app audio using screen share.": "Записывайте аудио из браузера/приложения.",
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
  "Live transcription will appear here after recording.": "Транскрипт появится здесь после записи.",
  "Transcript goes here... You can paste it manually or use auto-transcription.": "Текст транскрипта... Вы можете вставить его вручную или использовать авто-транскрибацию.",
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
  "New Folder": "Новая папка",
  "New Entry": "Новая запись",
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
  "Export created at": "Экспорт создан в",
  "updated": "обновлено",
  "Save Summary Prompt": "Сохранить промпт для Саммари",
  "Summary Prompt updated": "Промпт для Саммари обновлен",
  "Recording signal meter": "Индикатор сигнала записи",
  "Save Transcript": "Сохранить транскрипт",
  completed: "завершено",
  saved: "сохранено",
  "turbo | large-v3 | ggml-base.bin | /path/to/model.bin":
    "turbo | large-v3 | ggml-base.bin | /путь/к/модели.bin",
  new: "новая",
  "Record Call": "Записать звонок",
  "RECORDINGS & ENTRIES": "ЗАПИСИ И ЗВОНКИ",
  "No entries yet.": "Пока нет записей.",
  "Re-Transcribe": "Перетранскрибировать",
  "Generate Summary": "Сгенерировать Саммари",
  "Generate Analysis": "Сгенерировать Анализ",
  "Generate Critique": "Сгенерировать Критику",
  "AI Prompts": "AI Промпты"
};

type IconName =
  | "folder-plus" | "entry-plus" | "edit" | "trash" | "settings"
  | "refresh" | "remove" | "folder" | "entry" | "mic" | "play"
  | "arrow-right" | "arrow-left" | "save" | "more-vertical" | "file-text"
  | "sparkles" | "activity" | "target" | "pause" | "stop";

function Icon({ name, className }: { name: IconName; className?: string }) {
  const cls = className ? ` ${className}` : "";
  switch (name) {
    case "folder-plus":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3 7h7l2 2h9v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" />
          <path d="M12 12v5M9.5 14.5h5" />
        </svg>
      );
    case "entry-plus":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M7 3h7l4 4v14H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
          <path d="M14 3v4h4" />
          <path d="M12 11v6M9 14h6" />
        </svg>
      );
    case "folder":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3 7h7l2 2h9v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" />
        </svg>
      );
    case "entry":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M7 3h7l4 4v14H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
          <path d="M14 3v4h4" />
        </svg>
      );
    case "edit":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 20l4.5-1 10-10a1.8 1.8 0 0 0 0-2.5l-1-1a1.8 1.8 0 0 0-2.5 0l-10 10L4 20z" />
          <path d="M13.5 7.5l3 3" />
        </svg>
      );
    case "trash":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 7h16M9 7V5h6v2M7 7l1 13h8l1-13" />
          <path d="M10 11v6M14 11v6" />
        </svg>
      );
    case "settings":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M10.3 3.8h3.4l.5 2.1c.4.1.8.3 1.2.5l1.9-1.1 2.4 2.4-1.1 1.9c.2.4.4.8.5 1.2l2.1.5v3.4l-2.1.5c-.1.4-.3.8-.5 1.2l1.1 1.9-2.4 2.4-1.9-1.1c-.4.2-.8.4-1.2.5l-.5 2.1h-3.4l-.5-2.1c-.4-.1-.8-.3-1.2-.5l-1.9 1.1-2.4-2.4 1.1-1.9c-.2-.4-.4-.8-.5-1.2l-2.1-.5v-3.4l2.1-.5c.1-.4.3-.8.5-1.2l-1.1-1.9 2.4-2.4 1.9 1.1c.4-.2.8-.4 1.2-.5z" />
          <circle cx="12" cy="12" r="2.8" />
        </svg>
      );
    case "refresh":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M19 8a8 8 0 1 0 2 5.3" />
          <path d="M19 3v5h-5" />
        </svg>
      );
    case "remove":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M18 6L6 18M6 6l12 12" />
        </svg>
      );
    case "mic":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
          <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
          <line x1="12" y1="19" x2="12" y2="22" />
        </svg>
      );
    case "play":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <polygon points="5 3 19 12 5 21 5 3" />
        </svg>
      );
    case "arrow-right":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <line x1="5" y1="12" x2="19" y2="12" />
          <polyline points="12 5 19 12 12 19" />
        </svg>
      );
    case "arrow-left":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <line x1="19" y1="12" x2="5" y2="12" />
          <polyline points="12 19 5 12 12 5" />
        </svg>
      );
    case "save":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
          <polyline points="17 21 17 13 7 13 7 21" />
          <polyline points="7 3 7 8 15 8" />
        </svg>
      );
    case "more-vertical":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="1" />
          <circle cx="12" cy="5" r="1" />
          <circle cx="12" cy="19" r="1" />
        </svg>
      );
    case "file-text":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
          <polyline points="14 2 14 8 20 8" />
          <line x1="16" y1="13" x2="8" y2="13" />
          <line x1="16" y1="17" x2="8" y2="17" />
          <polyline points="10 9 9 9 8 9" />
        </svg>
      );
    case "sparkles":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z" />
          <path d="M5 3v4" />
          <path d="M19 17v4" />
          <path d="M3 5h4" />
          <path d="M17 19h4" />
        </svg>
      );
    case "activity":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
        </svg>
      );
    case "target":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" />
          <circle cx="12" cy="12" r="6" />
          <circle cx="12" cy="12" r="2" />
        </svg>
      );
    case "pause":
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <rect x="6" y="4" width="4" height="16" />
          <rect x="14" y="4" width="4" height="16" />
        </svg>
      );
    default:
      return (
        <svg viewBox="0 0 24 24" className={cls} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" />
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
  return new Date(ts).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" });
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
    if (typeof window === "undefined") return "en";
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
  const [activeTab, setActiveTab] = useState<"transcript" | "summary" | "analysis" | "critique">("transcript");
  
  const tt = (text: string) => (uiLanguage === "ru" ? RU_TRANSLATIONS[text] ?? text : text);

  const activeEntry = useMemo(() => {
    if (!bootstrap || !selectedEntryId) return null;
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

  const activeFolder = useMemo(() => {
    return bootstrap?.folders.find(f => f.id === selectedFolderId) ?? null;
  }, [bootstrap, selectedFolderId]);

  const trashedFolders = useMemo(() => bootstrap?.folders.filter((folder) => folder.deleted_at) ?? [], [bootstrap]);
  const trashedEntries = useMemo(() => bootstrap?.entries.filter((entry) => entry.deleted_at) ?? [], [bootstrap]);
  
  const canRunPostRecordingActions = useMemo(
    () => Boolean(activeEntry?.recording_path) && !recordingSessionId && !transcribingAfterStop,
    [activeEntry?.recording_path, recordingSessionId, transcribingAfterStop]
  );

  const whisperModelChoices = useMemo(
    () => Array.from(new Set([...WHISPER_MODEL_PRESETS, ...whisperModelOptions, whisperModel].filter((value) => value.trim().length > 0))),
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
      setWhisperModelOptions((current) => current.includes(data.whisper_model) ? current : [data.whisper_model, ...current]);
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
      summary: "", analysis: "", critique_recruitment: "", critique_sales: "", critique_cs: ""
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
        void api.prepareAiBackend()
          .then((message) => {
            if (message && !message.toLowerCase().startsWith("ai backend ready")) {
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
        if (cancelled) return;

        const normalizedLevel = Math.max(0, Math.min(1, meter.level));
        setRecordingLevel(normalizedLevel);
        setRecordingBytes(meter.bytes_written);
        setMeterBars((previous) => {
          const next = [...previous.slice(1)];
          const bar = normalizedLevel < 0.02 ? 0.02 : Math.min(1, normalizedLevel * 0.95 + 0.03);
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

  const folderTree = useMemo(() => {
    if (!bootstrap) return new Map<string | null, Folder[]>();
    const activeFolders = bootstrap.folders.filter((folder) => !folder.deleted_at);
    return buildTree(activeFolders);
  }, [bootstrap]);

  const latestTranscript = useMemo(() => {
    if (!entryBundle) return null;
    return entryBundle.transcript_revisions.sort((a, b) => b.version - a.version)[0] ?? null;
  }, [entryBundle]);

  async function onSelectEntry(entryId: string) {
    const entry = bootstrap?.entries.find((item) => item.id === entryId);
    if (entry) setSelectedFolderId(entry.folder_id);
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
          className={`tree-node ${selectedFolderId === folder.id && !selectedEntryId ? "active" : ""}`}
          onClick={() => {
            setSelectedFolderId(folder.id);
            setSelectedEntryId(null);
            setEntryBundle(null);
          }}
        >
          <span className="node-icon"><Icon name="folder" /></span>
          <span>{folder.name}</span>
        </button>
        <ul className="tree-list">
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
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${Math.round(value / 1024)} KB`;
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  }

  function sourceKey(source: RecordingSource) {
    return `${source.format}::${source.input}`;
  }

  function deviceKey(device: RecordingDevice) {
    return `${device.format}::${device.input}`;
  }

  function sourceFromDevice(device: RecordingDevice): RecordingSource {
    return { label: device.name, format: device.format, input: device.input };
  }

  function defaultSourcesFromDevices(devices: RecordingDevice[]): RecordingSource[] {
    const nativeSystem = devices.find((device) => device.format === "screencapturekit" && device.input === "system");
    const preferredMicLike =
      devices.find((device) => !device.is_loopback && !(device.format === "screencapturekit" && device.input === "system")) ??
      devices.find((device) => !device.is_loopback) ??
      devices[0];

    if (nativeSystem && preferredMicLike && deviceKey(nativeSystem) !== deviceKey(preferredMicLike)) {
      return [sourceFromDevice(nativeSystem), sourceFromDevice(preferredMicLike)];
    }
    if (nativeSystem) return [sourceFromDevice(nativeSystem)];
    if (preferredMicLike) return [sourceFromDevice(preferredMicLike)];
    return [];
  }

  async function loadRecordingDevices(applyStartupDefaults = false) {
    const devices = await withTimeout(
      api.listRecordingDevices(),
      10000,
      tt("Audio device detection timed out. You can still use the app and retry refresh.")
    );
    setRecordingDevices(devices);
    if (devices.length === 0) return;
    const startupDefaults = defaultSourcesFromDevices(devices);
    const preferredMicLike = devices.find((device) => !device.is_loopback) ?? devices[0];

    setSources((current) => {
      if (applyStartupDefaults && startupDefaults.length > 0) return startupDefaults;
      if (current.length === 0) return startupDefaults.length > 0 ? startupDefaults : [sourceFromDevice(preferredMicLike)];
      return current.map((source, index) => {
        const exact = devices.find((device) => deviceKey(device) === sourceKey(source));
        if (exact) return sourceFromDevice(exact);
        const fallback = source.label.toLowerCase().includes("mic") ? preferredMicLike : devices[Math.min(index, devices.length - 1)] ?? devices[0];
        return sourceFromDevice(fallback);
      });
    });
  }

  function createFolderFromCurrentSelection() {
    const name = workspaceNameDraft.trim() || defaultLabel(tt("Folder"));
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
      runTask(async () => { await api.renameEntry(selectedEntryId, name); setWorkspaceNameDraft(""); }, tt("Entry renamed"));
      return;
    }
    if (selectedFolderId) {
      runTask(async () => { await api.renameFolder(selectedFolderId, name); setWorkspaceNameDraft(""); }, tt("Folder renamed"));
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

  function handleSaveCurrentTab() {
    if (!activeEntry) return;
    if (activeTab === 'transcript') {
      const language = transcriptionLanguage || latestTranscript?.language || "auto";
      runTask(async () => api.updateTranscript(activeEntry.id, transcriptDraft, language), tt("Transcript saved"));
    } else if (activeTab === 'summary') {
      runTask(async () => api.updateArtifact(activeEntry.id, "summary", artifactDrafts.summary), tt("Summarize completed"));
    } else if (activeTab === 'analysis') {
      runTask(async () => api.updateArtifact(activeEntry.id, "analysis", artifactDrafts.analysis), tt("Analyze completed"));
    } else if (activeTab === 'critique') {
      runTask(async () => api.updateArtifact(activeEntry.id, critiqueType, artifactDrafts[critiqueType]), `${tt(ARTIFACT_TYPES.find(a => a.type === critiqueType)?.label ?? critiqueType)} ${tt("saved")}`);
    }
  }

  const folderEntries = selectedFolderId ? (entriesByFolder.get(selectedFolderId) ?? []) : [];

  return (
    <div className="vocalize-app">
      {/* Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <div className="logo-icon"><Icon name="mic" /></div>
          <div className="logo-text">
             <div className="brand-name">{tt("Vocalize")}</div>
             <div className="brand-sub">LOCAL INTELLIGENCE</div>
          </div>
        </div>
        
        <div className="sidebar-section">
          <div className="sidebar-section-header">
            <span>WORKSPACES</span>
            <button className="icon-btn-small" onClick={() => {
               if (workspaceNameDraft.trim()) {
                 createFolderFromCurrentSelection();
               } else {
                 setWorkspaceNameDraft(defaultLabel(tt("Folder")));
               }
            }}>
               <Icon name="folder-plus" />
            </button>
          </div>
          <div style={{padding: '0 8px', marginBottom: '12px'}}>
              <input 
                 className="global-input" 
                 placeholder={tt("Workspace item name")}
                 value={workspaceNameDraft}
                 onChange={e => setWorkspaceNameDraft(e.target.value)}
              />
          </div>
          <ul className="workspace-list">
            {renderFolderNodes(null)}
          </ul>
        </div>

        <div className="sidebar-footer">
           <button className="sidebar-footer-btn" onClick={() => { setShowSettings(true); setShowTrash(false); }}>
              <Icon name="settings" /> {tt("AI Prompts")}
           </button>
           <button className="sidebar-footer-btn" onClick={() => { setShowTrash(true); setShowSettings(false); }}>
              <Icon name="trash" /> {tt("Trash")}
           </button>
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="main-content">
        {(error || notice) && (
          <div className="status-messages">
            {error && <div className="status error">{error}</div>}
            {notice && <div className="status success">{notice}</div>}
          </div>
        )}

        {/* View: Folder Details */}
        {!selectedEntryId && activeFolder && (
           <>
              <div className="topbar">
                 <div className="breadcrumbs">
                    Home <Icon name="arrow-right" className="text-muted" /> <span className="crumb-active">{activeFolder.name}</span>
                 </div>
                 <div className="topbar-actions">
                    <button className="outline-btn" onClick={createEntryForSelectedFolder} disabled={busy}>
                       <Icon name="mic" className="text-red"/> {tt("Record Call")}
                    </button>
                 </div>
              </div>
              <div className="view-container">
                 <div className="view-header">
                    <div className="view-title">
                       <h2>{activeFolder.name}</h2>
                       <p>{tt("Manage your recordings and analysis.")}</p>
                    </div>
                    <div className="view-actions">
                       <button className="outline-btn" onClick={renameSelectedEntity} disabled={busy || !workspaceNameDraft.trim()}>
                          <Icon name="edit" />
                       </button>
                       <button className="outline-btn" onClick={createFolderFromCurrentSelection} disabled={busy}>
                          <Icon name="folder" /> {tt("New Folder")}
                       </button>
                       <button className="solid-btn" onClick={createEntryForSelectedFolder} disabled={busy}>
                          <Icon name="entry-plus" /> {tt("New Entry")}
                       </button>
                    </div>
                 </div>
                 <div className="folder-content">
                    <div className="section-label">{tt("RECORDINGS & ENTRIES")}</div>
                    <div className="entries-list">
                       {folderEntries.length === 0 && <p className="help-text">{tt("No entries yet.")}</p>}
                       {folderEntries.map(entry => (
                          <div key={entry.id} className="entry-card" onClick={() => onSelectEntry(entry.id)}>
                             <div className="entry-card-left">
                                <button className="play-btn-circle"><Icon name="play" /></button>
                                <div className="entry-info">
                                   <span className="entry-title">{entry.title}</span>
                                   <span className="entry-date">{formatDate(entry.created_at)}</span>
                                </div>
                             </div>
                             <Icon name="arrow-right" className="text-muted" />
                          </div>
                       ))}
                    </div>
                 </div>
              </div>
           </>
        )}

        {/* View: Entry Details */}
        {selectedEntryId && activeEntry && (
           <>
              <div className="topbar">
                 <div className="breadcrumbs" onClick={() => { setSelectedEntryId(null); setEntryBundle(null); }} style={{cursor: 'pointer'}}>
                    <Icon name="arrow-left" /> <span className="crumb-active">{activeEntry.title}</span>
                 </div>
                 <div className="topbar-actions">
                    <button className="outline-btn" onClick={handleSaveCurrentTab} disabled={busy}>
                       <Icon name="save" /> {tt("Save")}
                    </button>
                    <button className="icon-btn" onClick={moveSelectedEntityToTrash} disabled={busy} title={tt("Delete selected")}>
                       <Icon name="trash" />
                    </button>
                 </div>
              </div>
              <div className="view-container">
                 <div className="recording-card">
                    <div className="recording-card-main">
                       {!recordingSessionId ? (
                         <button
                           className="big-record-btn"
                           title={tt("Start Recording")}
                           disabled={busy || transcribingAfterStop || sources.length === 0}
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
                       ) : (
                         <div className="recording-active-controls">
                            {!recordingPaused ? (
                               <button className="pause-btn" disabled={busy || transcribingAfterStop} onClick={() => {
                                  runTask(async () => { await api.setRecordingPaused(recordingSessionId, true); setRecordingPaused(true); }, tt("Recording paused"));
                               }}>
                                  <Icon name="pause" />
                               </button>
                            ) : (
                               <button className="pause-btn" disabled={busy || transcribingAfterStop} onClick={() => {
                                  runTask(async () => { await api.setRecordingPaused(recordingSessionId, false); setRecordingPaused(false); }, tt("Recording resumed"));
                               }}>
                                  <Icon name="play" />
                               </button>
                            )}
                            <button className="stop-btn" disabled={busy || transcribingAfterStop} onClick={async () => {
                               const sessionId = recordingSessionId;
                               setBusy(true); setError(null); setNotice(null);
                               try {
                                  await api.stopRecording(sessionId);
                                  setRecordingSessionId(null); setRecordingPaused(false);
                                  await reloadBootstrap(true);
                                  setNotice(tt("Recording stopped. Transcribing..."));
                               } catch (taskError) {
                                  setError(taskError instanceof Error ? taskError.message : String(taskError)); setBusy(false); return;
                               }
                               setTranscribingAfterStop(true);
                               try {
                                  await api.transcribeEntry(activeEntry.id, transcriptionLanguage);
                                  await reloadBootstrap(true); await loadEntryBundle(activeEntry.id);
                                  setNotice(tt("Recording stopped and transcribed"));
                               } catch (taskError) {
                                  setNotice(null); setError(taskError instanceof Error ? taskError.message : String(taskError));
                               } finally { setTranscribingAfterStop(false); }
                            }}>
                               <span className="stop-square" />
                            </button>
                         </div>
                       )}

                       <div className="recording-info">
                         <h3>
                           {recordingSessionId 
                             ? (recordingPaused ? tt("Recording paused") : tt("Recording in progress")) 
                             : transcribingAfterStop ? tt("Transcribing latest recording") : tt("Start Recording")}
                         </h3>
                         <p>
                           {recordingSessionId || transcribingAfterStop 
                             ? `${tt("Duration")}: ${activeEntry.duration_sec}s | ${tt("Captured")}: ${formatBytes(recordingBytes)}` 
                             : tt("Record browser/app audio using screen share.")}
                         </p>
                       </div>
                    </div>

                    {recordingSessionId && (
                      <div className="meter-strip" aria-label={tt("Recording signal meter")}>
                        {meterBars.map((bar, index) => (
                          <span key={`bar-${index}`} className="meter-bar" style={{ height: `${Math.round(8 + bar * 34)}px` }} />
                        ))}
                      </div>
                    )}

                    {!recordingSessionId && !transcribingAfterStop && (
                      <div className="source-controls-compact">
                         <div style={{display: 'flex', gap: '8px', alignItems: 'center'}}>
                            <button className="icon-btn-small" onClick={() => runTask(async () => { await loadRecordingDevices(); }, tt("Audio devices refreshed"))}><Icon name="refresh" /></button>
                            <button className="icon-btn-small" onClick={() => {
                               const used = new Set(sources.map((source) => sourceKey(source)));
                               const candidate = recordingDevices.find((device) => device.is_loopback && !used.has(deviceKey(device))) ?? recordingDevices.find((device) => !used.has(deviceKey(device))) ?? recordingDevices[0];
                               if (candidate) setSources([...sources, sourceFromDevice(candidate)]);
                            }}><Icon name="entry-plus" /></button>
                         </div>
                         {sources.map((source, index) => (
                            <div className="source-row-compact" key={`${source.label}-${index}`}>
                               <select value={sourceKey(source)} disabled={busy || recordingDevices.length === 0} onChange={(event) => {
                                  const selected = recordingDevices.find((device) => deviceKey(device) === event.target.value);
                                  if (selected) { const next = [...sources]; next[index] = sourceFromDevice(selected); setSources(next); }
                               }}>
                                  {recordingDevices.map((device) => (
                                     <option key={deviceKey(device)} value={deviceKey(device)}>{device.name}</option>
                                  ))}
                               </select>
                               <button className="icon-btn-small" disabled={busy || sources.length <= 1} onClick={() => setSources(sources.filter((_, i) => i !== index))}><Icon name="remove" /></button>
                            </div>
                         ))}
                      </div>
                    )}
                 </div>

                 <div className="tabs-container">
                    <div className="tabs-header">
                       <button className={`tab ${activeTab === 'transcript' ? 'active' : ''}`} onClick={() => setActiveTab('transcript')}><Icon name="file-text" /> {tt("Transcript")}</button>
                       <button className={`tab ${activeTab === 'summary' ? 'active' : ''}`} onClick={() => setActiveTab('summary')}><Icon name="sparkles" /> {tt("Summary")}</button>
                       <button className={`tab ${activeTab === 'analysis' ? 'active' : ''}`} onClick={() => setActiveTab('analysis')}><Icon name="activity" /> {tt("Analysis")}</button>
                       <button className={`tab ${activeTab === 'critique' ? 'active' : ''}`} onClick={() => setActiveTab('critique')}><Icon name="target" /> {tt("Critique")}</button>
                    </div>
                    
                    <div className="tab-content">
                       {activeTab === 'transcript' && (
                          <div className="tab-pane">
                             <div className="tab-pane-header">
                                <span>
                                   {latestTranscript 
                                      ? `v${latestTranscript.version} | ${latestTranscript.language} | ${formatDate(latestTranscript.created_at)}` 
                                      : tt("Live transcription will appear here after recording.")}
                                </span>
                                <div style={{display:'flex', gap: '8px', alignItems:'center'}}>
                                   <select className="global-input" style={{margin:0, width: 'auto'}} value={transcriptionLanguage} disabled={busy || Boolean(recordingSessionId)} onChange={(e) => setTranscriptionLanguage(e.target.value)}>
                                      {TRANSCRIPTION_LANGUAGES.map(l => <option key={l.value} value={l.value}>{tt(l.label)}</option>)}
                                   </select>
                                   <button className="outline-btn" onClick={() => runTask(async () => api.transcribeEntry(activeEntry.id, transcriptionLanguage))} disabled={!canRunPostRecordingActions || busy}>
                                      <Icon name="mic" /> {tt("Re-Transcribe")}
                                   </button>
                                </div>
                             </div>
                             <textarea 
                                className="content-textarea" 
                                placeholder={tt("Transcript goes here... You can paste it manually or use auto-transcription.")}
                                value={transcriptDraft}
                                onChange={e => setTranscriptDraft(e.target.value)}
                             />
                          </div>
                       )}

                       {activeTab === 'summary' && (
                          <div className="tab-pane">
                             <div className="tab-pane-header">
                                <span>{latestByType(entryBundle?.artifact_revisions ?? [], 'summary')?.is_stale ? tt("stale") : ""}</span>
                                <button className="outline-btn" onClick={() => runTask(async () => api.generateArtifact(activeEntry.id, "summary"), tt("Summarize completed"))} disabled={!canRunPostRecordingActions || busy}>
                                   <Icon name="sparkles" /> {tt("Generate Summary")}
                                </button>
                             </div>
                             <textarea 
                                className="content-textarea" 
                                value={artifactDrafts.summary}
                                onChange={e => setArtifactDrafts({...artifactDrafts, summary: e.target.value})}
                             />
                          </div>
                       )}

                       {activeTab === 'analysis' && (
                          <div className="tab-pane">
                             <div className="tab-pane-header">
                                <span>{latestByType(entryBundle?.artifact_revisions ?? [], 'analysis')?.is_stale ? tt("stale") : ""}</span>
                                <button className="outline-btn" onClick={() => runTask(async () => api.generateArtifact(activeEntry.id, "analysis"), tt("Analyze completed"))} disabled={!canRunPostRecordingActions || busy}>
                                   <Icon name="activity" /> {tt("Generate Analysis")}
                                </button>
                             </div>
                             <textarea 
                                className="content-textarea" 
                                value={artifactDrafts.analysis}
                                onChange={e => setArtifactDrafts({...artifactDrafts, analysis: e.target.value})}
                             />
                          </div>
                       )}

                       {activeTab === 'critique' && (
                          <div className="tab-pane">
                             <div className="tab-pane-header">
                                <span>{latestByType(entryBundle?.artifact_revisions ?? [], critiqueType)?.is_stale ? tt("stale") : ""}</span>
                                <div style={{display:'flex', gap:'8px'}}>
                                   <select className="global-input" style={{margin:0, width: 'auto'}} value={critiqueType} onChange={(e) => setCritiqueType(e.target.value as ArtifactType)} disabled={busy}>
                                      {CRITIQUE_ACTIONS.map((item) => <option key={item.type} value={item.type}>{tt(item.label)}</option>)}
                                   </select>
                                   <button className="outline-btn" onClick={() => runTask(async () => api.generateArtifact(activeEntry.id, critiqueType), `${tt("Critique")} ${tt("completed")}`)} disabled={!canRunPostRecordingActions || busy}>
                                      <Icon name="target" /> {tt("Generate Critique")}
                                   </button>
                                </div>
                             </div>
                             <textarea 
                                className="content-textarea" 
                                value={artifactDrafts[critiqueType]}
                                onChange={e => setArtifactDrafts({...artifactDrafts, [critiqueType]: e.target.value})}
                             />
                          </div>
                       )}
                    </div>
                 </div>
              </div>
           </>
        )}
      </main>

      {/* Modals */}
      {showSettings && (
        <div className="overlay" onClick={() => setShowSettings(false)}>
           <div className="modal" onClick={e => e.stopPropagation()}>
              <div className="modal-header">
                 <h2>{tt("Local Model & Prompt Settings")}</h2>
                 <button className="icon-btn" onClick={() => setShowSettings(false)}><Icon name="remove" /></button>
              </div>
              <div className="form-group">
                 <label>{tt("Interface Language")}</label>
                 <select value={uiLanguage} onChange={(e) => setUiLanguage(e.target.value as "en" | "ru")}>
                    <option value="en">{tt("English")}</option>
                    <option value="ru">{tt("Russian")}</option>
                 </select>
              </div>
              <div className="form-group">
                 <label>{tt("Ollama Model Name")}</label>
                 <input value={modelName} onChange={(e) => setModelName(e.target.value)} />
                 <button className="outline-btn" style={{marginTop: '8px'}} onClick={() => runTask(async () => api.updateModelName(modelName), tt("Model name updated"))}>{tt("Save Model")}</button>
              </div>
              <div className="form-group">
                 <label>{tt("Whisper Model")}</label>
                 <select value={whisperModel} onChange={(e) => setWhisperModel(e.target.value)}>
                    {whisperModelChoices.map((model) => <option key={model} value={model}>{model}</option>)}
                 </select>
              </div>
              <div className="form-group">
                 <label>{tt("Custom Whisper Model (optional)")}</label>
                 <input value={whisperModel} onChange={(e) => setWhisperModel(e.target.value)} placeholder={tt("turbo | large-v3 | ggml-base.bin | /path/to/model.bin")} />
              </div>
              <div style={{display:'flex', gap:'8px', marginBottom: '16px'}}>
                 <button className="outline-btn" onClick={() => runTask(async () => api.updateWhisperModel(whisperModel), tt("Whisper model updated"))}>{tt("Save Whisper Model")}</button>
                 <button className="outline-btn" onClick={() => runTask(async () => { const models = await api.listWhisperModels(); setWhisperModelOptions(Array.from(new Set([whisperModel, ...models]))); }, tt("Whisper models refreshed"))}>{tt("Refresh Whisper Models")}</button>
              </div>
              
              <div className="form-group">
                 <label>{tt(SUMMARY_PROMPT.label)}</label>
                 <textarea style={{minHeight:'100px'}} value={promptDrafts[SUMMARY_PROMPT.role]} onChange={(e) => setPromptDrafts({ ...promptDrafts, [SUMMARY_PROMPT.role]: e.target.value })} />
                 <button className="outline-btn" style={{marginTop: '8px'}} onClick={() => runTask(async () => api.updatePrompt(SUMMARY_PROMPT.role, promptDrafts[SUMMARY_PROMPT.role]), `${tt(SUMMARY_PROMPT.label)} ${tt("updated")}`)}>{tt("Save")} {tt(SUMMARY_PROMPT.label)}</button>
              </div>

              {CRITIQUE_ROLES.map((item) => (
                 <div key={item.role} className="form-group">
                    <label>{tt(item.label)}</label>
                    <textarea style={{minHeight:'100px'}} value={promptDrafts[item.role]} onChange={(e) => setPromptDrafts({ ...promptDrafts, [item.role]: e.target.value })} />
                    <button className="outline-btn" style={{marginTop: '8px'}} onClick={() => runTask(async () => api.updatePrompt(item.role, promptDrafts[item.role]), `${tt(item.label)} ${tt("updated")}`)}>{tt("Save Prompt")}</button>
                 </div>
              ))}
           </div>
        </div>
      )}

      {showTrash && (
        <div className="overlay" onClick={() => setShowTrash(false)}>
           <div className="modal" onClick={e => e.stopPropagation()}>
              <div className="modal-header">
                 <h2>{tt("Trash")}</h2>
                 <button className="icon-btn" onClick={() => setShowTrash(false)}><Icon name="remove" /></button>
              </div>
              <div className="trash-grid">
                 <div>
                    <h3 style={{fontSize:'14px', marginBottom:'12px'}}>{tt("Folders")}</h3>
                    {trashedFolders.map((folder) => (
                       <div key={folder.id} className="trash-row">
                          <span style={{fontSize:'13px'}}>{folder.name}</span>
                          <div style={{display:'flex', gap:'4px'}}>
                             <button className="icon-btn-small" title={tt("Restore")} onClick={() => runTask(async () => api.restoreFromTrash("folder", folder.id))}><Icon name="refresh" /></button>
                             <button className="icon-btn-small" style={{color:'var(--danger)'}} title={tt("Purge")} onClick={() => runTask(async () => api.purgeEntity("folder", folder.id))}><Icon name="remove" /></button>
                          </div>
                       </div>
                    ))}
                 </div>
                 <div>
                    <h3 style={{fontSize:'14px', marginBottom:'12px'}}>{tt("Entries")}</h3>
                    {trashedEntries.map((entry) => (
                       <div key={entry.id} className="trash-row">
                          <span style={{fontSize:'13px'}}>{entry.title}</span>
                          <div style={{display:'flex', gap:'4px'}}>
                             <button className="icon-btn-small" title={tt("Restore")} onClick={() => runTask(async () => api.restoreFromTrash("entry", entry.id))}><Icon name="refresh" /></button>
                             <button className="icon-btn-small" style={{color:'var(--danger)'}} title={tt("Purge")} onClick={() => runTask(async () => api.purgeEntity("entry", entry.id))}><Icon name="remove" /></button>
                          </div>
                       </div>
                    ))}
                 </div>
              </div>
           </div>
        </div>
      )}
    </div>
  );
}
