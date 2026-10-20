import React, { useCallback, useEffect, useState } from "react";
import { FlatList, Platform, Pressable, Text, View } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import {
  Bot,
  Calendar,
  Clock,
  Compass,
  FileCode,
  Folder,
  Layers,
  Play,
  RotateCw,
  Search,
  Sparkles,
  Terminal,
} from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { OmpSessionInspectorModal, type OmpSessionDetailData } from "./omp-session-inspector-modal";

export interface OmpSessionItem {
  id: string;
  filePath: string;
  cwd: string;
  title: string | null;
  firstPrompt: string | null;
  lastPrompt: string | null;
  lastActivityAt: string;
  model: string | null;
  thinkingOptionId: string | null;
  turnCount: number;
  toolCallCount: number;
}

export interface OmpSessionsViewProps {
  currentProjectCwd?: string;
  onSelectResumeSession?: (session: OmpSessionItem) => void;
}

export function OmpSessionsView({
  currentProjectCwd,
  onSelectResumeSession,
}: OmpSessionsViewProps) {
  const { theme } = useUnistyles();
  const [sessions, setSessions] = useState<OmpSessionItem[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [filterProjectOnly, setFilterProjectOnly] = useState(Boolean(currentProjectCwd));
  const [selectedSessionForInspect, setSelectedSessionForInspect] =
    useState<OmpSessionDetailData | null>(null);
  const [isInspectorOpen, setIsInspectorOpen] = useState(false);
  const [inspectLoading, setInspectLoading] = useState(false);

  const fetchSessions = useCallback(async () => {
    setIsLoading(true);
    try {
      const url = new URL("http://127.0.0.1:6767/api/omp-sessions");
      if (filterProjectOnly && currentProjectCwd) {
        url.searchParams.set("cwd", currentProjectCwd);
      }
      url.searchParams.set("limit", "100");

      const res = await fetch(url.toString());
      if (res.ok) {
        const data = (await res.json()) as { ok: boolean; sessions?: OmpSessionItem[] };
        if (data.ok && Array.isArray(data.sessions)) {
          setSessions(data.sessions);
        }
      }
    } catch {
      // offline / daemon connecting
    } finally {
      setIsLoading(false);
    }
  }, [filterProjectOnly, currentProjectCwd]);

  useEffect(() => {
    void fetchSessions();
  }, [fetchSessions]);

  const handleOpenInspector = useCallback(async (session: OmpSessionItem) => {
    setInspectLoading(true);
    try {
      const res = await fetch(
        `http://127.0.0.1:6767/api/omp-sessions/${encodeURIComponent(session.id)}`,
      );
      if (res.ok) {
        const data = (await res.json()) as { ok: boolean; session?: OmpSessionDetailData };
        if (data.ok && data.session) {
          setSelectedSessionForInspect(data.session);
          setIsInspectorOpen(true);
        }
      }
    } catch {
      // failed to fetch session details
    } finally {
      setInspectLoading(false);
    }
  }, []);

  const handleResume = useCallback(
    (detail: OmpSessionDetailData) => {
      setIsInspectorOpen(false);
      const match = sessions.find((s) => s.id === detail.id);
      if (match && onSelectResumeSession) {
        onSelectResumeSession(match);
      }
    },
    [sessions, onSelectResumeSession],
  );

  const renderSessionItem = ({ item }: { item: OmpSessionItem }) => {
    const timeFormatted = new Date(item.lastActivityAt).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });

    return (
      <View style={styles.card}>
        <View style={styles.cardHeader}>
          <View style={styles.cardTitleCol}>
            <View style={styles.metaBadgeRow}>
              <View style={styles.ompPill}>
                <Bot size={11} color="#10b981" />
                <Text style={styles.ompPillText}>OMP</Text>
              </View>
              {item.model ? (
                <View style={styles.modelPill}>
                  <Sparkles size={11} color={theme.colors.foregroundMuted} />
                  <Text style={styles.modelPillText}>{item.model}</Text>
                </View>
              ) : null}
            </View>
            <Text style={styles.cardTitle} numberOfLines={1}>
              {item.title || item.id}
            </Text>
          </View>
          <View style={styles.dateCol}>
            <Clock size={12} color={theme.colors.foregroundMuted} />
            <Text style={styles.dateText}>{timeFormatted}</Text>
          </View>
        </View>

        {/* Directory Path */}
        <View style={styles.pathRow}>
          <Folder size={12} color={theme.colors.foregroundMuted} />
          <Text style={styles.pathText} numberOfLines={1}>
            {item.cwd}
          </Text>
        </View>

        {/* Prompt Preview */}
        {item.firstPrompt ? (
          <Text style={styles.promptPreview} numberOfLines={2}>
            {item.firstPrompt}
          </Text>
        ) : null}

        {/* Stats & Actions */}
        <View style={styles.cardFooter}>
          <View style={styles.statsRow}>
            <View style={styles.statItem}>
              <Terminal size={12} color={theme.colors.foregroundMuted} />
              <Text style={styles.statText}>{item.turnCount} turns</Text>
            </View>
            <View style={styles.statItem}>
              <FileCode size={12} color={theme.colors.foregroundMuted} />
              <Text style={styles.statText}>{item.toolCallCount} tool calls</Text>
            </View>
          </View>

          <View style={styles.actionsRow}>
            <Button
              variant="outline"
              size="xs"
              onPress={() => void handleOpenInspector(item)}
              testID={`inspect-session-${item.id}`}
            >
              <Text style={styles.inspectBtnText}>Inspect Diff</Text>
            </Button>
            {onSelectResumeSession ? (
              <Button
                variant="default"
                size="xs"
                onPress={() => onSelectResumeSession(item)}
                testID={`resume-session-${item.id}`}
              >
                <Text style={styles.resumeBtnText}>Resume</Text>
              </Button>
            ) : null}
          </View>
        </View>
      </View>
    );
  };

  return (
    <View style={styles.container}>
      {/* Top Filter Bar */}
      <View style={styles.topBar}>
        <View style={styles.topBarLeft}>
          <View style={styles.titleWithIcon}>
            <Bot size={18} color="#10b981" />
            <Text style={styles.pageTitle}>OMP Sessions Explorer</Text>
          </View>
          <Text style={styles.pageSubtitle}>
            Browse, inspect past tool executions & diffs, and resume sessions
          </Text>
        </View>

        <View style={styles.topBarRight}>
          {currentProjectCwd ? (
            <Pressable
              style={[styles.filterToggle, filterProjectOnly && styles.filterToggleActive]}
              onPress={() => setFilterProjectOnly(!filterProjectOnly)}
            >
              <Folder
                size={13}
                color={filterProjectOnly ? "#ffffff" : theme.colors.foregroundMuted}
              />
              <Text
                style={[
                  styles.filterToggleText,
                  filterProjectOnly && styles.filterToggleTextActive,
                ]}
              >
                This Project Only
              </Text>
            </Pressable>
          ) : null}

          <Button
            variant="ghost"
            size="xs"
            onPress={() => void fetchSessions()}
            accessibilityLabel="Refresh sessions"
          >
            <RotateCw size={14} color={theme.colors.foregroundMuted} />
          </Button>
        </View>
      </View>

      {/* Main Sessions List */}
      {isLoading ? (
        <View style={styles.loadingContainer}>
          <LoadingSpinner color={theme.colors.foregroundMuted} />
          <Text style={styles.loadingText}>Loading OMP sessions...</Text>
        </View>
      ) : sessions.length === 0 ? (
        <View style={styles.emptyContainer}>
          <Bot size={36} color={theme.colors.foregroundMuted} />
          <Text style={styles.emptyTitle}>No OMP Sessions Found</Text>
          <Text style={styles.emptySubtitle}>
            {filterProjectOnly
              ? "No past OMP sessions found for this directory. Try turning off 'This Project Only'."
              : "No past sessions found in ~/.omp/agent/sessions."}
          </Text>
        </View>
      ) : (
        <FlatList
          data={sessions}
          keyExtractor={(item) => item.id}
          renderItem={renderSessionItem}
          contentContainerStyle={styles.listContent}
        />
      )}

      {/* Inspect Session Modal */}
      <OmpSessionInspectorModal
        visible={isInspectorOpen}
        session={selectedSessionForInspect}
        onClose={() => setIsInspectorOpen(false)}
        onResume={handleResume}
      />
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    backgroundColor: theme.colors.surface1,
  },
  topBar: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface2,
  },
  topBarLeft: {
    flex: 1,
  },
  titleWithIcon: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  pageTitle: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.bold,
    color: theme.colors.foreground,
  },
  pageSubtitle: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    marginTop: 2,
  },
  topBarRight: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  filterToggle: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[1.5],
    borderRadius: theme.borderRadius.full,
    backgroundColor: theme.colors.surface3,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  filterToggleActive: {
    backgroundColor: theme.colors.primary,
    borderColor: theme.colors.primary,
  },
  filterToggleText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    fontWeight: theme.fontWeight.medium,
  },
  filterToggleTextActive: {
    color: "#ffffff",
    fontWeight: theme.fontWeight.semibold,
  },
  listContent: {
    padding: theme.spacing[4],
    gap: theme.spacing[3],
  },
  card: {
    backgroundColor: theme.colors.surface2,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[3],
    gap: theme.spacing[2],
  },
  cardHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "flex-start",
  },
  cardTitleCol: {
    flex: 1,
    marginRight: theme.spacing[2],
  },
  metaBadgeRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    marginBottom: 4,
  },
  ompPill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: "rgba(16, 185, 129, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: theme.borderRadius.full,
  },
  ompPillText: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.bold,
    color: "#10b981",
  },
  modelPill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: theme.colors.surface3,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: theme.borderRadius.full,
  },
  modelPillText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  cardTitle: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.semibold,
    color: theme.colors.foreground,
  },
  dateCol: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  dateText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  pathRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  pathText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    fontFamily: Platform.OS === "web" ? "monospace" : undefined,
  },
  promptPreview: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    lineHeight: 18,
    backgroundColor: theme.colors.surface1,
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.md,
  },
  cardFooter: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginTop: 4,
    paddingTop: theme.spacing[2],
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  statsRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[3],
  },
  statItem: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  statText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  actionsRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  inspectBtnText: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.medium,
  },
  resumeBtnText: {
    color: "#ffffff",
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.semibold,
  },
  loadingContainer: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  loadingText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  emptyContainer: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[8],
    gap: theme.spacing[2],
  },
  emptyTitle: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.bold,
    color: theme.colors.foreground,
  },
  emptySubtitle: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    textAlign: "center",
    maxWidth: 380,
  },
}));
