export type HistoryScope = "workspace" | "project" | "all";
export type HistorySort = "last_updated" | "messages" | "newest";

export interface UniversalIndexedSessionItem {
  id: string;
  provider: string; // "antigravity" | "omp" | "claude" | "codex" | "opencode" | "pi" | "kronos"
  providerId?: string;
  provider_id?: string;
  project_name: string;
  projectName?: string;
  directory?: string | null;
  cwd?: string | null;
  title: string;
  preview?: string | null;
  last_message_preview?: string | null;
  lastMessagePreview?: string | null;
  message_count: number;
  messageCount?: number;
  subagent_count: number;
  subagentCount?: number;
  model?: string | null;
  created_at?: string | null;
  updated_at?: string | null;
  last_activity_at?: string | null;
  lastActivityAt?: string | null;
  raw_path?: string | null;
}

export interface UniversalSessionHistoryProps {
  currentProjectCwd?: string;
  onSelectResumeSession?: (session: UniversalIndexedSessionItem) => void;
}
