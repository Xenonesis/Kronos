import { invoke } from "@tauri-apps/api/core";

export interface NativeSearchResult {
  readonly path: string;
  readonly score: number;
}

export async function executeNativeWorkspaceSearch(
  query: string,
  root?: string,
  limit?: number,
): Promise<readonly NativeSearchResult[]> {
  try {
    return await invoke<NativeSearchResult[]>("workspace_search", {
      query,
      root: root ?? null,
      limit: limit ?? null,
    });
  } catch {
    return [];
  }
}
