import { describe, expect, it } from "vitest";
import { formatContextPercentage, resolveModelContextWindow } from "./context-window-meter.utils";

describe("context window meter formatting", () => {
  it("localizes percentages without hiding context overflow", () => {
    expect(formatContextPercentage(125, "en")).toBe("125%");
    expect(formatContextPercentage(42, "fr")).toBe("42\u00a0%");
  });

  it("resolves context window from catalog models dynamically", () => {
    const models = [
      { id: "custom/fast-model", contextWindowMaxTokens: 64000 },
      {
        id: "anthropic/claude-3-7-sonnet",
        aliases: ["claude-3-7-sonnet"],
        contextWindowMaxTokens: 200000,
      },
    ];
    expect(resolveModelContextWindow("custom/fast-model", "omp", models)).toBe(64000);
    expect(resolveModelContextWindow("claude-3-7-sonnet", "omp", models)).toBe(200000);
  });

  it("returns null without hardcoding when model or capacity is unknown", () => {
    const models = [{ id: "custom/fast-model", contextWindowMaxTokens: 64000 }];
    expect(resolveModelContextWindow("unknown-model", "omp", models)).toBeNull();
    expect(resolveModelContextWindow(null, "claude", models)).toBeNull();
    expect(resolveModelContextWindow("custom/fast-model", "omp", null)).toBeNull();
  });
});
