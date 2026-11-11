import { create } from "zustand";

export interface ScratchpadState {
  objective: string;
  hypotheses: string[];
  findings: string[];
  decisions: string[];
  blockers: string[];
  lastUpdated: number;
}

const DEFAULT_SCRATCHPAD: ScratchpadState = {
  objective: "",
  hypotheses: [],
  findings: [],
  decisions: [],
  blockers: [],
  lastUpdated: Date.now(),
};

interface ScratchpadStore {
  scratchpads: Record<string, ScratchpadState>;
  getScratchpad: (agentId: string) => ScratchpadState;
  setObjective: (agentId: string, objective: string) => void;
  addHypothesis: (agentId: string, item: string) => void;
  addFinding: (agentId: string, item: string) => void;
  addDecision: (agentId: string, item: string) => void;
  addBlocker: (agentId: string, item: string) => void;
  removeBlocker: (agentId: string, index: number) => void;
  clearScratchpad: (agentId: string) => void;
  parseStreamIntoScratchpad: (agentId: string, content: string) => void;
}

export const useScratchpadStore = create<ScratchpadStore>((set, get) => ({
  scratchpads: {},

  getScratchpad: (agentId: string): ScratchpadState => {
    return get().scratchpads[agentId] ?? { ...DEFAULT_SCRATCHPAD, lastUpdated: Date.now() };
  },

  setObjective: (agentId: string, objective: string) => {
    set((state) => {
      const current = state.scratchpads[agentId] ?? { ...DEFAULT_SCRATCHPAD };
      return {
        scratchpads: {
          ...state.scratchpads,
          [agentId]: { ...current, objective, lastUpdated: Date.now() },
        },
      };
    });
  },

  addHypothesis: (agentId: string, item: string) => {
    set((state) => {
      const current = state.scratchpads[agentId] ?? { ...DEFAULT_SCRATCHPAD };
      if (current.hypotheses.includes(item)) return state;
      return {
        scratchpads: {
          ...state.scratchpads,
          [agentId]: {
            ...current,
            hypotheses: [...current.hypotheses, item],
            lastUpdated: Date.now(),
          },
        },
      };
    });
  },

  addFinding: (agentId: string, item: string) => {
    set((state) => {
      const current = state.scratchpads[agentId] ?? { ...DEFAULT_SCRATCHPAD };
      if (current.findings.includes(item)) return state;
      return {
        scratchpads: {
          ...state.scratchpads,
          [agentId]: {
            ...current,
            findings: [...current.findings, item],
            lastUpdated: Date.now(),
          },
        },
      };
    });
  },

  addDecision: (agentId: string, item: string) => {
    set((state) => {
      const current = state.scratchpads[agentId] ?? { ...DEFAULT_SCRATCHPAD };
      if (current.decisions.includes(item)) return state;
      return {
        scratchpads: {
          ...state.scratchpads,
          [agentId]: {
            ...current,
            decisions: [...current.decisions, item],
            lastUpdated: Date.now(),
          },
        },
      };
    });
  },

  addBlocker: (agentId: string, item: string) => {
    set((state) => {
      const current = state.scratchpads[agentId] ?? { ...DEFAULT_SCRATCHPAD };
      if (current.blockers.includes(item)) return state;
      return {
        scratchpads: {
          ...state.scratchpads,
          [agentId]: {
            ...current,
            blockers: [...current.blockers, item],
            lastUpdated: Date.now(),
          },
        },
      };
    });
  },

  removeBlocker: (agentId: string, index: number) => {
    set((state) => {
      const current = state.scratchpads[agentId] ?? { ...DEFAULT_SCRATCHPAD };
      const nextBlockers = current.blockers.filter((_, i) => i !== index);
      return {
        scratchpads: {
          ...state.scratchpads,
          [agentId]: { ...current, blockers: nextBlockers, lastUpdated: Date.now() },
        },
      };
    });
  },

  clearScratchpad: (agentId: string) => {
    set((state) => {
      const next = { ...state.scratchpads };
      delete next[agentId];
      return { scratchpads: next };
    });
  },

  parseStreamIntoScratchpad: (agentId: string, content: string) => {
    if (!content) return;
    const lines = content.split("\n");
    let currentCategory: "objective" | "hypothesis" | "finding" | "decision" | "blocker" | null =
      null;

    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line) continue;

      if (/^(?:#+\s*)?(?:objective|goal):/i.test(line)) {
        currentCategory = "objective";
        const val = line.replace(/^(?:#+\s*)?(?:objective|goal):\s*/i, "").trim();
        if (val) get().setObjective(agentId, val);
        continue;
      }
      if (/^(?:#+\s*)?(?:hypotheses|hypothesis):/i.test(line)) {
        currentCategory = "hypothesis";
        const val = line.replace(/^(?:#+\s*)?(?:hypotheses|hypothesis):\s*/i, "").trim();
        if (val) get().addHypothesis(agentId, val);
        continue;
      }
      if (/^(?:#+\s*)?(?:findings|findings discovered|discovery):/i.test(line)) {
        currentCategory = "finding";
        const val = line.replace(/^(?:#+\s*)?(?:findings|discovery):\s*/i, "").trim();
        if (val) get().addFinding(agentId, val);
        continue;
      }
      if (/^(?:#+\s*)?(?:decisions|decision made):/i.test(line)) {
        currentCategory = "decision";
        const val = line.replace(/^(?:#+\s*)?(?:decisions|decision made):\s*/i, "").trim();
        if (val) get().addDecision(agentId, val);
        continue;
      }
      if (/^(?:#+\s*)?(?:blockers|blocker|impediment):/i.test(line)) {
        currentCategory = "blocker";
        const val = line.replace(/^(?:#+\s*)?(?:blockers|blocker):\s*/i, "").trim();
        if (val) get().addBlocker(agentId, val);
        continue;
      }

      if (line.startsWith("- ") || line.startsWith("* ")) {
        const item = line.replace(/^[-*]\s*/, "").trim();
        if (!item) continue;
        if (currentCategory === "hypothesis") get().addHypothesis(agentId, item);
        else if (currentCategory === "finding") get().addFinding(agentId, item);
        else if (currentCategory === "decision") get().addDecision(agentId, item);
        else if (currentCategory === "blocker") get().addBlocker(agentId, item);
      }
    }
  },
}));
