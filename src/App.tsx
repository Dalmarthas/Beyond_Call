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

type IconName =
  | "folder-plus"
  | "entry-plus"
  | "edit"
  | "trash"
  | "settings"
  | "refresh"
  | "remove"
  | "folder"
  | "entry";

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
            setNotice(`AI backend is not ready yet: ${message}`);
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
    const devices = await withTimeout(
      api.listRecordingDevices(),
      10000,
      "Audio device detection timed out. You can still use the app and retry refresh."
    );
    setRecordingDevices(devices);
    if (devices.length === 0) {
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
    return ARTIFACT_TYPES.find((item) => item.type === type)?.label ?? type;
  }

  function createFolderFromCurrentSelection() {
    const fallback = selectedFolderId ? defaultLabel("Subfolder") : defaultLabel("Folder");
    const name = workspaceNameDraft.trim() || fallback;
    runTask(async () => {
      await api.createFolder(name, selectedFolderId);
      setWorkspaceNameDraft("");
    }, "Folder created");
  }

  function createEntryForSelectedFolder() {
    if (!selectedFolderId) {
      setError("Select a folder first");
      return;
    }
    const title = workspaceNameDraft.trim() || defaultLabel("Entry");
    runTask(async () => {
      await api.createEntry(selectedFolderId, title);
      setWorkspaceNameDraft("");
    }, "Entry created");
  }

  function renameSelectedEntity() {
    const name = workspaceNameDraft.trim();
    if (!name) {
      setError("Type a new name first, then press rename.");
      return;
    }
    if (selectedEntryId) {
      runTask(async () => {
        await api.renameEntry(selectedEntryId, name);
        setWorkspaceNameDraft("");
      }, "Entry renamed");
      return;
    }
    if (selectedFolderId) {
      runTask(async () => {
        await api.renameFolder(selectedFolderId, name);
        setWorkspaceNameDraft("");
      }, "Folder renamed");
      return;
    }
    setError("Select a folder or entry first.");
  }

  function moveSelectedEntityToTrash() {
    if (selectedEntryId) {
      runTask(async () => {
        await api.moveToTrash("entry", selectedEntryId);
        setSelectedEntryId(null);
        setEntryBundle(null);
      }, "Entry moved to trash");
      return;
    }
    if (selectedFolderId) {
      runTask(async () => {
        await api.moveToTrash("folder", selectedFolderId);
        setSelectedFolderId(null);
      }, "Folder moved to trash");
      return;
    }
    setError("Select a folder or entry first.");
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="header-title-row">
          <h1>AI Call Recorder Local</h1>
          <div className="header-icon-actions">
            <button
              className={showSettings ? "icon-only active" : "icon-only"}
              title="Settings"
              aria-label="Settings"
              onClick={() => {
                setShowSettings((current) => !current);
                setShowTrash(false);
              }}
            >
              <Icon name="settings" />
            </button>
            <button
              className={showTrash ? "icon-only active" : "icon-only"}
              title="Trash"
              aria-label="Trash"
              onClick={() => {
                setShowTrash((current) => !current);
                setShowSettings(false);
              }}
            >
              <Icon name="trash" />
            </button>
          </div>
        </div>
      </header>

      {showSettings && (
        <section className="card flyout-panel">
          <div className="panel-heading">
            <h2>Local Model & Prompt Settings</h2>
            <button
              className="icon-only"
              aria-label="Close settings"
              title="Close settings"
              onClick={() => setShowSettings(false)}
            >
              <Icon name="remove" />
            </button>
          </div>
          <label>
            Ollama Model Name
            <input value={modelName} onChange={(event) => setModelName(event.target.value)} />
          </label>
          <button
            disabled={busy}
            onClick={() => runTask(async () => api.updateModelName(modelName), "Model name updated")}
          >
            Save Model
          </button>
          <label>
            Whisper Model
            <select
              value={whisperModel}
              onChange={(event) => setWhisperModel(event.target.value)}
            >
              {whisperModelChoices.map((model) => (
                <option key={model} value={model}>
                  {model}
                </option>
              ))}
            </select>
          </label>
          <label>
            Custom Whisper Model (optional)
            <input
              value={whisperModel}
              onChange={(event) => setWhisperModel(event.target.value)}
              placeholder="turbo | large-v3 | ggml-base.bin | /path/to/model.bin"
            />
          </label>
          <div className="action-row">
            <button
              disabled={busy}
              onClick={() =>
                runTask(async () => api.updateWhisperModel(whisperModel), "Whisper model updated")
              }
            >
              Save Whisper Model
            </button>
            <button
              disabled={busy}
              onClick={() =>
                runTask(async () => {
                  const models = await api.listWhisperModels();
                  setWhisperModelOptions(Array.from(new Set([whisperModel, ...models])));
                }, "Whisper models refreshed")
              }
            >
              Refresh Whisper Models
            </button>
          </div>
          <p className="help-text">
            Use <code>turbo</code>/<code>large-v3</code> with OpenAI Whisper CLI (<code>whisper</code>), or
            use local <code>ggml-*.bin</code> models with <code>whisper-cli</code>.
          </p>

          <div className="artifact-block">
            <p>
              <strong>{SUMMARY_PROMPT.label}</strong>
            </p>
            <textarea
              className="medium-text"
              value={promptDrafts[SUMMARY_PROMPT.role]}
              onChange={(event) =>
                setPromptDrafts({ ...promptDrafts, [SUMMARY_PROMPT.role]: event.target.value })
              }
            />
            <button
              disabled={busy}
              onClick={() =>
                runTask(
                  async () => api.updatePrompt(SUMMARY_PROMPT.role, promptDrafts[SUMMARY_PROMPT.role]),
                  `${SUMMARY_PROMPT.label} updated`
                )
              }
            >
              Save {SUMMARY_PROMPT.label}
            </button>
          </div>

          {CRITIQUE_ROLES.map((item) => (
            <div key={item.role} className="artifact-block">
              <p>
                <strong>{item.label}</strong>
              </p>
              <textarea
                className="medium-text"
                value={promptDrafts[item.role]}
                onChange={(event) =>
                  setPromptDrafts({ ...promptDrafts, [item.role]: event.target.value })
                }
              />
              <button
                disabled={busy}
                onClick={() =>
                  runTask(
                    async () => api.updatePrompt(item.role, promptDrafts[item.role]),
                    `${item.label} updated`
                  )
                }
              >
                Save Prompt
              </button>
            </div>
          ))}
        </section>
      )}

      {showTrash && (
        <section className="card flyout-panel">
          <div className="panel-heading">
            <h2>Trash</h2>
            <button
              className="icon-only"
              aria-label="Close trash"
              title="Close trash"
              onClick={() => setShowTrash(false)}
            >
              <Icon name="remove" />
            </button>
          </div>
          <div className="trash-grid">
            <div>
              <h3>Folders</h3>
              {trashedFolders.map((folder) => (
                <div key={folder.id} className="trash-row">
                  <span>{folder.name}</span>
                  <button onClick={() => runTask(async () => api.restoreFromTrash("folder", folder.id))}>
                    Restore
                  </button>
                  <button onClick={() => runTask(async () => api.purgeEntity("folder", folder.id))}>
                    Purge
                  </button>
                </div>
              ))}
            </div>
            <div>
              <h3>Entries</h3>
              {trashedEntries.map((entry) => (
                <div key={entry.id} className="trash-row">
                  <span>{entry.title}</span>
                  <button onClick={() => runTask(async () => api.restoreFromTrash("entry", entry.id))}>
                    Restore
                  </button>
                  <button onClick={() => runTask(async () => api.purgeEntity("entry", entry.id))}>
                    Purge
                  </button>
                </div>
              ))}
            </div>
          </div>
        </section>
      )}

      {error && <p className="status error">{error}</p>}
      {notice && <p className="status success">{notice}</p>}

      <main className="layout-grid">
        <aside className="card workspace-panel">
          <div className="panel-heading">
            <h2>Workspace</h2>
            <div className="icon-button-group">
              <button
                className="icon-only"
                title={selectedFolderId ? "Add subfolder" : "Add folder"}
                aria-label={selectedFolderId ? "Add subfolder" : "Add folder"}
                disabled={busy}
                onClick={createFolderFromCurrentSelection}
              >
                <Icon name="folder-plus" />
              </button>
              <button
                className="icon-only"
                title="Add entry"
                aria-label="Add entry"
                disabled={busy || !selectedFolderId}
                onClick={createEntryForSelectedFolder}
              >
                <Icon name="entry-plus" />
              </button>
              <button
                className="icon-only"
                title="Rename selected"
                aria-label="Rename selected"
                disabled={busy || (!selectedFolderId && !selectedEntryId)}
                onClick={renameSelectedEntity}
              >
                <Icon name="edit" />
              </button>
              <button
                className="icon-only"
                title="Delete selected"
                aria-label="Delete selected"
                disabled={busy || (!selectedFolderId && !selectedEntryId)}
                onClick={moveSelectedEntityToTrash}
              >
                <Icon name="trash" />
              </button>
            </div>
          </div>
          <input
            value={workspaceNameDraft}
            onChange={(event) => setWorkspaceNameDraft(event.target.value)}
            placeholder={
              selectedEntryId
                ? "Entry name"
                : selectedFolderId
                  ? "Subfolder, entry, or rename"
                  : "Workspace item name"
            }
            disabled={busy}
            className="panel-name-input"
          />
          <ul className="tree-list">{renderFolderNodes(null)}</ul>
        </aside>

        <section className="card detail-panel">
          <h2>Entry Detail</h2>
          {!activeEntry && <p>Select an entry to work on recording and AI tasks.</p>}

          {activeEntry && (
            <>
              <p>
                <strong>{activeEntry.title}</strong>
              </p>
              <p
                className={
                  (recordingSessionId ? "recording" : activeEntry.status) === "recording"
                    ? "status-pill recording"
                    : "status-pill"
                }
              >
                Status: {recordingSessionId ? "recording" : activeEntry.status}
                {recordingPaused ? " (paused)" : ""}
                {transcribingAfterStop ? " (transcribing)" : ""}
              </p>
              <p>Duration: {activeEntry.duration_sec}s</p>

              <div className="source-controls">
                  <div className="source-icon-row">
                    <button
                      className="icon-only"
                      title="Refresh devices"
                      aria-label="Refresh devices"
                      disabled={busy}
                      onClick={() =>
                        runTask(async () => {
                          await loadRecordingDevices();
                        }, "Audio devices refreshed")
                      }
                    >
                      <Icon name="refresh" />
                    </button>
                    <button
                      className="icon-only"
                      title="Add source"
                      aria-label="Add source"
                      disabled={busy || recordingDevices.length === 0}
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
                      <Icon name="entry-plus" />
                    </button>
                  </div>
                  {sources.map((source, index) => (
                    <div className="source-row" key={`${source.label}-${index}`}>
                      <select
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
                        disabled={busy || recordingDevices.length === 0}
                      >
                        {recordingDevices.map((device) => (
                          <option key={deviceKey(device)} value={deviceKey(device)}>
                            {device.name}
                          </option>
                        ))}
                      </select>
                      <button
                        className="icon-only"
                        title="Remove source"
                        aria-label="Remove source"
                        disabled={busy || sources.length <= 1}
                        onClick={() => setSources(sources.filter((_, i) => i !== index))}
                      >
                        <Icon name="remove" />
                      </button>
                    </div>
                  ))}
              </div>

              <div className="record-action-row">
                <div className="recording-controls">
                  {!recordingSessionId ? (
                    <button
                      className="record-button"
                      title="Start recording"
                      aria-label="Start recording"
                      disabled={busy || transcribingAfterStop || sources.length === 0}
                      onClick={() => {
                        runTask(async () => {
                          const sessionId = await api.startRecording(activeEntry.id, sources);
                          setRecordingSessionId(sessionId);
                          setRecordingPaused(false);
                        }, "Recording started");
                      }}
                    >
                      <span className="record-dot" />
                    </button>
                  ) : (
                    <>
                      {!recordingPaused ? (
                        <button
                          disabled={busy || transcribingAfterStop}
                          onClick={() => {
                            if (!recordingSessionId) {
                              return;
                            }
                            runTask(
                              async () => {
                                await api.setRecordingPaused(recordingSessionId, true);
                                setRecordingPaused(true);
                              },
                              "Recording paused"
                            );
                          }}
                        >
                          Pause
                        </button>
                      ) : (
                        <button
                          disabled={busy || transcribingAfterStop}
                          onClick={() => {
                            if (!recordingSessionId) {
                              return;
                            }
                            runTask(
                              async () => {
                                await api.setRecordingPaused(recordingSessionId, false);
                                setRecordingPaused(false);
                              },
                              "Recording resumed"
                            );
                          }}
                        >
                          Resume
                        </button>
                      )}
                      <button
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
                            setNotice("Recording stopped. Transcribing...");
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
                            setNotice("Recording stopped and transcribed");
                          } catch (taskError) {
                            const message = taskError instanceof Error ? taskError.message : String(taskError);
                            setNotice(null);
                            setError(message);
                          } finally {
                            setTranscribingAfterStop(false);
                          }
                        }}
                      >
                        Stop Recording
                      </button>
                    </>
                  )}
                </div>

                <div className="action-row post-actions">
                  <button
                    disabled={!canRunPostRecordingActions || busy}
                    onClick={() =>
                      runTask(
                        async () => api.transcribeEntry(activeEntry.id, transcriptionLanguage),
                        "Transcription ready"
                      )
                    }
                  >
                    Transcribe
                  </button>
                  <button
                    disabled={!canRunPostRecordingActions || busy}
                    onClick={() =>
                      runTask(
                        async () => api.generateArtifact(activeEntry.id, "summary"),
                        "Summarize completed"
                      )
                    }
                  >
                    Summarize
                  </button>
                  <button
                    disabled={!canRunPostRecordingActions || busy}
                    onClick={() =>
                      runTask(
                        async () => api.generateArtifact(activeEntry.id, "analysis"),
                        "Analyze completed"
                      )
                    }
                  >
                    Analyze
                  </button>
                  <div className="inline-select-action">
                    <select
                      value={critiqueType}
                      onChange={(event) => setCritiqueType(event.target.value as ArtifactType)}
                      disabled={!canRunPostRecordingActions || busy}
                    >
                      {CRITIQUE_ACTIONS.map((item) => (
                        <option key={item.type} value={item.type}>
                          {item.label}
                        </option>
                      ))}
                    </select>
                    <button
                      disabled={!canRunPostRecordingActions || busy}
                      onClick={() =>
                        runTask(
                          async () => api.generateArtifact(activeEntry.id, critiqueType),
                          `${artifactLabel(critiqueType)} completed`
                        )
                      }
                    >
                      Critique
                    </button>
                  </div>
                  <button
                    disabled={!canRunPostRecordingActions || busy}
                    onClick={() => {
                      runTask(async () => {
                        const path = await api.exportEntry(activeEntry.id);
                        setNotice(`Export created at ${path}`);
                      });
                    }}
                  >
                    Export
                  </button>
                </div>
              </div>

              {(recordingSessionId || transcribingAfterStop) && (
                <div className="recording-monitor">
                  <p className="recording-live">
                    {recordingSessionId ? "Recording in progress" : "Transcribing latest recording"}
                  </p>
                  {recordingSessionId && (
                    <>
                      <div className="meter-strip" aria-label="Recording signal meter">
                        {meterBars.map((bar, index) => (
                          <span
                            key={`bar-${index}`}
                            className="meter-bar"
                            style={{ height: `${Math.round(8 + bar * 34)}px` }}
                          />
                        ))}
                      </div>
                      <p className="help-text">
                        Signal level: {Math.round(recordingLevel * 100)}% | Captured: {formatBytes(recordingBytes)}
                      </p>
                    </>
                  )}
                </div>
              )}

              <h3>Transcript</h3>
              <label>
                Transcription Language
                <select
                  value={transcriptionLanguage}
                  disabled={busy || Boolean(recordingSessionId)}
                  onChange={(event) => setTranscriptionLanguage(event.target.value)}
                >
                  {TRANSCRIPTION_LANGUAGES.map((language) => (
                    <option key={language.value} value={language.value}>
                      {language.label}
                    </option>
                  ))}
                </select>
              </label>
              {latestTranscript && (
                <p className="help-text">
                  Version {latestTranscript.version} | Language: {latestTranscript.language} | Updated:{" "}
                  {formatDate(latestTranscript.created_at)}
                </p>
              )}
              <textarea
                className="large-text"
                value={transcriptDraft}
                onChange={(event) => setTranscriptDraft(event.target.value)}
                placeholder="Transcript text"
              />
              <button
                disabled={busy || !activeEntry}
                onClick={() => {
                  const language = transcriptionLanguage || latestTranscript?.language || "auto";
                  runTask(
                    async () => api.updateTranscript(activeEntry.id, transcriptDraft, language),
                    "Transcript saved"
                  );
                }}
              >
                Save Transcript
              </button>

              {visibleArtifactTypes.length > 0 && (
                <>
                  <h3>Artifacts</h3>
                  {visibleArtifactTypes.map((item) => {
                    const latestArtifact = entryBundle
                      ? latestByType(entryBundle.artifact_revisions, item.type)
                      : undefined;
                    return (
                      <div key={item.type} className="artifact-block">
                        <p>
                          <strong>{item.label}</strong>
                        </p>
                        {latestArtifact && (
                          <p className="help-text">
                            v{latestArtifact.version} | transcript v{latestArtifact.source_transcript_version}
                            {latestArtifact.is_stale ? " | stale" : ""}
                          </p>
                        )}
                        <textarea
                          className="medium-text"
                          value={artifactDrafts[item.type]}
                          onChange={(event) =>
                            setArtifactDrafts({ ...artifactDrafts, [item.type]: event.target.value })
                          }
                        />
                        <button
                          disabled={busy}
                          onClick={() =>
                            runTask(
                              async () =>
                                api.updateArtifact(activeEntry.id, item.type, artifactDrafts[item.type]),
                              `${item.label} saved`
                            )
                          }
                        >
                          Save {item.label}
                        </button>
                      </div>
                    );
                  })}
                </>
              )}
            </>
          )}
        </section>
      </main>
    </div>
  );
}
