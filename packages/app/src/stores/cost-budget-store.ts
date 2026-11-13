import { create } from "zustand";
import { invokeDesktopCommand } from "@/desktop/electron/invoke";

/**
 * FEAT-22: Live Token Usage & USD Cost Budget Tracker Store (Claude Code parity)
 */

export interface ModelPricing {
  inputPerMillionUsd: number;
  outputPerMillionUsd: number;
  cacheReadPerMillionUsd: number;
}

const DEFAULT_PRICING: ModelPricing = {
  inputPerMillionUsd: 3.0,
  outputPerMillionUsd: 15.0,
  cacheReadPerMillionUsd: 0.3,
};

export const MODEL_PRICING_TABLE: Record<string, ModelPricing> = {
  "claude-sonnet": {
    inputPerMillionUsd: 3.0,
    outputPerMillionUsd: 15.0,
    cacheReadPerMillionUsd: 0.3,
  },
  "claude-opus": {
    inputPerMillionUsd: 15.0,
    outputPerMillionUsd: 75.0,
    cacheReadPerMillionUsd: 1.5,
  },
  "claude-haiku": {
    inputPerMillionUsd: 0.8,
    outputPerMillionUsd: 4.0,
    cacheReadPerMillionUsd: 0.08,
  },
  "gpt-4o": { inputPerMillionUsd: 2.5, outputPerMillionUsd: 10.0, cacheReadPerMillionUsd: 1.25 },
  o3: { inputPerMillionUsd: 10.0, outputPerMillionUsd: 40.0, cacheReadPerMillionUsd: 2.5 },
  codex: { inputPerMillionUsd: 1.5, outputPerMillionUsd: 6.0, cacheReadPerMillionUsd: 0.375 },
  "gemini-2.5-pro": {
    inputPerMillionUsd: 1.25,
    outputPerMillionUsd: 10.0,
    cacheReadPerMillionUsd: 0.31,
  },
  "gemini-2.5-flash": {
    inputPerMillionUsd: 0.15,
    outputPerMillionUsd: 0.6,
    cacheReadPerMillionUsd: 0.0375,
  },
  deepseek: { inputPerMillionUsd: 0.27, outputPerMillionUsd: 1.1, cacheReadPerMillionUsd: 0.07 },
  ollama: { inputPerMillionUsd: 0, outputPerMillionUsd: 0, cacheReadPerMillionUsd: 0 },
  local: { inputPerMillionUsd: 0, outputPerMillionUsd: 0, cacheReadPerMillionUsd: 0 },
};

export function resolveModelPricing(modelId?: string | null): ModelPricing {
  if (!modelId) return DEFAULT_PRICING;
  const lower = modelId.toLowerCase();
  for (const [key, pricing] of Object.entries(MODEL_PRICING_TABLE)) {
    if (lower.includes(key)) {
      return pricing;
    }
  }
  return DEFAULT_PRICING;
}

export function calculateCostUsd(input: {
  modelId?: string | null;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens?: number;
}): number {
  const pricing = resolveModelPricing(input.modelId);
  const inCost = (Math.max(0, input.inputTokens) / 1_000_000) * pricing.inputPerMillionUsd;
  const outCost = (Math.max(0, input.outputTokens) / 1_000_000) * pricing.outputPerMillionUsd;
  const cacheCost =
    (Math.max(0, input.cacheReadTokens ?? 0) / 1_000_000) * pricing.cacheReadPerMillionUsd;
  return inCost + outCost + cacheCost;
}

export interface AgentTokenBudgetEntry {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens?: number;
  costUsd: number;
  turns: number;
  modelId: string | null;
}

interface CostBudgetState {
  budgetLimitUsd: number;
  usageByAgent: Record<string, AgentTokenBudgetEntry>;
  recordedTurnKeys: Record<string, boolean>;
  requestedModalAgentId: string | null;
  setBudgetLimitUsd: (limitUsd: number) => void;
  recordUsage: (input: {
    agentId: string;
    turnId?: string;
    modelId?: string | null;
    inputTokens: number;
    outputTokens: number;
    cacheReadTokens?: number;
    cacheWriteTokens?: number;
    totalCostUsd?: number;
  }) => void;
  openCostModal: (agentId: string) => void;
  clearCostModalRequest: () => void;
  resetAgentUsage: (agentId: string) => void;
  getTotalSessionCostUsd: () => number;
}

export const useCostBudgetStore = create<CostBudgetState>((set, get) => ({
  budgetLimitUsd: 5.0,
  usageByAgent: {},
  recordedTurnKeys: {},
  requestedModalAgentId: null,

  setBudgetLimitUsd: (limitUsd) => {
    const safeLimit = Math.max(0.1, Number.isFinite(limitUsd) ? limitUsd : 5.0);
    set({ budgetLimitUsd: safeLimit });
    void invokeDesktopCommand("set_budget_policy", {
      policy: { maxTokens: null, maxCostUsd: safeLimit },
    }).catch(() => undefined);
  },

  openCostModal: (agentId) => {
    set({ requestedModalAgentId: agentId });
  },

  clearCostModalRequest: () => {
    set({ requestedModalAgentId: null });
  },

  recordUsage: ({
    agentId,
    turnId,
    modelId = null,
    inputTokens,
    outputTokens,
    cacheReadTokens = 0,
    cacheWriteTokens = 0,
    totalCostUsd,
  }) => {
    if (turnId && get().recordedTurnKeys[`${agentId}:${turnId}`]) {
      return;
    }
    const turnCost =
      typeof totalCostUsd === "number" && totalCostUsd > 0
        ? totalCostUsd
        : calculateCostUsd({ modelId, inputTokens, outputTokens, cacheReadTokens });
    set((state) => {
      const prev = state.usageByAgent[agentId] ?? {
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        costUsd: 0,
        turns: 0,
        modelId,
      };
      const nextRecorded = turnId
        ? { ...state.recordedTurnKeys, [`${agentId}:${turnId}`]: true }
        : state.recordedTurnKeys;
      return {
        recordedTurnKeys: nextRecorded,
        usageByAgent: {
          ...state.usageByAgent,
          [agentId]: {
            inputTokens: prev.inputTokens + inputTokens,
            outputTokens: prev.outputTokens + outputTokens,
            cacheReadTokens: prev.cacheReadTokens + cacheReadTokens,
            cacheWriteTokens: (prev.cacheWriteTokens ?? 0) + cacheWriteTokens,
            costUsd: prev.costUsd + turnCost,
            turns: prev.turns + 1,
            modelId: modelId ?? prev.modelId,
          },
        },
      };
    });
  },

  resetAgentUsage: (agentId) => {
    set((state) => {
      const next = { ...state.usageByAgent };
      delete next[agentId];
      return { usageByAgent: next };
    });
    void invokeDesktopCommand("reset_agent_budget_usage", { agentId }).catch(() => undefined);
  },

  getTotalSessionCostUsd: () => {
    return Object.values(get().usageByAgent).reduce((sum, entry) => sum + entry.costUsd, 0);
  },
}));
