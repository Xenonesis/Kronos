export interface NativeSearchResult {
  readonly path: string;
  readonly score: number;
}

function isNativeSearchResult(item: unknown): item is NativeSearchResult {
  return (
    typeof item === "object" &&
    item !== null &&
    "path" in item &&
    typeof item.path === "string" &&
    "score" in item &&
    typeof item.score === "number"
  );
}

export async function executeNativeWorkspaceSearch(
  query: string,
  root?: string,
  limit?: number,
): Promise<readonly NativeSearchResult[]> {
  if (typeof window === "undefined" || typeof window.paseoDesktop?.invoke !== "function") {
    return [];
  }

  try {
    const results = await window.paseoDesktop.invoke("workspace_search", {
      query,
      root: root ?? null,
      limit: limit ?? null,
    });
    if (!Array.isArray(results)) {
      return [];
    }
    return results.filter(isNativeSearchResult);
  } catch {
    return [];
  }
}
