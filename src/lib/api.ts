import { invoke } from "@tauri-apps/api/core";
import type {
  ArtifactType,
  BootstrapState,
  EntryBundle,
  PromptRole,
  RecordingSource
} from "./types";

export const api = {
  bootstrapState: () => invoke<BootstrapState>("bootstrap_state"),
  getEntryBundle: (entryId: string) =>
    invoke<EntryBundle>("get_entry_bundle", { entryId }),
  createFolder: (name: string, parentId: string | null) =>
    invoke<void>("create_folder", { name, parentId }),
  renameFolder: (folderId: string, name: string) =>
    invoke<void>("rename_folder", { folderId, name }),
  createEntry: (folderId: string, title: string) =>
    invoke<void>("create_entry", { folderId, title }),
  renameEntry: (entryId: string, title: string) =>
    invoke<void>("rename_entry", { entryId, title }),
  moveToTrash: (entityType: "folder" | "entry", id: string) =>
    invoke<void>("move_to_trash", { entityType, id }),
  restoreFromTrash: (entityType: "folder" | "entry", id: string) =>
    invoke<void>("restore_from_trash", { entityType, id }),
  purgeEntity: (entityType: "folder" | "entry", id: string) =>
    invoke<void>("purge_entity", { entityType, id }),
  startRecording: (entryId: string, sources: RecordingSource[]) =>
    invoke<string>("start_recording", { entryId, sources }),
  stopRecording: (sessionId: string) =>
    invoke<void>("stop_recording", { sessionId }),
  transcribeEntry: (entryId: string, language: string | null = null) =>
    invoke<void>("transcribe_entry", { entryId, language }),
  generateArtifact: (entryId: string, artifactType: ArtifactType) =>
    invoke<void>("generate_artifact", { entryId, artifactType }),
  updateTranscript: (entryId: string, text: string, language: string) =>
    invoke<void>("update_transcript", { entryId, text, language }),
  updateArtifact: (entryId: string, artifactType: ArtifactType, text: string) =>
    invoke<void>("update_artifact", { entryId, artifactType, text }),
  updatePrompt: (role: PromptRole, promptText: string) =>
    invoke<void>("update_prompt_template", { role, promptText }),
  updateModelName: (modelName: string) =>
    invoke<void>("update_model_name", { modelName }),
  listAudioDeviceHints: () => invoke<string[]>("list_audio_device_hints"),
  exportEntry: (entryId: string) =>
    invoke<string>("export_entry_markdown", { entryId })
};
