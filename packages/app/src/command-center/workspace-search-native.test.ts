import { beforeEach, describe, expect, it, vi } from "vitest";
import { executeNativeWorkspaceSearch } from "./workspace-search-native";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

describe("executeNativeWorkspaceSearch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("invokes workspace_search with query, root, and limit", async () => {
    const mockResults = [
      { path: "src/main.rs", score: 0.95 },
      { path: "src/search.rs", score: 0.85 },
    ];
    vi.mocked(invoke).mockResolvedValueOnce(mockResults);

    const results = await executeNativeWorkspaceSearch("search", "src", 50);

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("workspace_search", {
      query: "search",
      root: "src",
      limit: 50,
    });
    expect(results).toEqual(mockResults);
  });

  it("defaults root and limit to null when omitted", async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]);

    const results = await executeNativeWorkspaceSearch("foo");

    expect(invoke).toHaveBeenCalledWith("workspace_search", {
      query: "foo",
      root: null,
      limit: null,
    });
    expect(results).toEqual([]);
  });

  it("catches invocation failures and returns an empty array", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(new Error("IPC failure"));

    const results = await executeNativeWorkspaceSearch("fail");

    expect(results).toEqual([]);
  });
});
