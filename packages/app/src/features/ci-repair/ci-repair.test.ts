import { describe, it, expect, beforeEach, vi } from "vitest";
import { useCiStore } from "./use-ci-status";

describe("useCiStore", () => {
  beforeEach(() => {
    useCiStore.getState().clearStatus();
  });

  it("initializes with null status and not repairing", () => {
    const state = useCiStore.getState();
    expect(state.ciStatus).toBeNull();
    expect(state.isRepairing).toBe(false);
    expect(state.isDismissed).toBe(false);
  });

  it("simulates a CI failure and manages dismissal", () => {
    const store = useCiStore.getState();
    store.simulateFailure({
      workflowName: "deploy.yml",
      branch: "release/v2",
      runId: 1042,
      failedJobName: "e2e-tests",
      failureStep: "Playwright browser tests",
      errorSnippet: "Error: page.goto timed out after 30000ms",
    });

    let state = useCiStore.getState();
    expect(state.ciStatus).not.toBeNull();
    expect(state.ciStatus?.hasFailure).toBe(true);
    expect(state.ciStatus?.workflowName).toBe("deploy.yml");
    expect(state.ciStatus?.branch).toBe("release/v2");
    expect(state.ciStatus?.runId).toBe(1042);
    expect(state.isDismissed).toBe(false);

    store.dismiss();
    expect(useCiStore.getState().isDismissed).toBe(true);

    store.resetDismissed();
    expect(useCiStore.getState().isDismissed).toBe(false);
  });

  it("fetches failure log with fallback when IPC returns empty", async () => {
    const store = useCiStore.getState();
    store.simulateFailure({
      errorSnippet: "FAIL test/api.test.ts (Assertion failed)",
    });

    const log = await store.fetchFailureLog(999, "/dummy");
    expect(log).toContain("FAIL test/api.test.ts (Assertion failed)");
    expect(useCiStore.getState().failureLog).toBe(log);
  });

  it("returns honest message when failure log and errorSnippet are absent", async () => {
    const store = useCiStore.getState();
    store.simulateFailure({
      errorSnippet: undefined,
    });

    const log = await store.fetchFailureLog(999, "/dummy");
    expect(log).toBe(
      "No CI failure logs available. Please ensure the 'gh' CLI is installed and authenticated ('gh auth status'), and that workflow runs exist for this repository.",
    );
    expect(useCiStore.getState().failureLog).toBe(log);
  });

  it("dispatches kronos:dispatch-composer by default without insert duplicate", async () => {
    const store = useCiStore.getState();
    store.simulateFailure({
      workflowName: "ci.yml",
      branch: "main",
      runId: 100,
      errorSnippet: "Build failed",
    });

    const dispatchEvent = vi.fn();
    const originalWindow = globalThis.window;
    // @ts-expect-error mock window for node environment
    globalThis.window = { dispatchEvent };

    try {
      await store.repairWithAgent();
      expect(dispatchEvent).toHaveBeenCalledTimes(1);
      const event = dispatchEvent.mock.calls[0][0];
      expect(event.type).toBe("kronos:dispatch-composer");
      expect(event.detail.text).toContain("CI Failure Auto-Repair Request");
    } finally {
      globalThis.window = originalWindow;
    }
  });

  it("dispatches kronos:insert-composer when dispatchMode is insert", async () => {
    const store = useCiStore.getState();
    store.simulateFailure({
      workflowName: "ci.yml",
      branch: "main",
      runId: 100,
      errorSnippet: "Build failed",
    });

    const dispatchEvent = vi.fn();
    const originalWindow = globalThis.window;
    // @ts-expect-error mock window for node environment
    globalThis.window = { dispatchEvent };

    try {
      await store.repairWithAgent({ dispatchMode: "insert" });
      expect(dispatchEvent).toHaveBeenCalledTimes(1);
      const event = dispatchEvent.mock.calls[0][0];
      expect(event.type).toBe("kronos:insert-composer");
      expect(event.detail.text).toContain("CI Failure Auto-Repair Request");
    } finally {
      globalThis.window = originalWindow;
    }
  });

  it("formats repair prompt and triggers composer callback", async () => {
    const store = useCiStore.getState();
    store.simulateFailure({
      workflowName: "ci-pipeline.yml",
      branch: "fix/jwt-auth",
      runId: 4210,
      failedJobName: "auth-unit",
      errorSnippet: "TypeError: token is undefined",
    });

    const onPopulate = vi.fn();
    const prompt = await store.repairWithAgent({ onPopulateComposer: onPopulate });

    expect(prompt).toContain("CI Failure Auto-Repair Request");
    expect(prompt).toContain("**Workflow**: ci-pipeline.yml");
    expect(prompt).toContain("**Branch**: fix/jwt-auth");
    expect(prompt).toContain("Run #4210");
    expect(prompt).toContain("TypeError: token is undefined");
    expect(prompt).toContain("Agent Instructions");

    expect(onPopulate).toHaveBeenCalledWith(prompt);
    expect(useCiStore.getState().isRepairing).toBe(false);
    expect(useCiStore.getState().isDismissed).toBe(true);
  });
});
