import { describe, expect, it } from "vitest";
import { shouldShowCompactWarning } from "./context-window-meter.utils";

describe("context window meter proactive compaction warning", () => {
  it("triggers proactive compact warning at or above 75% context capacity", () => {
    expect(shouldShowCompactWarning(74)).toBe(false);
    expect(shouldShowCompactWarning(75)).toBe(true);
    expect(shouldShowCompactWarning(92)).toBe(true);
  });

  it("handles boundary values and invalid inputs safely", () => {
    expect(shouldShowCompactWarning(0)).toBe(false);
    expect(shouldShowCompactWarning(74.99)).toBe(false);
    expect(shouldShowCompactWarning(100)).toBe(true);
    expect(shouldShowCompactWarning(null)).toBe(false);
    expect(shouldShowCompactWarning(undefined)).toBe(false);
  });
});
