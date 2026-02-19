export type ArtifactType =
  | "summary"
  | "analysis"
  | "critique_recruitment"
  | "critique_sales"
  | "critique_cs";

export type PromptRole =
  | "summary"
  | "analysis"
  | "critique_recruitment"
  | "critique_sales"
  | "critique_cs";

export interface Folder {
  id: string;
  parent_id: string | null;
  name: string;
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
}

export interface Entry {
  id: string;
  folder_id: string;
  title: string;
  status: string;
  duration_sec: number;
  recording_path: string | null;
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
}

export interface TranscriptRevision {
  id: string;
  entry_id: string;
  version: number;
  text: string;
  language: string;
  is_manual_edit: boolean;
  created_at: string;
}

export interface ArtifactRevision {
  id: string;
  entry_id: string;
  artifact_type: ArtifactType;
  version: number;
  text: string;
  source_transcript_version: number;
  is_stale: boolean;
  is_manual_edit: boolean;
  created_at: string;
}

export interface PromptTemplate {
  role: PromptRole;
  prompt_text: string;
  updated_at: string;
}

export interface BootstrapState {
  folders: Folder[];
  entries: Entry[];
  prompt_templates: PromptTemplate[];
  model_name: string;
}

export interface EntryBundle {
  transcript_revisions: TranscriptRevision[];
  artifact_revisions: ArtifactRevision[];
}

export interface RecordingSource {
  label: string;
  format: string;
  input: string;
}

export interface RecordingDevice {
  name: string;
  format: string;
  input: string;
  is_loopback: boolean;
}

export interface RecordingMeter {
  bytes_written: number;
  level: number;
}
