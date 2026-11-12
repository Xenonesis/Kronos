import { describe, expect, test, beforeEach } from "vitest";
import { calculateCostUsd, useCostBudgetStore } from "./cost-budget-store";

describe("cost-budget-store (FEAT-22)", () => {
  beforeEach(() => {
    useCostBudgetStore.setState({ budgetLimitUsd: 5.0, usageByAgent: {} });
  });

  test("calculates accurate USD costs for cloud and free local models", () => {
    const sonnetCost = calculateCostUsd({
      modelId: "claude-sonnet-4",
      inputTokens: 1_000_000,
      outputTokens: 100_000,
    });
    expect(sonnetCost).toBeCloseTo(4.5, 4);

    const ollamaCost = calculateCostUsd({
      modelId: "ollama/qwen2.5-coder",
      inputTokens: 500_000,
      outputTokens: 50_000,
    });
    expect(ollamaCost).toBe(0);
  });

  test("accumulates agent token usage and session total", () => {
    const store = useCostBudgetStore.getState();
    store.recordUsage({
      agentId: "ag-1",
      modelId: "gemini-2.5-flash",
      inputTokens: 100_000,
      outputTokens: 10_000,
    });
    store.recordUsage({
      agentId: "ag-1",
      modelId: "gemini-2.5-flash",
      inputTokens: 100_000,
      outputTokens: 10_000,
    });
    const entry = useCostBudgetStore.getState().usageByAgent["ag-1"];
    expect(entry?.turns).toBe(2);
    expect(entry?.inputTokens).toBe(200_000);
    expect(useCostBudgetStore.getState().getTotalSessionCostUsd()).toBeGreaterThan(0);
  });
});
