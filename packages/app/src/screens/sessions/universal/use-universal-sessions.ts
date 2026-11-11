import { useCallback, useEffect, useMemo, useState } from "react";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import type { HistoryScope, HistorySort, UniversalIndexedSessionItem } from "./session-types";

function pickString(candidates: unknown[], fallback = ""): string {
  for (const c of candidates) {
    if (typeof c === "string" && c.length > 0) return c;
  }
  return fallback;
}

function pickNullableString(candidates: unknown[]): string | null {
  for (const c of candidates) {
    if (typeof c === "string" && c.length > 0) return c;
  }
  return null;
}

function pickNumber(candidates: unknown[], fallback = 0): number {
  for (const c of candidates) {
    if (typeof c === "number" && !Number.isNaN(c)) return c;
  }
  return fallback;
}

function normalizeSession(raw: Record<string, unknown>): UniversalIndexedSessionItem {
  return {
    id: pickString([raw.id], ""),
    provider: pickString([raw.providerId, raw.provider_id, raw.provider], "kronos"),
    project_name: pickString([raw.projectName, raw.project_name], "general"),
    directory: pickNullableString([raw.cwd, raw.directory]),
    title: pickString([raw.title, raw.id], "Session"),
    preview: pickString([raw.lastMessagePreview, raw.last_message_preview, raw.preview], ""),
    message_count: pickNumber([raw.messageCount, raw.message_count], 1),
    subagent_count: pickNumber([raw.subagentCount, raw.subagent_count], 0),
    model: pickNullableString([raw.model]),
    created_at: pickNullableString([
      raw.createdAt,
      raw.created_at,
      raw.lastActivityAt,
      raw.last_activity_at,
    ]),
    updated_at: pickNullableString([
      raw.lastActivityAt,
      raw.last_activity_at,
      raw.updatedAt,
      raw.updated_at,
    ]),
    raw_path: pickNullableString([raw.rawPath, raw.raw_path]),
  };
}

export function useUniversalSessions(currentProjectCwd?: string) {
  const [scope, setScope] = useState<HistoryScope>(currentProjectCwd ? "workspace" : "all");
  const [searchInput, setSearchInput] = useState("");
  const debouncedSearch = useDebouncedValue(searchInput, 250);
  const [sort, setSort] = useState<HistorySort>("last_updated");
  const [limit, setLimit] = useState(250);

  const [sessions, setSessions] = useState<UniversalIndexedSessionItem[]>([]);
  const [scannedFiles, setScannedFiles] = useState(0);
  const [totalSessions, setTotalSessions] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  const [expandedSessionIds, setExpandedSessionIds] = useState<Set<string>>(new Set());
  const [collapsedProjects, setCollapsedProjects] = useState<Set<string>>(new Set());
  const [selectedSessionForModal, setSelectedSessionForModal] =
    useState<UniversalIndexedSessionItem | null>(null);

  useEffect(() => {
    if (!currentProjectCwd && scope === "workspace") {
      setScope("all");
    }
  }, [currentProjectCwd, scope]);

  const fetchHistory = useCallback(async () => {
    setIsLoading(true);
    try {
      const url = new URL("http://127.0.0.1:6767/api/agent-history/indexed");
      url.searchParams.set("scope", scope);
      const query = debouncedSearch.trim();
      if (query) {
        url.searchParams.set("search", query);
        url.searchParams.set("query", query);
      }
      url.searchParams.set("sort", sort);
      url.searchParams.set("limit", String(limit));
      if (currentProjectCwd) {
        url.searchParams.set("cwd", currentProjectCwd);
      }

      const res = await fetch(url.toString());
      if (res.ok) {
        const data = (await res.json()) as Record<string, unknown>;
        if (data.ok && Array.isArray(data.sessions)) {
          const rawSessions = data.sessions as Record<string, unknown>[];
          const normalized = rawSessions.map(normalizeSession);
          setSessions(normalized);
          const scanned = (data.scannedFilesCount ??
            data.scanned_files ??
            data.scanned_files_count ??
            0) as number;
          const total = (data.totalSessions ?? data.total_sessions ?? normalized.length) as number;
          setScannedFiles(scanned);
          setTotalSessions(total);
        }
      }
    } catch {
      // Offline / daemon starting
    } finally {
      setIsLoading(false);
    }
  }, [scope, debouncedSearch, sort, limit, currentProjectCwd]);

  useEffect(() => {
    void fetchHistory();
  }, [fetchHistory]);

  const toggleSessionExpand = useCallback((id: string) => {
    setExpandedSessionIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const toggleProjectCollapse = useCallback((project: string) => {
    setCollapsedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(project)) next.delete(project);
      else next.add(project);
      return next;
    });
  }, []);

  const groupedProjects = useMemo(() => {
    const map = new Map<string, UniversalIndexedSessionItem[]>();
    for (const s of sessions) {
      const key = s.project_name || "default";
      const existing = map.get(key) || [];
      existing.push(s);
      map.set(key, existing);
    }
    return Array.from(map.entries()).map(([name, items]) => ({
      name,
      items,
      count: items.length,
    }));
  }, [sessions]);

  const cycleSort = useCallback(() => {
    setSort((prev) => {
      if (prev === "last_updated") return "messages";
      if (prev === "messages") return "newest";
      return "last_updated";
    });
  }, []);

  const sortLabel = useMemo(() => {
    switch (sort) {
      case "last_updated":
        return "Last updated";
      case "messages":
        return "Most msgs";
      case "newest":
        return "Newest first";
    }
  }, [sort]);

  const showMore = useCallback(() => {
    setLimit((prev) => prev + 50);
  }, []);

  const clearSearch = useCallback(() => {
    setSearchInput("");
  }, []);

  const closeModal = useCallback(() => {
    setSelectedSessionForModal(null);
  }, []);

  return {
    scope,
    setScope,
    searchInput,
    setSearchInput,
    clearSearch,
    debouncedSearch,
    sort,
    sortLabel,
    cycleSort,
    sessions,
    scannedFiles,
    totalSessions,
    isLoading,
    fetchHistory,
    showMore,
    groupedProjects,
    expandedSessionIds,
    toggleSessionExpand,
    collapsedProjects,
    toggleProjectCollapse,
    selectedSessionForModal,
    setSelectedSessionForModal,
    closeModal,
  };
}
