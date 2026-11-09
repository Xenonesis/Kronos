import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  FlatList,
  Modal,
  Platform,
  Pressable,
  ScrollView,
  Text,
  TextInput,
  View,
} from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import {
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  Clock,
  Code,
  Cpu,
  ExternalLink,
  Folder,
  Layers,
  MoreHorizontal,
  RotateCw,
  Search,
  Sparkles,
  Terminal,
  X,
} from "lucide-react-native";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { useTimeAgo } from "@/hooks/use-time-ago";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import type { Theme } from "@/styles/theme";

export type HistoryScope = "workspace" | "project" | "all";
export type HistorySort = "last_updated" | "messages" | "newest";

export interface UniversalIndexedSessionItem {
  id: string;
  provider: string; // "antigravity" | "omp" | "claude" | "codex" | "opencode" | "pi" | "kronos"
  providerId?: string;
  provider_id?: string;
  project_name: string;
  projectName?: string;
  directory?: string | null;
  cwd?: string | null;
  title: string;
  preview?: string | null;
  last_message_preview?: string | null;
  lastMessagePreview?: string | null;
  message_count: number;
  messageCount?: number;
  subagent_count: number;
  subagentCount?: number;
  model?: string | null;
  created_at?: string | null;
  updated_at?: string | null;
  last_activity_at?: string | null;
  lastActivityAt?: string | null;
  raw_path?: string | null;
}

export interface UniversalSessionHistoryProps {
  currentProjectCwd?: string;
  onSelectResumeSession?: (session: UniversalIndexedSessionItem) => void;
}

const ThemedBot = withUnistyles(Bot);
const ThemedSparkles = withUnistyles(Sparkles);
const ThemedTerminal = withUnistyles(Terminal);
const ThemedCode = withUnistyles(Code);
const ThemedCpu = withUnistyles(Cpu);
const ThemedLayers = withUnistyles(Layers);
const ThemedRotateCw = withUnistyles(RotateCw);
const ThemedLoadingSpinner = withUnistyles(LoadingSpinner);

function getProviderTheme(provider: string): {
  name: string;
  color: string;
  bg: string;
} {
  switch (provider.toLowerCase()) {
    case "antigravity":
      return { name: "Antigravity", color: "#38bdf8", bg: "rgba(56, 189, 248, 0.12)" };
    case "omp":
      return { name: "OMP", color: "#a855f7", bg: "rgba(168, 85, 247, 0.12)" };
    case "claude":
    case "claudecode":
      return { name: "Claude Code", color: "#f59e0b", bg: "rgba(245, 158, 11, 0.12)" };
    case "codex":
      return { name: "Codex", color: "#10b981", bg: "rgba(16, 185, 129, 0.12)" };
    case "opencode":
      return { name: "OpenCode", color: "#06b6d4", bg: "rgba(6, 182, 212, 0.12)" };
    case "pi":
      return { name: "Pi", color: "#ec4899", bg: "rgba(236, 72, 153, 0.12)" };
    default:
      return { name: "Kronos", color: "#6366f1", bg: "rgba(99, 102, 241, 0.12)" };
  }
}

function ProviderGlyph({ provider, color }: { provider: string; color: string }) {
  switch (provider.toLowerCase()) {
    case "antigravity":
      return <ThemedSparkles size={11} uniProps={() => ({ color })} />;
    case "omp":
      return <ThemedBot size={11} uniProps={() => ({ color })} />;
    case "claude":
    case "claudecode":
      return <ThemedSparkles size={11} uniProps={() => ({ color })} />;
    case "codex":
      return <ThemedTerminal size={11} uniProps={() => ({ color })} />;
    case "opencode":
      return <ThemedCode size={11} uniProps={() => ({ color })} />;
    case "pi":
      return <ThemedCpu size={11} uniProps={() => ({ color })} />;
    default:
      return <ThemedLayers size={11} uniProps={() => ({ color })} />;
  }
}

