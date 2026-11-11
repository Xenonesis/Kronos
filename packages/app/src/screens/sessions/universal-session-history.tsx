import { memo, useCallback } from "react";
import { ScrollView, Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { ChevronDown, ChevronRight, Layers } from "lucide-react-native";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { PressHighlight } from "@/components/ui/press-highlight";
import {
  type HistoryScope,
  type HistorySort,
  type UniversalIndexedSessionItem,
  type UniversalSessionHistoryProps,
} from "./universal/session-types";
import { SessionFilterBar } from "./universal/session-filter-bar";
import { SessionItemCard } from "./universal/session-card";
import { SessionDetailsModal } from "./universal/session-details-modal";
import { useUniversalSessions } from "./universal/use-universal-sessions";

export type {
  HistoryScope,
  HistorySort,
  UniversalIndexedSessionItem,
  UniversalSessionHistoryProps,
};

const ThemedLoadingSpinner = withUnistyles(LoadingSpinner);
const loadingSpinnerProps = () => ({ color: "#94a3b8" });

interface ProjectGroupHeaderProps {
  name: string;
  count: number;
  isCollapsed: boolean;
  onToggle: (name: string) => void;
}

const ProjectGroupHeader = memo(function ProjectGroupHeader({
  name,
  count,
  isCollapsed,
  onToggle,
}: ProjectGroupHeaderProps) {
  const handleToggle = useCallback(() => {
    onToggle(name);
  }, [name, onToggle]);

  return (
    <PressHighlight style={styles.projectHeader} onPress={handleToggle}>
      <View style={styles.projectHeaderLeft}>
        {isCollapsed ? (
          <ChevronRight size={15} color="#94a3b8" />
        ) : (
          <ChevronDown size={15} color="#cbd5e1" />
        )}
        <Text style={styles.projectHeaderText}>{name}</Text>
      </View>
      <View style={styles.projectBadge}>
        <Text style={styles.projectBadgeText}>{count}</Text>
      </View>
    </PressHighlight>
  );
});

export const UniversalSessionHistory = memo(function UniversalSessionHistory({
  currentProjectCwd,
  onSelectResumeSession,
}: UniversalSessionHistoryProps) {
  const {
    scope,
    setScope,
    searchInput,
    setSearchInput,
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
  } = useUniversalSessions(currentProjectCwd);

  const handleOpenDetails = useCallback(
    (session: UniversalIndexedSessionItem) => {
      setSelectedSessionForModal(session);
    },
    [setSelectedSessionForModal],
  );

  return (
    <View style={styles.container}>
      <SessionFilterBar
        scope={scope}
        onSelectScope={setScope}
        searchInput={searchInput}
        onSearchChange={setSearchInput}
        sort={sort}
        sortLabel={sortLabel}
        onCycleSort={cycleSort}
        onRefresh={fetchHistory}
        isLoading={isLoading}
        scannedFiles={scannedFiles}
        sessionCount={sessions.length}
        totalSessions={totalSessions}
      />

      <ScrollView style={styles.listContainer} contentContainerStyle={styles.listContent}>
        {isLoading && sessions.length === 0 ? (
          <View style={styles.loadingContainer}>
            <ThemedLoadingSpinner size="small" uniProps={loadingSpinnerProps} />
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
            <PressHighlight style={styles.retryButton} onPress={fetchHistory}>
              <Text style={styles.retryButtonText}>Refresh</Text>
            </PressHighlight>
          </View>
        ) : null}

        {groupedProjects.map((group) => {
          const isCollapsed = collapsedProjects.has(group.name);
          return (
            <View key={group.name} style={styles.projectGroup}>
              <ProjectGroupHeader
                name={group.name}
                count={group.count}
                isCollapsed={isCollapsed}
                onToggle={toggleProjectCollapse}
              />

              {!isCollapsed ? (
                <View style={styles.cardsContainer}>
                  {group.items.map((session) => (
                    <SessionItemCard
                      key={session.id}
                      item={session}
                      isExpanded={expandedSessionIds.has(session.id)}
                      onToggleExpand={toggleSessionExpand}
                      onOpenDetails={handleOpenDetails}
                    />
                  ))}
                </View>
              ) : null}
            </View>
          );
        })}

        {sessions.length < totalSessions ? (
          <PressHighlight style={styles.showMoreButton} onPress={showMore}>
            <Text style={styles.showMoreButtonText}>Show more sessions</Text>
          </PressHighlight>
        ) : null}
      </ScrollView>

      <SessionDetailsModal
        session={selectedSessionForModal}
        onClose={closeModal}
        onResume={onSelectResumeSession}
      />
    </View>
  );
});

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#09090b",
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
});
