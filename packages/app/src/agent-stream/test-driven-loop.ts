/**
 * Kronos Autonomous Test-Driven Repair Loop
 *
 * Provides Claude Code / Aider parity (`/test-loop <command>`).
 * Executes test commands, parses test failures/stack traces, creates targeted repair prompts,
 * and orchestrates continuous test-and-repair iterations until all tests pass or max iterations reached.
 */

export interface TestRunResult {
  passed: boolean;
  failedTestsCount: number;
  totalTestsCount?: number;
  failureSnippet: string;
}

export interface TestLoopIteration {
  iteration: number;
  command: string;
  passed: boolean;
  failureSnippet: string;
  filesModified: string[];
  timestamp: number;
}

export interface TestLoopSession {
  id: string;
  command: string;
  maxIterations: number;
  currentIteration: number;
  status: "running" | "passed" | "failed" | "stopped";
  iterations: TestLoopIteration[];
}

/**
 * Parses stdout/stderr output from common test frameworks (Vitest, Jest, Cargo Test, Pytest, Go Test).
 */
export function parseTestOutput(rawOutput: string): TestRunResult {
  const normalized = rawOutput.replace(/\r\n/g, "\n");

  // Check for successful runs first
  const isVitestPass = /Tests\s+(\d+)\s+passed/i.test(normalized) && !/failed/i.test(normalized);
  const isCargoPass = /test result: ok\.\s+(\d+)\s+passed;\s+0\s+failed/i.test(normalized);
  const isPytestPass = /===\s+(\d+)\s+passed\s+in/i.test(normalized) && !/failed/i.test(normalized);
  const isJestPass =
    /Tests:\s+(\d+)\s+passed,\s+(\d+)\s+total/i.test(normalized) && !/failed/i.test(normalized);

  if (isVitestPass || isCargoPass || isPytestPass || isJestPass) {
    return {
      passed: true,
      failedTestsCount: 0,
      failureSnippet: "",
    };
  }

  // Detect failures in Vitest / Jest
  const vitestFailMatch = /Tests\s+(\d+)\s+failed/i.exec(normalized);
  const jestFailMatch = /Tests:\s+(\d+)\s+failed/i.exec(normalized);
  const cargoFailMatch = /test result: FAILED\.\s+(\d+)\s+passed;\s+(\d+)\s+failed/i.exec(
    normalized,
  );
  const pytestFailMatch = /(\d+)\s+failed/i.exec(normalized);

  let failedCount = 0;
  if (vitestFailMatch) failedCount = parseInt(vitestFailMatch[1], 10);
  else if (jestFailMatch) failedCount = parseInt(jestFailMatch[1], 10);
  else if (cargoFailMatch) failedCount = parseInt(cargoFailMatch[2], 10);
  else if (pytestFailMatch) failedCount = parseInt(pytestFailMatch[1], 10);
  else if (/FAIL|FAILED|assertion error/i.test(normalized)) failedCount = 1;

  if (failedCount === 0 && !/FAIL|FAILED|error:/i.test(normalized)) {
    return {
      passed: true,
      failedTestsCount: 0,
      failureSnippet: "",
    };
  }

  // Extract relevant failure lines (max 40 lines around failures)
  const lines = normalized.split("\n");
  const failureLines: string[] = [];
  let capturing = false;
  let capturedCount = 0;

  for (const line of lines) {
    if (/FAIL|FAILED|AssertionError|assertion failed|Error:|stack trace/i.test(line)) {
      capturing = true;
    }
    if (capturing) {
      failureLines.push(line);
      capturedCount++;
      if (capturedCount >= 40) break;
    }
  }

  const snippet = failureLines.length > 0 ? failureLines.join("\n") : lines.slice(-25).join("\n");

  return {
    passed: false,
    failedTestsCount: Math.max(1, failedCount),
    failureSnippet: snippet.trim(),
  };
}

/**
 * Formats a self-healing test repair prompt for the agent composer.
 */
export function formatTestRepairPrompt(
  command: string,
  iteration: number,
  maxIterations: number,
  failureSnippet: string,
): string {
  return (
    `### 🧪 Autonomous Test-Driven Repair (Iteration ${iteration}/${maxIterations})\n` +
    `The test command \`${command}\` failed with the following assertions/errors:\n\n` +
    `\`\`\`\n` +
    `${failureSnippet}\n` +
    `\`\`\`\n\n` +
    `**Instructions:**\n` +
    `1. Identify the failing assertions and the source file(s) responsible.\n` +
    `2. Apply minimal, precise edits to fix the root cause without breaking existing features.\n` +
    `3. Ensure all tests pass cleanly when re-executed.`
  );
}
