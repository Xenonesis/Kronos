import { describe, it, expect, beforeEach } from "vitest";
import { useQuickComposerStore } from "./use-quick-composer";
import { AGENT_HARNESS_OPTIONS } from "./types";

describe("useQuickComposerStore", () => {
  beforeEach(() => {
    useQuickComposerStore.getState().reset();
    useQuickComposerStore.setState({
      isOpen: false,
      target: "workspace",
      selectedAgent: "claude",
      prompt: "",
      isDispatching: false,
    });
  });

  it("has correct initial state and agent harness catalog", () => {
    const state = useQuickComposerStore.getState();
    expect(state.isOpen).toBe(false);
    expect(state.target).toBe("workspace");
    expect(state.selectedAgent).toBe("claude");
    expect(state.prompt).toBe("");

    expect(AGENT_HARNESS_OPTIONS.map((a) => a.id)).toEqual([
      "claude",
      "opencode",
      "codex",
      "pi",
      "omp",
    ]);
  });

  it("opens, closes, and toggles with options", () => {
    const store = useQuickComposerStore.getState();

    store.open({ initialPrompt: "Fix styling bug", agent: "opencode", target: "general" });
    let state = useQuickComposerStore.getState();
    expect(state.isOpen).toBe(true);
    expect(state.prompt).toBe("Fix styling bug");
    expect(state.selectedAgent).toBe("opencode");
    expect(state.target).toBe("general");

    store.close();
    expect(useQuickComposerStore.getState().isOpen).toBe(false);

    store.toggle();
    expect(useQuickComposerStore.getState().isOpen).toBe(true);
  });

  it("updates target, agent, prompt, and dispatching state", () => {
    const store = useQuickComposerStore.getState();

    store.setTarget("general");
    expect(useQuickComposerStore.getState().target).toBe("general");

    store.setSelectedAgent("codex");
    expect(useQuickComposerStore.getState().selectedAgent).toBe("codex");

    store.setPrompt("Write unit tests");
    expect(useQuickComposerStore.getState().prompt).toBe("Write unit tests");

    store.setIsDispatching(true);
    expect(useQuickComposerStore.getState().isDispatching).toBe(true);

    store.reset();
    expect(useQuickComposerStore.getState().prompt).toBe("");
    expect(useQuickComposerStore.getState().isDispatching).toBe(false);
  });
});
