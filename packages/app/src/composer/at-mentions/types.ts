/**
 * Kronos Composer @ Mentions Types
 */

export type MentionKind = "file" | "folder" | "git_diff" | "git_status" | "terminal";

export interface MentionItem {
  id: string;
  kind: MentionKind;
  label: string;
  detail?: string;
  insertToken: string;
}

export const STATIC_MENTIONS: readonly MentionItem[] = [
  {
    id: "git_diff",
    kind: "git_diff",
    label: "Git Diff",
    detail: "Attach uncommitted git diff into prompt",
    insertToken: "@git:diff ",
  },
  {
    id: "git_status",
    kind: "git_status",
    label: "Git Status",
    detail: "Attach git branch and modified files summary",
    insertToken: "@git:status ",
  },
  {
    id: "terminal",
    kind: "terminal",
    label: "Terminal Output",
    detail: "Attach latest terminal execution logs and output",
    insertToken: "@terminal:last-output ",
  },
];
