import { describe, it, expect } from "vitest";
import { parseTestOutput, formatTestRepairPrompt } from "./test-driven-loop";

describe("test-driven-loop", () => {
  it("parses passing Vitest output cleanly", () => {
    const output = `
      ✓ packages/app/src/utils/test.test.ts (2 tests)
      Test Files  1 passed (1)
      Tests  2 passed (2)
      Duration  120ms
    `;
    const res = parseTestOutput(output);
    expect(res.passed).toBe(true);
    expect(res.failedTestsCount).toBe(0);
  });

  it("parses failing Vitest output and extracts failure snippet", () => {
    const output = `
      ❯ packages/app/src/utils/test.test.ts (2 tests | 1 failed)
        × should calculate correct sum
          AssertionError: expected 4 to deeply equal 5
            at packages/app/src/utils/test.test.ts:12:15
      Tests  1 failed | 1 passed (2)
    `;
    const res = parseTestOutput(output);
    expect(res.passed).toBe(false);
    expect(res.failedTestsCount).toBe(1);
    expect(res.failureSnippet).toContain("AssertionError");
  });

  it("parses Cargo test output correctly", () => {
    const cargoPass = "test result: ok. 12 passed; 0 failed; 0 ignored";
    expect(parseTestOutput(cargoPass).passed).toBe(true);

    const cargoFail = "test result: FAILED. 10 passed; 2 failed; 0 ignored";
    const failRes = parseTestOutput(cargoFail);
    expect(failRes.passed).toBe(false);
    expect(failRes.failedTestsCount).toBe(2);
  });

  it("formats actionable test repair prompt", () => {
    const prompt = formatTestRepairPrompt(
      "npm test",
      1,
      5,
      "AssertionError: expected true to be false",
    );
    expect(prompt).toContain("Autonomous Test-Driven Repair (Iteration 1/5)");
    expect(prompt).toContain("`npm test`");
    expect(prompt).toContain("AssertionError");
  });
});
