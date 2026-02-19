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

export default function App() {
  const [bootstrap, setBootstrap] = useState<BootstrapState | null>(null);
  const [selectedFolderId, setSelectedFolderId] = useState<string | null>(null);
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null);
  const [entryBundle, setEntryBundle] = useState<EntryBundle | null>(null);
  const [recordingSessionId, setRecordingSessionId] = useState<string | null>(null);
  const [sources, setSources] = useState<RecordingSource[]>([
    { label: "Microphone", format: "avfoundation", input: ":0" }
  ]);
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
  const [newRootFolderName, setNewRootFolderName] = useState("");
  const [newSubfolderName, setNewSubfolderName] = useState("");
  const [newEntryTitle, setNewEntryTitle] = useState("");
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

  const activeEntries = useMemo(() => {
    if (!bootstrap || !selectedFolderId) {
      return [];
    }
    return bootstrap.entries
      .filter((entry) => entry.folder_id === selectedFolderId && !entry.deleted_at)
      .sort((a, b) => b.created_at.localeCompare(a.created_at));
  }, [bootstrap, selectedFolderId]);

  const trashedFolders = useMemo(
    () => bootstrap?.folders.filter((folder) => folder.deleted_at) ?? [],
    [bootstrap]
  );
  const trashedEntries = useMemo(
    () => bootstrap?.entries.filter((entry) => entry.deleted_at) ?? [],
    [bootstrap]
  );
  const hasLoopbackDevice = useMemo(
    () => recordingDevices.some((device) => device.is_loopback),
    [recordingDevices]
  );
  const selectedLoopbackOnly = useMemo(() => {
    if (sources.length === 0) {
      return false;
    }
    return sources.every((source) => {
      const device = recordingDevices.find((item) => deviceKey(item) === sourceKey(source));
      return Boolean(device?.is_loopback);
    });
  }, [recordingDevices, sources]);
  const selectedNativeSystemSource = useMemo(
    () => sources.some((source) => source.format === "screencapturekit"),
    [sources]
  );

  async function reloadBootstrap(keepSelection = true) {
    const data = await api.bootstrapState();
    setBootstrap(data);
    setModelName(data.model_name);
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
        await loadRecordingDevices();
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
          {folder.name}
        </button>
        <ul className="tree-list">{renderFolderNodes(folder.id)}</ul>
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

  async function loadRecordingDevices() {
    const devices = await api.listRecordingDevices();
    setRecordingDevices(devices);
    if (devices.length === 0) {
      return;
    }
    const preferredMicLike = devices.find((device) => !device.is_loopback) ?? devices[0];

    setSources((current) => {
      if (current.length === 0) {
        return [sourceFromDevice(preferredMicLike)];
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

  return (
    <div className="app-shell">
      <header className="app-header">
        <h1>AI Call Recorder Local</h1>
        <div className="header-actions">
          <div className="header-action-group">
            <input
              value={newRootFolderName}
              onChange={(event) => setNewRootFolderName(event.target.value)}
              placeholder="Root folder name (optional)"
              disabled={busy}
            />
            <button
              onClick={() => {
                const name = newRootFolderName.trim() || defaultLabel("Root Folder");
                runTask(async () => {
                  await api.createFolder(name, null);
                  setNewRootFolderName("");
                }, "Root folder created");
              }}
              disabled={busy}
            >
              + Root Folder
            </button>
          </div>
          <div className="header-action-group">
            <input
              value={newSubfolderName}
              onChange={(event) => setNewSubfolderName(event.target.value)}
              placeholder="Subfolder name (optional)"
              disabled={busy || !selectedFolderId}
            />
            <button
              onClick={() => {
                if (!selectedFolderId) {
                  setError("Select a folder first");
                  return;
                }
                const name = newSubfolderName.trim() || defaultLabel("Subfolder");
                runTask(async () => {
                  await api.createFolder(name, selectedFolderId);
                  setNewSubfolderName("");
                }, "Subfolder created");
              }}
              disabled={busy || !selectedFolderId}
            >
              + Subfolder
            </button>
          </div>
          <div className="header-action-group">
            <input
              value={newEntryTitle}
              onChange={(event) => setNewEntryTitle(event.target.value)}
              placeholder="Entry title (optional)"
              disabled={busy || !selectedFolderId}
            />
            <button
              onClick={() => {
                if (!selectedFolderId) {
                  setError("Select a folder first");
                  return;
                }
                const title = newEntryTitle.trim() || defaultLabel("Entry");
                runTask(async () => {
                  await api.createEntry(selectedFolderId, title);
                  setNewEntryTitle("");
                }, "Entry created");
              }}
              disabled={busy || !selectedFolderId}
            >
              + Entry
            </button>
          </div>
        </div>
      </header>

      {error && <p className="status error">{error}</p>}
      {notice && <p className="status success">{notice}</p>}

      <main className="layout-grid">
        <aside className="card side-panel">
          <h2>Folders</h2>
          <ul className="tree-list">{renderFolderNodes(null)}</ul>
          <div className="stack-actions">
            <button
              disabled={busy || !selectedFolderId}
              onClick={() => {
                if (!selectedFolderId) {
                  return;
                }
                const name = window.prompt("Rename folder");
                if (!name) {
                  return;
                }
                runTask(async () => {
                  await api.renameFolder(selectedFolderId, name);
                }, "Folder renamed");
              }}
            >
              Rename Folder
            </button>
            <button
              disabled={busy || !selectedFolderId}
              onClick={() => {
                if (!selectedFolderId) {
                  return;
                }
                runTask(async () => {
                  await api.moveToTrash("folder", selectedFolderId);
                }, "Folder moved to trash");
              }}
            >
              Move Folder to Trash
            </button>
          </div>
        </aside>

        <section className="card entry-panel">
          <h2>Entries</h2>
          {activeEntries.map((entry: Entry) => (
            <div
              key={entry.id}
              className={selectedEntryId === entry.id ? "entry-row active" : "entry-row"}
            >
              <button className="entry-select" onClick={() => onSelectEntry(entry.id)}>
                <strong>{entry.title}</strong>
                <small>{formatDate(entry.created_at)}</small>
              </button>
              <div className="entry-actions">
                <button
                  disabled={busy}
                  onClick={() => {
                    const title = window.prompt("Rename entry", entry.title);
                    if (!title) {
                      return;
                    }
                    runTask(async () => {
                      await api.renameEntry(entry.id, title);
                    }, "Entry renamed");
                  }}
                >
                  Rename
                </button>
                <button
                  disabled={busy}
                  onClick={() => {
                    runTask(async () => {
                      await api.moveToTrash("entry", entry.id);
                    }, "Entry moved to trash");
                  }}
                >
                  Trash
                </button>
              </div>
            </div>
          ))}
        </section>

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

              <h3>Recording Sources</h3>
              <p className="help-text">
                Pick devices by name. On macOS 13+, select "System Audio (macOS Native)" for
                direct system/call capture. Loopback devices (for example BlackHole) remain as
                fallback options.
              </p>
              <p className="help-text">
                macOS loopback setup: 1) Open Audio MIDI Setup. 2) Create a Multi-Output Device
                with your speakers + BlackHole 2ch. 3) Set your call app output to that
                Multi-Output device. 4) Keep microphone as a separate source in this app.
              </p>
              <button
                disabled={busy}
                onClick={() =>
                  runTask(async () => {
                    await loadRecordingDevices();
                  }, "Audio devices refreshed")
                }
              >
                Refresh Audio Devices
              </button>
              {!hasLoopbackDevice && recordingDevices.length > 0 && (
                <p className="help-text">
                  No loopback/speaker device detected yet. Install and configure BlackHole to record
                  audio coming from your speakers/call app.
                </p>
              )}
              {recordingDevices.length > 0 && (
                <div className="hint-box">
                  <strong>Detected Devices</strong>
                  {recordingDevices.map((device) => (
                    <code key={deviceKey(device)}>
                      {device.name}
                      {device.is_loopback ? " (Loopback/System)" : ""}
                    </code>
                  ))}
                </div>
              )}
              {recordingDevices.length === 0 && (
                <p className="help-text">
                  No audio devices detected yet. Click "Refresh Audio Devices".
                </p>
              )}
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
                        {device.is_loopback ? " (Loopback/System)" : ""}
                      </option>
                    ))}
                  </select>
                  <button
                    disabled={busy || sources.length <= 1}
                    onClick={() => setSources(sources.filter((_, i) => i !== index))}
                  >
                    Remove
                  </button>
                </div>
              ))}
              <button
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
                + Add Source
              </button>

              <div className="action-row">
                {!recordingSessionId ? (
                  <button
                    disabled={busy || transcribingAfterStop || sources.length === 0}
                    onClick={() => {
                      runTask(async () => {
                        const sessionId = await api.startRecording(activeEntry.id, sources);
                        setRecordingSessionId(sessionId);
                        setRecordingPaused(false);
                      }, "Recording started");
                    }}
                  >
                    Start Recording
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

                <button
                  disabled={busy || transcribingAfterStop}
                  onClick={() =>
                    runTask(
                      async () => api.transcribeEntry(activeEntry.id, transcriptionLanguage),
                      "Transcription ready"
                    )
                  }
                >
                  Transcribe
                </button>

                {ARTIFACT_TYPES.map((item) => (
                  <button
                    key={item.type}
                    disabled={busy || transcribingAfterStop}
                    onClick={() =>
                      runTask(
                        async () => api.generateArtifact(activeEntry.id, item.type),
                        `${item.label} completed`
                      )
                    }
                  >
                    {item.label}
                  </button>
                ))}

                <button
                  disabled={busy || transcribingAfterStop}
                  onClick={() => {
                    runTask(async () => {
                      const path = await api.exportEntry(activeEntry.id);
                      setNotice(`Export created at ${path}`);
                    });
                  }}
                >
                  Export Markdown + Audio
                </button>
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
                        Signal level: {Math.round(recordingLevel * 100)}% | Captured:{" "}
                        {formatBytes(recordingBytes)}
                      </p>
                      {selectedLoopbackOnly && recordingLevel < 0.03 && (
                        <p className="help-text">
                          {selectedNativeSystemSource
                            ? "System source appears silent. Ensure macOS granted Screen & System Audio Recording permission and your call app is playing audio."
                            : "Loopback input appears silent. Ensure your call/browser output is routed to BlackHole (or a Multi-Output Device that includes BlackHole)."}
                        </p>
                      )}
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
              <p className="help-text">
                Use Auto for mixed calls. If Auto is wrong, force the spoken language (for example
                Russian) to transcribe in that language, not English.
              </p>
              {latestTranscript && (
                <p className="help-text">
                  Version {latestTranscript.version} | Language: {latestTranscript.language} | Updated:
                  {" "}
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

              <h3>Artifacts</h3>
              {ARTIFACT_TYPES.map((item) => {
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
        </section>
      </main>

      <section className="card settings-panel">
        <h2>Local Model & Prompt Settings</h2>
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

      <section className="card settings-panel">
        <h2>Trash</h2>
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
    </div>
  );
}
