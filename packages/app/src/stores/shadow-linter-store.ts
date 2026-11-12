import { create } from "zustand";

export interface ShadowLintDiagnostic {
  system: string;
  errorCount: number;
  primaryFile?: string;
  rawOutput: string;
  repairPrompt: string;
}

interface ShadowLinterState {
  diagnosticsByAgent: Record<string, ShadowLintDiagnostic>;
  dismissedByAgent: Record<string, boolean>;
  setDiagnostic: (agentId: string, diag: ShadowLintDiagnostic) => void;
  clearDiagnostic: (agentId: string) => void;
  dismissDiagnostic: (agentId: string) => void;
}

export const useShadowLinterStore = create<ShadowLinterState>((set) => ({
  diagnosticsByAgent: {},
  dismissedByAgent: {},
  setDiagnostic: (agentId, diag) =>
    set((state) => ({
      diagnosticsByAgent: { ...state.diagnosticsByAgent, [agentId]: diag },
      dismissedByAgent: { ...state.dismissedByAgent, [agentId]: false },
    })),
  clearDiagnostic: (agentId) =>
    set((state) => {
      const copy = { ...state.diagnosticsByAgent };
      delete copy[agentId];
      return { diagnosticsByAgent: copy };
    }),
  dismissDiagnostic: (agentId) =>
    set((state) => ({
      dismissedByAgent: { ...state.dismissedByAgent, [agentId]: true },
    })),
}));
