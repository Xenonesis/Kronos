import { describe, it, expect, beforeEach } from "vitest";
import { useCascadeFlowStore } from "./cascade-flow-store";

describe("cascade-flow-store", () => {
  beforeEach(() => {
    useCascadeFlowStore.setState({
      isFlowActive: true,
      activeFile: null,
      cursorLine: 1,
      cursorColumn: 1,
      recentFiles: [],
    });
  });

  it("updates active file and records recent files list", () => {
    const store = useCascadeFlowStore.getState();
    store.setActiveFile("src/App.tsx", 42, 10);

    const state = useCascadeFlowStore.getState();
    expect(state.activeFile).toBe("src/App.tsx");
    expect(state.cursorLine).toBe(42);
    expect(state.cursorColumn).toBe(10);
    expect(state.recentFiles).toEqual(["src/App.tsx"]);

    store.setActiveFile("src/index.css", 5, 1);
    expect(useCascadeFlowStore.getState().recentFiles).toEqual(["src/index.css", "src/App.tsx"]);
  });

  it("generates formatted flow context string", () => {
    const store = useCascadeFlowStore.getState();
    store.setActiveFile("src/components/button.tsx", 18, 4);
    store.setActiveFile("src/App.tsx", 50, 12);

    const context = store.getFlowContextString();
    expect(context).toContain("Active File: App.tsx:50 (col 12)");
    expect(context).toContain("Recent Workspace Tabs: [button.tsx]");
  });

  it("returns null context when flow is disabled", () => {
    const store = useCascadeFlowStore.getState();
    store.setActiveFile("src/App.tsx", 1, 1);
    store.toggleFlow(false);

    expect(store.getFlowContextString()).toBeNull();
  });
});
