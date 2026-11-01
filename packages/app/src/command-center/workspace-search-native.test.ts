import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { executeNativeWorkspaceSearch } from "./workspace-search-native";

interface DesktopHostWindow {
  window?: {
    paseoDesktop?: {
      invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
  };
}

const env = globalThis as unknown as DesktopHostWindow;

describe("executeNativeWorkspaceSearch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    delete env.window;
  });

  it("invokes workspace_search with query, root, and limit", async () => {
    const mockResults = [
      { path: "src/main.rs", score: 0.95 },
      { path: "src/search.rs", score: 0.85 },
    ];
    const mockInvoke = vi.fn().mockResolvedValueOnce(mockResults);
    env.window = {
      paseoDesktop: {
        invoke: mockInvoke,
      },
    };

    const results = await executeNativeWorkspaceSearch("search", "src", 50);

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith("workspace_search", {
      query: "search",
      root: "src",
      limit: 50,
    });
    expect(results).toEqual(mockResults);
  });

  it("defaults root and limit to null when omitted", async () => {
    const mockInvoke = vi.fn().mockResolvedValueOnce([]);
    env.window = {
      paseoDesktop: {
        invoke: mockInvoke,
      },
    };

    const results = await executeNativeWorkspaceSearch("foo");

    expect(mockInvoke).toHaveBeenCalledWith("workspace_search", {
      query: "foo",
      root: null,
      limit: null,
    });
    expect(results).toEqual([]);
  });

  it("returns empty array when window is undefined", async () => {
    delete env.window;
    const results = await executeNativeWorkspaceSearch("foo");
    expect(results).toEqual([]);
  });

  it("returns empty array when window.paseoDesktop or invoke is unavailable", async () => {
    env.window = {
      paseoDesktop: undefined,
    };

    const results = await executeNativeWorkspaceSearch("foo");
    expect(results).toEqual([]);
  });

  it("catches invocation failures and returns an empty array", async () => {
    const mockInvoke = vi.fn().mockRejectedValueOnce(new Error("IPC failure"));
    env.window = {
      paseoDesktop: {
        invoke: mockInvoke,
      },
    };

    const results = await executeNativeWorkspaceSearch("fail");

    expect(results).toEqual([]);
  });

  it("filters out malformed entries cleanly", async () => {
    const mockInvoke = vi
      .fn()
      .mockResolvedValueOnce([
        { path: "valid.rs", score: 1.0 },
        { path: 123, score: 0.5 },
        null,
        "not an object",
        { path: "missing-score.rs" },
      ]);
    env.window = {
      paseoDesktop: {
        invoke: mockInvoke,
      },
    };

    const results = await executeNativeWorkspaceSearch("filter");
    expect(results).toEqual([{ path: "valid.rs", score: 1.0 }]);
  });
});
