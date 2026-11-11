import { describe, it, expect } from "vitest";
import { synthesizeModelProposals } from "./consensus-synthesizer";

describe("consensus-synthesizer", () => {
  it("handles unanimous agreement between models", () => {
    const proposals = [
      {
        modelId: "claude-3-7-sonnet",
        modelName: "Claude 3.7 Sonnet",
        solutionText:
          "We need to fix the async race condition in auth.ts by awaiting token refresh.",
      },
      {
        modelId: "gpt-4o",
        modelName: "GPT-4o",
        solutionText: "Fix the race condition in auth.ts using async mutex on token refresh.",
      },
    ];

    const res = synthesizeModelProposals("Fix auth race condition", proposals);
    expect(res.hasUnanimousAgreement).toBe(true);
    expect(res.confidenceScore).toBeGreaterThanOrEqual(0.9);
    expect(res.agreements.length).toBeGreaterThan(0);
    expect(res.synthesizedPrompt).toContain("Multi-Model Consensus Synthesis");
  });

  it("handles single model fallback cleanly", () => {
    const proposals = [
      {
        modelId: "gemini-2-5-pro",
        modelName: "Gemini 2.5 Pro",
        solutionText: "Direct single model fix.",
      },
    ];
    const res = synthesizeModelProposals("Task", proposals);
    expect(res.hasUnanimousAgreement).toBe(true);
    expect(res.confidenceScore).toBe(1.0);
  });
});
