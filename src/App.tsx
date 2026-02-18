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
    { label: "Microphone", format: "avfoundation", input: ":0" },
    { label: "Loopback/System", format: "avfoundation", input: ":1" }
  ]);
  const [transcriptDraft, setTranscriptDraft] = useState<string>("");
  const [deviceHints, setDeviceHints] = useState<string[]>([]);
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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

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

  async function reloadBootstrap(keepSelection = true) {
    const data = await api.bootstrapState();
    setBootstrap(data);
    setModelName(data.model_name);
    const nextPrompts = { ...promptDrafts };
    for (const template of data.prompt_templates) {
      nextPrompts[template.role] = template.prompt_text;
    }
    setPromptDrafts(nextPrompts);

    if (!keepSelection) {
      const firstFolder = data.folders.find((folder) => !folder.deleted_at);
      setSelectedFolderId(firstFolder?.id ?? null);
      setSelectedEntryId(null);
      setEntryBundle(null);
    }

    if (keepSelection && selectedFolderId) {
      const exists = data.folders.some((folder) => folder.id === selectedFolderId && !folder.deleted_at);
      if (!exists) {
        setSelectedFolderId(null);
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

  return (
    <div className="app-shell">
      <header className="app-header">
        <h1>AI Call Recorder Local</h1>
        <div className="header-actions">
          <button
            onClick={() => {
              const name = window.prompt("Root folder name");
              if (!name) {
                return;
              }
              runTask(async () => {
                await api.createFolder(name, null);
              }, "Root folder created");
            }}
            disabled={busy}
          >
            + Root Folder
          </button>
          <button
            onClick={() => {
              if (!selectedFolderId) {
                setError("Select a folder first");
                return;
              }
              const name = window.prompt("Subfolder name");
              if (!name) {
                return;
              }
              runTask(async () => {
                await api.createFolder(name, selectedFolderId);
              }, "Subfolder created");
            }}
            disabled={busy || !selectedFolderId}
          >
            + Subfolder
          </button>
          <button
            onClick={() => {
              if (!selectedFolderId) {
                setError("Select a folder first");
                return;
              }
              const title = window.prompt("Entry title");
              if (!title) {
                return;
              }
              runTask(async () => {
                await api.createEntry(selectedFolderId, title);
              }, "Entry created");
            }}
            disabled={busy || !selectedFolderId}
          >
            + Entry
          </button>
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
              <p>Status: {activeEntry.status}</p>
              <p>Duration: {activeEntry.duration_sec}s</p>

              <h3>Recording Sources</h3>
              <p className="help-text">
                Use `avfoundation` on macOS (`:0`, `:1`) or `dshow` on Windows (device name). These
                map directly to ffmpeg inputs.
              </p>
              <button
                disabled={busy}
                onClick={() =>
                  runTask(async () => {
                    const hints = await api.listAudioDeviceHints();
                    setDeviceHints(hints);
                  }, "Audio device hints loaded")
                }
              >
                Detect Audio Inputs
              </button>
              {deviceHints.length > 0 && (
                <div className="hint-box">
                  <strong>Detected Devices</strong>
                  {deviceHints.map((hint) => (
                    <code key={hint}>{hint}</code>
                  ))}
                </div>
              )}
              {sources.map((source, index) => (
                <div className="source-row" key={`${source.label}-${index}`}>
                  <input
                    value={source.label}
                    onChange={(event) => {
                      const next = [...sources];
                      next[index] = { ...next[index], label: event.target.value };
                      setSources(next);
                    }}
                    placeholder="Label"
                  />
                  <input
                    value={source.format}
                    onChange={(event) => {
                      const next = [...sources];
                      next[index] = { ...next[index], format: event.target.value };
                      setSources(next);
                    }}
                    placeholder="ffmpeg format"
                  />
                  <input
                    value={source.input}
                    onChange={(event) => {
                      const next = [...sources];
                      next[index] = { ...next[index], input: event.target.value };
                      setSources(next);
                    }}
                    placeholder="ffmpeg input"
                  />
                  <button
                    disabled={busy || sources.length <= 1}
                    onClick={() => setSources(sources.filter((_, i) => i !== index))}
                  >
                    Remove
                  </button>
                </div>
              ))}
              <button
                disabled={busy}
                onClick={() =>
                  setSources([...sources, { label: "New Source", format: "avfoundation", input: ":0" }])
                }
              >
                + Add Source
              </button>

              <div className="action-row">
                {!recordingSessionId ? (
                  <button
                    disabled={busy}
                    onClick={() => {
                      const confirmed = window.confirm(
                        "Confirm you have consent to record this call under your local laws."
                      );
                      if (!confirmed) {
                        return;
                      }

                      runTask(async () => {
                        const sessionId = await api.startRecording(activeEntry.id, sources);
                        setRecordingSessionId(sessionId);
                      }, "Recording started");
                    }}
                  >
                    Start Recording
                  </button>
                ) : (
                  <button
                    disabled={busy}
                    onClick={() => {
                      runTask(async () => {
                        await api.stopRecording(recordingSessionId);
                        setRecordingSessionId(null);
                      }, "Recording stopped");
                    }}
                  >
                    Stop Recording
                  </button>
                )}

                <button
                  disabled={busy}
                  onClick={() => runTask(async () => api.transcribeEntry(activeEntry.id), "Transcription ready")}
                >
                  Transcribe
                </button>

                {ARTIFACT_TYPES.map((item) => (
                  <button
                    key={item.type}
                    disabled={busy}
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
                  disabled={busy}
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

              <h3>Transcript</h3>
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
                  const language = latestTranscript?.language ?? "auto";
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
