export interface CiStatusReport {
  hasFailure: boolean;
  workflowName: string;
  branch: string;
  runId: string | number;
  commitSha?: string;
  commitMessage?: string;
  failedJobName?: string;
  failureStep?: string;
  errorSnippet?: string;
  conclusion?: string;
  url?: string;
  htmlUrl?: string;
  message?: string;
  updatedAtMs?: number;
}

export interface CiRepairOptions {
  onPopulateComposer?: (prompt: string) => void;
  cwd?: string;
  dispatchMode?: "dispatch" | "insert";
}