function SessionItemCard({
  item,
  isExpanded,
  onToggleExpand,
  onOpenDetails,
}: {
  item: UniversalIndexedSessionItem;
  isExpanded: boolean;
  onToggleExpand: () => void;
  onOpenDetails: () => void;
}) {
  const providerMeta = getProviderTheme(item.provider);
  const dateObj = useMemo(() => {
    const raw = item.updated_at || item.created_at;
    return raw ? new Date(raw) : null;
  }, [item.updated_at, item.created_at]);
  const timeAgo = useTimeAgo(dateObj);

  return (
    <Pressable
      style={({ pressed }) => [styles.sessionCard, pressed && styles.sessionCardPressed]}
      onPress={onToggleExpand}
    >
      <View style={styles.cardHeaderRow}>
        <Text style={styles.cardTitle} numberOfLines={isExpanded ? undefined : 1}>
          {item.title || item.id}
        </Text>
        <View style={styles.cardActionsRow}>
          <Pressable hitSlop={8} onPress={onToggleExpand} style={styles.chevronButton}>
            {isExpanded ? (
              <ChevronDown size={14} color="#94a3b8" />
            ) : (
              <ChevronRight size={14} color="#64748b" />
            )}
          </Pressable>
          <Pressable hitSlop={8} onPress={onOpenDetails} style={styles.moreButton}>
            <MoreHorizontal size={14} color="#64748b" />
          </Pressable>
        </View>
      </View>

      {item.preview ? (
        <Text style={styles.cardPreview} numberOfLines={isExpanded ? undefined : 2}>
          {item.preview}
        </Text>
      ) : null}

      <View style={styles.cardBadgeRow}>
        <View style={[styles.providerPill, { backgroundColor: providerMeta.bg }]}>
          <ProviderGlyph provider={item.provider} color={providerMeta.color} />
          <Text style={[styles.providerText, { color: providerMeta.color }]}>
            {providerMeta.name}
          </Text>
        </View>

        <Text style={styles.dotSeparator}>·</Text>
        <Text style={styles.metaStatText}>{item.message_count} msgs</Text>

        {item.subagent_count > 0 ? (
          <>
            <Text style={styles.dotSeparator}>·</Text>
            <Text style={styles.metaStatText}>{item.subagent_count} subagents</Text>
          </>
        ) : null}

        {timeAgo ? (
          <>
            <Text style={styles.dotSeparator}>·</Text>
            <Text style={styles.metaStatText}>{timeAgo}</Text>
          </>
        ) : null}

        {item.model ? (
          <>
            <Text style={styles.dotSeparator}>·</Text>
            <Text style={styles.modelTag} numberOfLines={1}>
              {item.model}
            </Text>
          </>
        ) : null}
      </View>
    </Pressable>
  );
}

