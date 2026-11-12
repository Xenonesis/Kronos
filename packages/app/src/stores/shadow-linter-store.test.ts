import { describe, it, expect, beforeEach } from "vitest";
import { useShadowLinterStore } from "./shadow-linter-store";

describe("shadow-linter-store", () => {
  beforeEach(() => {
    useShadowLinterStore.setState({ diagnosticsByAgent: {}, dismissedByAgent: {} });
  });

  it("sets and retrieves diagnostic for an agent", () => {
    const diag = {
      system: "tsc --noEmit",
      errorCount: 2,
      primaryFile: "src/main.ts",
      rawOutput: "error TS2322",
      repairPrompt: "Fix error TS2322",
    };
    useShadowLinterStore.getState().setDiagnostic("agent-1", diag);

    expect(useShadowLinterStore.getState().diagnosticsByAgent["agent-1"]).toEqual(diag);
    expect(useShadowLinterStore.getState().dismissedByAgent["agent-1"]).toBe(false);
  });

  it("dismisses diagnostic for an agent", () => {
    useShadowLinterStore.getState().dismissDiagnostic("agent-1");
    expect(useShadowLinterStore.getState().dismissedByAgent["agent-1"]).toBe(true);
  });

  it("clears diagnostic for an agent", () => {
    useShadowLinterStore.getState().setDiagnostic("agent-1", {
      system: "cargo check",
      errorCount: 1,
      rawOutput: "error[E0308]",
      repairPrompt: "Fix type mismatch",
    });
    useShadowLinterStore.getState().clearDiagnostic("agent-1");
    expect(useShadowLinterStore.getState().diagnosticsByAgent["agent-1"]).toBeUndefined();
  });
});
