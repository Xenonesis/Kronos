import { create } from "zustand";
import { invokeTauri } from "@/utils/tauri-invoke";
import type { CiRepairOptions, CiStatusReport } from "./types";

export interface CiState {
  ciStatus: CiStatusReport | null;
  isLoading: boolean;
  isRepairing: boolean;
  isDismissed: boolean;
  failureLog: string | null;

  checkStatus: (cwd?: string, branch?: string) => Promise<CiStatusReport | null>;
  fetchFailureLog: (runId: string | number, cwd?: string) => Promise<string>;
  dismiss: () => void;
  resetDismissed: () => void;
  repairWithAgent: (options?: CiRepairOptions) => Promise<string>;
  simulateFailure: (mock?: Partial<CiStatusReport>) => void;
  clearStatus: () => void;
}

export const useCiStore = create<CiState>((set, get) => ({
  ciStatus: null,
  isLoading: false,
  isRepairing: false,
  isDismissed: false,
  failureLog: null,

  checkStatus: async (cwd, branch) => {
    set({ isLoading: true });
    try {
      // 1. Query Tauri bridge with workspacePath matching kernel/ci_repair.rs
      const report = await invokeTauri<{
        hasFailure: boolean;
        workflowName?: string;
        branch?: string;
        runId?: number;
        conclusion?: string;
        url?: string;
        message?: string;
      }>("ci_check_status", {
        workspacePath: cwd,
        cwd,
        branch: branch || "main",
      });

      if (report) {
        const normalized: CiStatusReport = {
          hasFailure: report.hasFailure,
          workflowName: report.workflowName || "CI Pipeline",
          branch: report.branch || branch || "main",
          runId: report.runId ?? 1,
          conclusion: report.conclusion,
          url: report.url,
          errorSnippet: report.message,
          updatedAtMs: Date.now(),
        };

        set({
          ciStatus: normalized,
          isLoading: false,
          isDismissed: false,
        });
        return normalized;
      }
    } catch {
      // Non-desktop or command unhandled
    }

    set({ isLoading: false });
    return get().ciStatus;
  },

  fetchFailureLog: async (runId, cwd) => {
    try {
      const numericRunId = typeof runId === "number" ? runId : parseInt(String(runId), 10) || 1;
      const log = await invokeTauri<string>("ci_get_failure_log", {
        runId: numericRunId,
        workspacePath: cwd,
        cwd,
      });
      if (log && typeof log === "string") {
        set({ failureLog: log });
        return log;
      }
    } catch {
      // fallback
    }

    const current = get().ciStatus;
    const fallbackLog = current?.errorSnippet
      ? current.errorSnippet
      : "No CI failure logs available. Please ensure the 'gh' CLI is installed and authenticated ('gh auth status'), and that workflow runs exist for this repository.";

    set({ failureLog: fallbackLog });
    return fallbackLog;
  },

  dismiss: () => {
    set({ isDismissed: true });
  },

  resetDismissed: () => {
    set({ isDismissed: false });
  },

  simulateFailure: (mock) => {
    set({
      ciStatus: {
        hasFailure: true,
        workflowName: mock?.workflowName ?? "ci.yml",
        branch: mock?.branch ?? "feature/monocode-integration",
        runId: mock?.runId ?? 8942,
        commitSha: mock?.commitSha ?? "f28b401",
        commitMessage: mock?.commitMessage ?? "feat: integrate monocode scratchpad and spotlight",
        failedJobName: mock?.failedJobName ?? "test (node 20.x)",
        failureStep: mock?.failureStep ?? "Run Vitest suite",
        errorSnippet:
          mock && "errorSnippet" in mock
            ? mock.errorSnippet
            : "FAIL src/features/pipeline.spec.ts\n  ✕ should correctly synchronize native state (assertion failed)",
        updatedAtMs: Date.now(),
      },
      isDismissed: false,
    });
  },

  clearStatus: () => {
    set({
      ciStatus: null,
      failureLog: null,
      isDismissed: false,
    });
  },

  repairWithAgent: async (options) => {
    const status = get().ciStatus;
    if (!status) return "";

    set({ isRepairing: true });
    try {
      const log = await get().fetchFailureLog(status.runId, options?.cwd);

      const prompt = `# CI Failure Auto-Repair Request

The continuous integration pipeline failed on branch **\`${status.branch}\`** for workflow **\`${status.workflowName}\`** (Run #${status.runId}).

## Failure Details
- **Workflow**: ${status.workflowName}
- **Branch**: ${status.branch}
- **Job**: ${status.failedJobName || "build / test"}
- **Step**: ${status.failureStep || "Unit & Integration Tests"}
${status.commitSha ? `- **Commit**: \`${status.commitSha}\` - ${status.commitMessage || ""}` : ""}

## CI Failure Log
\`\`\`log
${log.trim()}
\`\`\`

## Agent Instructions
1. Analyze the failing test output and stack trace above.
2. Locate the source files causing this failure in the repository.
3. Apply the required code changes to resolve the failure.
4. Run verification tests to ensure the CI build passes cleanly.
`;

      if (options?.onPopulateComposer) {
        options.onPopulateComposer(prompt);
      } else if (typeof window !== "undefined") {
        const eventName =
          options?.dispatchMode === "insert"
            ? "kronos:insert-composer"
            : "kronos:dispatch-composer";
        window.dispatchEvent(
          new CustomEvent(eventName, {
            detail: { text: prompt },
          }),
        );
      }

      set({ isRepairing: false, isDismissed: true });
      return prompt;
    } catch {
      set({ isRepairing: false });
      return "";
    }
  },
}));

export function useCiStatus() {
  return useCiStore();
}