export function UniversalSessionHistory({
  currentProjectCwd,
  onSelectResumeSession,
}: UniversalSessionHistoryProps) {
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
      if (debouncedSearch.trim()) {
        url.searchParams.set("search", debouncedSearch.trim());
        url.searchParams.set("query", debouncedSearch.trim());
      }
      url.searchParams.set("sort", sort);
      url.searchParams.set("limit", String(limit));
      if (currentProjectCwd) {
        url.searchParams.set("cwd", currentProjectCwd);
      }

      const res = await fetch(url.toString());
      if (res.ok) {
        const data = (await res.json()) as Record<string, any>;
        if (data.ok && Array.isArray(data.sessions)) {
          const normalized: UniversalIndexedSessionItem[] = data.sessions.map((raw: any) => ({
            id: String(raw.id || ""),
            provider: String(raw.providerId || raw.provider_id || raw.provider || "kronos"),
            project_name: String(raw.projectName || raw.project_name || "general"),
            directory: raw.cwd || raw.directory || null,
            title: String(raw.title || raw.id || "Session"),
            preview: raw.lastMessagePreview || raw.last_message_preview || raw.preview || "",
            message_count: Number(raw.messageCount ?? raw.message_count ?? 1),
            subagent_count: Number(raw.subagentCount ?? raw.subagent_count ?? 0),
            model: raw.model || null,
            created_at:
              raw.createdAt || raw.created_at || raw.lastActivityAt || raw.last_activity_at || null,
            updated_at:
              raw.lastActivityAt || raw.last_activity_at || raw.updatedAt || raw.updated_at || null,
            raw_path: raw.rawPath || raw.raw_path || null,
          }));
          setSessions(normalized);
          const scanned =
            data.scannedFilesCount ?? data.scanned_files ?? data.scanned_files_count ?? 0;
          const total = data.totalSessions ?? data.total_sessions ?? normalized.length;
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

  // Group sessions by project name
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

  const handleCycleSort = useCallback(() => {
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

  const handleShowMore = useCallback(() => {
    setLimit((prev) => prev + 50);
  }, []);

  return (
    <View style={styles.container}>
      {/* Header section matching mockup */}
      <View style={styles.header}>
        <View style={styles.headerTitleRow}>
          <View>
            <Text style={styles.headerTitle}>Agent Session History</Text>
            <Text style={styles.headerSubtitle}>Indexed history</Text>
          </View>
          <View style={styles.headerActions}>
            <Pressable style={styles.iconButton} onPress={handleCycleSort} hitSlop={6}>
              <Text style={styles.sortToggleText}>{sortLabel} ∨</Text>
            </Pressable>
            <Pressable style={styles.iconButton} onPress={fetchHistory} hitSlop={6}>
              <ThemedRotateCw
                size={14}
                uniProps={() => ({ color: isLoading ? "#38bdf8" : "#94a3b8" })}
              />
            </Pressable>
          </View>
        </View>

        {/* Scope selector tabs: [ Workspace | Project | All ] */}
        <View style={styles.scopeTabs}>
          {(["workspace", "project", "all"] as const).map((mode) => (
            <Pressable
              key={mode}
              style={[styles.scopeTabItem, scope === mode && styles.scopeTabItemActive]}
              onPress={() => setScope(mode)}
            >
              <Text style={[styles.scopeTabText, scope === mode && styles.scopeTabTextActive]}>
                {mode.charAt(0).toUpperCase() + mode.slice(1)}
              </Text>
            </Pressable>
          ))}
        </View>

        {/* Search Input field */}
        <View style={styles.searchRow}>
          <Search size={14} color="#64748b" style={styles.searchIcon} />
          <TextInput
            value={searchInput}
            onChangeText={setSearchInput}
            placeholder="Search sessions"
            placeholderTextColor="#64748b"
            style={styles.searchInput}
          />
          {searchInput.length > 0 ? (
            <Pressable onPress={() => setSearchInput("")} hitSlop={6}>
              <X size={14} color="#94a3b8" />
            </Pressable>
          ) : null}
        </View>

        {/* Telemetry and stats rows */}
        <View style={styles.telemetryRow}>
          <View style={styles.telemetryLeft}>
            <Check size={13} color="#22c55e" />
            <Text style={styles.telemetryText}>
              Scanned <Text style={styles.boldText}>{scannedFiles.toLocaleString()}</Text> files
            </Text>
          </View>
          <Text style={styles.telemetryRight}>Just now ∨</Text>
        </View>

        <View style={styles.sessionCountRow}>
          <Text style={styles.sessionCountText}>
            <Text style={styles.boldText}>{sessions.length}</Text> of{" "}
            <Text style={styles.boldText}>{totalSessions}</Text> sessions
          </Text>
          <Pressable onPress={handleCycleSort}>
            <Text style={styles.sortSelectorText}>{sortLabel} ∨</Text>
          </Pressable>
        </View>
      </View>

      {/* Sessions list grouped by project */}
      <ScrollView style={styles.listContainer} contentContainerStyle={styles.listContent}>
        {isLoading && sessions.length === 0 ? (
          <View style={styles.loadingContainer}>
            <ThemedLoadingSpinner size="small" uniProps={() => ({ color: "#94a3b8" })} />
            <Text style={styles.loadingText}>Indexing sessions across providers...</Text>
          </View>
        ) : null}

        {!isLoading && sessions.length === 0 ? (
          <View style={styles.emptyContainer}>
            <Layers size={36} color="#475569" />
            <Text style={styles.emptyTitle}>No sessions found</Text>
            <Text style={styles.emptySubtitle}>
              {debouncedSearch
                ? `No sessions matched "${debouncedSearch}"`
                : "No agent transcripts indexed in this scope."}
            </Text>
            <Pressable style={styles.retryButton} onPress={fetchHistory}>
              <Text style={styles.retryButtonText}>Refresh</Text>
            </Pressable>
          </View>
        ) : null}

        {groupedProjects.map((group) => {
          const isCollapsed = collapsedProjects.has(group.name);
          return (
            <View key={group.name} style={styles.projectGroup}>
              {/* Accordion project header */}
              <Pressable
                style={styles.projectHeader}
                onPress={() => toggleProjectCollapse(group.name)}
              >
                <View style={styles.projectHeaderLeft}>
                  {isCollapsed ? (
                    <ChevronRight size={15} color="#94a3b8" />
                  ) : (
                    <ChevronDown size={15} color="#cbd5e1" />
                  )}
                  <Text style={styles.projectHeaderText}>{group.name}</Text>
                </View>
                <View style={styles.projectBadge}>
                  <Text style={styles.projectBadgeText}>{group.count}</Text>
                </View>
              </Pressable>

              {/* Session cards */}
              {!isCollapsed ? (
                <View style={styles.cardsContainer}>
                  {group.items.map((session) => (
                    <SessionItemCard
                      key={session.id}
                      item={session}
                      isExpanded={expandedSessionIds.has(session.id)}
                      onToggleExpand={() => toggleSessionExpand(session.id)}
                      onOpenDetails={() => setSelectedSessionForModal(session)}
                    />
                  ))}
                </View>
              ) : null}
            </View>
          );
        })}

        {sessions.length < totalSessions ? (
          <Pressable style={styles.showMoreButton} onPress={handleShowMore}>
            <Text style={styles.showMoreButtonText}>Show more sessions</Text>
          </Pressable>
        ) : null}
      </ScrollView>

      {/* Session Details / Inspect Modal */}
      {selectedSessionForModal ? (
        <Modal
          visible
          transparent
          animationType="fade"
          onRequestClose={() => setSelectedSessionForModal(null)}
        >
          <View style={styles.modalOverlay}>
            <View style={styles.modalContent}>
              <View style={styles.modalHeader}>
                <View style={styles.modalTitleCol}>
                  <View style={styles.cardBadgeRow}>
                    <View
                      style={[
                        styles.providerPill,
                        {
                          backgroundColor: getProviderTheme(selectedSessionForModal.provider).bg,
                        },
                      ]}
                    >
                      <ProviderGlyph
                        provider={selectedSessionForModal.provider}
                        color={getProviderTheme(selectedSessionForModal.provider).color}
                      />
                      <Text
                        style={[
                          styles.providerText,
                          {
                            color: getProviderTheme(selectedSessionForModal.provider).color,
                          },
                        ]}
                      >
                        {getProviderTheme(selectedSessionForModal.provider).name}
                      </Text>
                    </View>
                    {selectedSessionForModal.model ? (
                      <Text style={styles.modelTag}>{selectedSessionForModal.model}</Text>
                    ) : null}
                  </View>
                  <Text style={styles.modalTitle} numberOfLines={2}>
                    {selectedSessionForModal.title}
                  </Text>
                </View>
                <Pressable
                  onPress={() => setSelectedSessionForModal(null)}
                  style={styles.closeModalButton}
                >
                  <X size={16} color="#94a3b8" />
                </Pressable>
              </View>

              <ScrollView style={styles.modalBody}>
                {selectedSessionForModal.preview ? (
                  <View style={styles.modalSection}>
                    <Text style={styles.modalSectionLabel}>Last Exchange Preview</Text>
                    <Text style={styles.modalPreviewText}>{selectedSessionForModal.preview}</Text>
                  </View>
                ) : null}

                <View style={styles.modalSection}>
                  <Text style={styles.modalSectionLabel}>Session Information</Text>
                  <View style={styles.modalInfoRow}>
                    <Text style={styles.modalInfoKey}>Messages:</Text>
                    <Text style={styles.modalInfoVal}>{selectedSessionForModal.message_count}</Text>
                  </View>
                  <View style={styles.modalInfoRow}>
                    <Text style={styles.modalInfoKey}>Subagents:</Text>
                    <Text style={styles.modalInfoVal}>
                      {selectedSessionForModal.subagent_count}
                    </Text>
                  </View>
                  {selectedSessionForModal.directory ? (
                    <View style={styles.modalInfoRow}>
                      <Text style={styles.modalInfoKey}>Project Path:</Text>
                      <Text style={styles.modalInfoVal} numberOfLines={1}>
                        {selectedSessionForModal.directory}
                      </Text>
                    </View>
                  ) : null}
                  {selectedSessionForModal.raw_path ? (
                    <View style={styles.modalInfoRow}>
                      <Text style={styles.modalInfoKey}>Transcript File:</Text>
                      <Text style={styles.modalInfoVal} numberOfLines={1}>
                        {selectedSessionForModal.raw_path}
                      </Text>
                    </View>
                  ) : null}
                </View>
              </ScrollView>

              <View style={styles.modalFooter}>
                {onSelectResumeSession ? (
                  <Pressable
                    style={styles.resumeButton}
                    onPress={() => {
                      const s = selectedSessionForModal;
                      setSelectedSessionForModal(null);
                      onSelectResumeSession(s);
                    }}
                  >
                    <ExternalLink size={14} color="#ffffff" />
                    <Text style={styles.resumeButtonText}>Import / Resume in Kronos</Text>
                  </Pressable>
                ) : null}
                <Pressable
                  style={styles.dismissButton}
                  onPress={() => setSelectedSessionForModal(null)}
                >
                  <Text style={styles.dismissButtonText}>Close</Text>
                </Pressable>
              </View>
            </View>
          </View>
        </Modal>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create((theme: Theme) => ({
  container: {
    flex: 1,
    backgroundColor: "#09090b",
  },
  header: {
    paddingHorizontal: 16,
    paddingTop: 14,
    paddingBottom: 10,
    borderBottomWidth: 1,
    borderBottomColor: "rgba(255, 255, 255, 0.06)",
    backgroundColor: "#0d0e12",
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: 12,
  },
  headerTitle: {
    fontSize: 15,
    fontWeight: "700",
    color: "#f8fafc",
    letterSpacing: -0.2,
  },
  headerSubtitle: {
    fontSize: 11,
    color: "#94a3b8",
    marginTop: 1,
  },
  headerActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  iconButton: {
    padding: 6,
    borderRadius: 6,
    backgroundColor: "rgba(255, 255, 255, 0.04)",
  },
  sortToggleText: {
    fontSize: 11,
    color: "#94a3b8",
  },
  scopeTabs: {
    flexDirection: "row",
    backgroundColor: "rgba(255, 255, 255, 0.04)",
    borderRadius: 8,
    padding: 3,
    marginBottom: 12,
  },
  scopeTabItem: {
    flex: 1,
    paddingVertical: 6,
    alignItems: "center",
    borderRadius: 6,
  },
  scopeTabItemActive: {
    backgroundColor: "rgba(255, 255, 255, 0.12)",
    shadowColor: "#000",
    shadowOpacity: 0.2,
    shadowRadius: 2,
  },
  scopeTabText: {
    fontSize: 12,
    fontWeight: "500",
    color: "#94a3b8",
  },
  scopeTabTextActive: {
    color: "#ffffff",
    fontWeight: "600",
  },
  searchRow: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: "rgba(255, 255, 255, 0.04)",
    borderRadius: 8,
    paddingHorizontal: 10,
    paddingVertical: 7,
    borderWidth: 1,
    borderColor: "rgba(255, 255, 255, 0.06)",
    marginBottom: 10,
  },
  searchIcon: {
    marginRight: 8,
  },
  searchInput: {
    flex: 1,
    fontSize: 13,
    color: "#f8fafc",
    padding: 0,
  },
  telemetryRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: 6,
  },
  telemetryLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  telemetryText: {
    fontSize: 11,
    color: "#cbd5e1",
  },
  boldText: {
    fontWeight: "700",
    color: "#f8fafc",
  },
  telemetryRight: {
    fontSize: 11,
    color: "#94a3b8",
  },
  sessionCountRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingTop: 4,
  },
  sessionCountText: {
    fontSize: 11,
    color: "#94a3b8",
  },
  sortSelectorText: {
    fontSize: 11,
    color: "#94a3b8",
  },
  listContainer: {
    flex: 1,
  },
  listContent: {
    padding: 12,
    paddingBottom: 40,
  },
  loadingContainer: {
    padding: 40,
    alignItems: "center",
    justifyContent: "center",
    gap: 12,
  },
  loadingText: {
    fontSize: 12,
    color: "#94a3b8",
  },
  emptyContainer: {
    padding: 48,
    alignItems: "center",
    justifyContent: "center",
    gap: 8,
  },
  emptyTitle: {
    fontSize: 14,
    fontWeight: "600",
    color: "#cbd5e1",
  },
  emptySubtitle: {
    fontSize: 12,
    color: "#64748b",
    textAlign: "center",
  },
  retryButton: {
    marginTop: 10,
    paddingHorizontal: 14,
    paddingVertical: 6,
    borderRadius: 6,
    backgroundColor: "rgba(255, 255, 255, 0.06)",
  },
  retryButtonText: {
    fontSize: 12,
    color: "#cbd5e1",
    fontWeight: "500",
  },
  projectGroup: {
    marginBottom: 14,
  },
  projectHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingVertical: 8,
    paddingHorizontal: 6,
    marginBottom: 4,
  },
  projectHeaderLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  projectHeaderText: {
    fontSize: 13,
    fontWeight: "700",
    color: "#f1f5f9",
  },
  projectBadge: {
    backgroundColor: "rgba(255, 255, 255, 0.08)",
    paddingHorizontal: 8,
    paddingVertical: 2,
    borderRadius: 12,
  },
  projectBadgeText: {
    fontSize: 11,
    fontWeight: "600",
    color: "#cbd5e1",
  },
  cardsContainer: {
    gap: 8,
  },
  sessionCard: {
    backgroundColor: "#13141a",
    borderRadius: 8,
    padding: 12,
    borderWidth: 1,
    borderColor: "rgba(255, 255, 255, 0.05)",
  },
  sessionCardPressed: {
    backgroundColor: "#181a22",
  },
  cardHeaderRow: {
    flexDirection: "row",
    alignItems: "flex-start",
    justifyContent: "space-between",
    marginBottom: 6,
  },
  cardTitle: {
    flex: 1,
    fontSize: 13,
    fontWeight: "700",
    color: "#f8fafc",
    lineHeight: 18,
    marginRight: 8,
  },
  cardActionsRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  chevronButton: {
    padding: 2,
  },
  moreButton: {
    padding: 2,
  },
  cardPreview: {
    fontSize: 12,
    color: "#94a3b8",
    lineHeight: 17,
    marginBottom: 8,
  },
  cardBadgeRow: {
    flexDirection: "row",
    alignItems: "center",
    flexWrap: "wrap",
    gap: 6,
  },
  providerPill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  providerText: {
    fontSize: 11,
    fontWeight: "600",
  },
  dotSeparator: {
    fontSize: 11,
    color: "#475569",
  },
  metaStatText: {
    fontSize: 11,
    color: "#94a3b8",
  },
  modelTag: {
    fontSize: 11,
    color: "#64748b",
  },
  showMoreButton: {
    paddingVertical: 12,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "rgba(255, 255, 255, 0.04)",
    borderRadius: 8,
    marginTop: 10,
  },
  showMoreButtonText: {
    fontSize: 12,
    fontWeight: "600",
    color: "#cbd5e1",
  },
  modalOverlay: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.7)",
    justifyContent: "center",
    alignItems: "center",
    padding: 16,
  },
  modalContent: {
    width: "100%",
    maxWidth: 580,
    maxHeight: "85%",
    backgroundColor: "#111217",
    borderRadius: 12,
    borderWidth: 1,
    borderColor: "rgba(255, 255, 255, 0.1)",
    padding: 16,
  },
  modalHeader: {
    flexDirection: "row",
    alignItems: "flex-start",
    justifyContent: "space-between",
    marginBottom: 14,
    paddingBottom: 12,
    borderBottomWidth: 1,
    borderBottomColor: "rgba(255, 255, 255, 0.06)",
  },
  modalTitleCol: {
    flex: 1,
    gap: 6,
  },
  modalTitle: {
    fontSize: 15,
    fontWeight: "700",
    color: "#ffffff",
  },
  closeModalButton: {
    padding: 4,
  },
  modalBody: {
    flex: 1,
  },
  modalSection: {
    marginBottom: 14,
    backgroundColor: "rgba(255, 255, 255, 0.03)",
    borderRadius: 8,
    padding: 12,
  },
  modalSectionLabel: {
    fontSize: 11,
    fontWeight: "600",
    color: "#94a3b8",
    textTransform: "uppercase",
    letterSpacing: 0.5,
    marginBottom: 6,
  },
  modalPreviewText: {
    fontSize: 12,
    color: "#e2e8f0",
    lineHeight: 18,
  },
  modalInfoRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingVertical: 4,
  },
  modalInfoKey: {
    fontSize: 12,
    color: "#94a3b8",
  },
  modalInfoVal: {
    fontSize: 12,
    fontWeight: "500",
    color: "#f1f5f9",
    maxWidth: "70%",
  },
  modalFooter: {
    flexDirection: "row",
    justifyContent: "flex-end",
    gap: 8,
    paddingTop: 12,
    borderTopWidth: 1,
    borderTopColor: "rgba(255, 255, 255, 0.06)",
  },
  resumeButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    backgroundColor: "#3b82f6",
    paddingHorizontal: 14,
    paddingVertical: 8,
    borderRadius: 6,
  },
  resumeButtonText: {
    fontSize: 12,
    fontWeight: "600",
    color: "#ffffff",
  },
  dismissButton: {
    paddingHorizontal: 14,
    paddingVertical: 8,
    borderRadius: 6,
    backgroundColor: "rgba(255, 255, 255, 0.06)",
  },
  dismissButtonText: {
    fontSize: 12,
    color: "#cbd5e1",
  },
}));
