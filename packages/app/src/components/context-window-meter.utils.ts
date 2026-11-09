export function formatContextPercentage(value: number, locale?: string): string {
  return new Intl.NumberFormat(locale, {
    style: "percent",
    maximumFractionDigits: 0,
  }).format(value / 100);
}

export function formatTokenCount(value: number): string {
  if (value >= 1_000_000) {
    return `${Math.round(value / 1_000_000)}m`;
  }
  if (value >= 1_000) {
    return `${Math.round(value / 1_000)}k`;
  }
  return Math.round(value).toString();
}

export function shouldShowCompactWarning(percentage: number | null | undefined): boolean {
  if (percentage === null || percentage === undefined || !Number.isFinite(percentage)) {
    return false;
  }
  return percentage >= 75;
}

export interface ModelContextLookupItem {
  id: string;
  aliases?: string[];
  contextWindowMaxTokens?: number;
  contextWindow?: number;
}

/**
 * Resolves the maximum context window in tokens for a model.
 * Looks into the loaded provider model catalog provided dynamically by the runtime.
 * Returns null if the provider/model does not report context capacity (no hardcoding).
 */
export function resolveModelContextWindow(
  modelId: string | null | undefined,
  _provider?: string | null,
  models?: ModelContextLookupItem[] | null,
): number | null {
  if (models && models.length > 0 && modelId) {
    const trimmed = modelId.trim();
    const found =
      models.find((m) => m.id === trimmed || m.aliases?.includes(trimmed)) ??
      models.find(
        (m) =>
          trimmed.endsWith(`/${m.id}`) ||
          m.id.endsWith(`/${trimmed}`) ||
          trimmed.toLowerCase() === m.id.toLowerCase(),
      );
    const candidate =
      found?.contextWindowMaxTokens ?? (found as { contextWindow?: number })?.contextWindow;
    if (typeof candidate === "number" && Number.isFinite(candidate) && candidate > 0) {
      return candidate;
    }
  }

  return null;
}
