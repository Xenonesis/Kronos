import { useMemo, useState, useCallback, useEffect } from "react";
import { View, Text } from "react-native";
import { useIsFocused } from "expo-router/react-navigation";
import { router } from "expo-router";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { OmpSessionsView } from "@/desktop/omp";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { MenuHeader } from "@/components/headers/menu-header";
import { Button } from "@/components/ui/button";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { AgentList } from "@/components/agent-list";
import { SearchField } from "@/components/ui/search-field";
import { HostFilter } from "@/components/hosts/host-filter";
import { ALL_HOSTS_OPTION_ID } from "@/components/hosts/host-picker";
import { useAgentHistory } from "@/hooks/use-agent-history";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { useImportSession } from "@/hooks/use-import-session";
import { useHosts } from "@/runtime/host-runtime";
import { buildOpenProjectRoute } from "@/utils/host-routes";
import type { Theme } from "@/styles/theme";
import { useActiveWorkspaceSelection } from "@/stores/navigation-active-workspace-store";
import { useWorkspaceDirectory } from "@/stores/session-store-hooks";
import { SessionViewModeTabs, type SessionViewMode } from "./sessions/view-mode-tabs";
import { UniversalSessionHistory } from "./sessions/universal-session-history";
import {
  SessionHostErrorsBanner,
  SessionsEmptyState,
  SessionsListFooter,
} from "./sessions/list-feedback";

const ThemedLoadingSpinner = withUnistyles(LoadingSpinner);
const foregroundMutedColorMapping = (theme: Theme) => ({
  color: theme.colors.foregroundMuted,
});

/** Long enough that a typed word is one request, short enough to feel live. */
const SEARCH_DEBOUNCE_MS = 200;

const sessionsHostOptionTestID = (serverId: string) => `sessions-host-filter-item-${serverId}`;

/** An empty list means something different once a query is narrowing it. */
function resolveEmptyText(input: {
  t: TFunction;
  isSearching: boolean;
  isAllHosts: boolean;
}): string {
  if (input.isSearching) return input.t("sessions.noMatches");
  if (input.isAllHosts) return input.t("sessions.empty");
  return "No sessions for this host";
}

export function SessionsScreen() {
  const isFocused = useIsFocused();

  if (!isFocused) {
    return <View style={styles.container} />;
  }

  return <SessionsScreenContent />;
}

function SessionsScreenContent() {
  const { t } = useTranslation();
  const importSession = useImportSession();
  const { open: openImportSession } = importSession;
  const hosts = useHosts();
  const activeSelection = useActiveWorkspaceSelection();
  const currentProjectCwd = useWorkspaceDirectory(
    activeSelection?.serverId ?? null,
    activeSelection?.workspaceId ?? null,
  );
  const [selectedHost, setSelectedHost] = useState(ALL_HOSTS_OPTION_ID);
  const [activeViewMode, setActiveViewMode] = useState<SessionViewMode>("indexed");
  const [searchInput, setSearchInput] = useState("");
  const search = useDebouncedValue(searchInput, SEARCH_DEBOUNCE_MS).trim();
  const historyServerId = selectedHost === ALL_HOSTS_OPTION_ID ? null : selectedHost;
  const {
    agents,
    hasMore,
    isInitialLoad,
    isLoadingMore,
    isError,
    isSearchSupported,
    isSearchTruncated,
    hostErrors,
    loadMore,
    refreshAll,
  } = useAgentHistory({
    serverId: historyServerId,
    search,
  });
  const isSearching = isSearchSupported && search.length > 0;

  useEffect(() => {
    if (
      selectedHost !== ALL_HOSTS_OPTION_ID &&
      !hosts.some((host) => host.serverId === selectedHost)
    ) {
      setSelectedHost(ALL_HOSTS_OPTION_ID);
    }
  }, [hosts, selectedHost]);

  const [isManualRefresh, setIsManualRefresh] = useState(false);

  const handleRefresh = useCallback(() => {
    setIsManualRefresh(true);
    void refreshAll().finally(() => setIsManualRefresh(false));
  }, [refreshAll]);

  // Searching filters the chronological history without changing its date buckets.
  const emptyText = resolveEmptyText({
    t,
    isSearching,
    isAllHosts: selectedHost === ALL_HOSTS_OPTION_ID,
  });
  const showHostFilter = hosts.length > 1;
  const showFilterRow = showHostFilter || isSearchSupported;
  const showLoadError = isError && agents.length === 0;

  const handleBack = useCallback(() => {
    router.navigate(buildOpenProjectRoute());
  }, []);

  const handleClearSearch = useCallback(() => setSearchInput(""), []);

  const handleSelectViewMode = useCallback((mode: SessionViewMode) => {
    setActiveViewMode(mode);
  }, []);

  const handleSelectResumeSession = useCallback(() => {
    openImportSession();
  }, [openImportSession]);

  const listFooterComponent = useMemo(() => {
    if (!isSearchTruncated && !hasMore) {
      return null;
    }
    return (
      <SessionsListFooter
        hasMore={hasMore}
        isLoadingMore={isLoadingMore}
        isSearchTruncated={isSearchTruncated}
        onLoadMore={loadMore}
        t={t}
      />
    );
  }, [hasMore, isLoadingMore, isSearchTruncated, loadMore, t]);

  return (
    <View style={styles.container}>
      <MenuHeader title={t("sessions.title")} />
      {/* Switcher tabs between All Agent History, Kronos Sessions, & OMP CLI Sessions */}
      <SessionViewModeTabs
        activeViewMode={activeViewMode}
        onSelectViewMode={handleSelectViewMode}
      />
      {activeViewMode === "indexed" ? (
        <UniversalSessionHistory
          currentProjectCwd={currentProjectCwd ?? undefined}
          onSelectResumeSession={handleSelectResumeSession}
        />
      ) : activeViewMode === "omp" ? (
        <OmpSessionsView onSelectResumeSession={handleSelectResumeSession} />
      ) : (
        <>
          {showFilterRow ? (
            <View style={styles.filterContainer}>
              {isSearchSupported ? (
                <SearchField
                  value={searchInput}
                  onChangeText={setSearchInput}
                  placeholder={t("sessions.searchPlaceholder")}
                  clearAccessibilityLabel={t("sessions.actions.clearSearch")}
                  testID="sessions-search-input"
                  clearTestID="sessions-search-clear"
                />
              ) : null}
              {showHostFilter ? (
                <HostFilter
                  hosts={hosts}
                  selectedHost={selectedHost}
                  onSelectHost={setSelectedHost}
                  triggerTestID="sessions-host-filter-trigger"
                  hostOptionTestID={sessionsHostOptionTestID}
                />
              ) : null}
            </View>
          ) : null}
          {hostErrors.length > 0 ? <SessionHostErrorsBanner errors={hostErrors} t={t} /> : null}
          {isInitialLoad ? (
            <View style={styles.loadingContainer}>
              <ThemedLoadingSpinner size="large" uniProps={foregroundMutedColorMapping} />
            </View>
          ) : null}
          {!isInitialLoad && showLoadError ? (
            <View style={styles.emptyContainer}>
              <Text style={styles.emptyText}>Unable to load sessions</Text>
              <Button variant="ghost" onPress={handleRefresh}>
                Try again
              </Button>
            </View>
          ) : null}
          {!isInitialLoad && !showLoadError && agents.length === 0 ? (
            <SessionsEmptyState
              emptyText={emptyText}
              isSearching={isSearching}
              onClearSearch={handleClearSearch}
              onBack={handleBack}
              onImport={importSession.open}
              t={t}
            />
          ) : null}
          {!isInitialLoad && !showLoadError && agents.length > 0 ? (
            <AgentList
              agents={agents}
              showCheckoutInfo={false}
              isRefreshing={isManualRefresh}
              onRefresh={handleRefresh}
              listFooterComponent={listFooterComponent}
              showAttentionIndicator={false}
              showHostColumn
              search={isSearching ? search : undefined}
            />
          ) : null}
          {importSession.sheet}
        </>
      )}
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
  },
  filterContainer: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    paddingHorizontal: {
      xs: theme.spacing[3],
      md: theme.spacing[6],
    },
    paddingTop: theme.spacing[4],
  },
  emptyContainer: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
    gap: theme.spacing[6],
    padding: theme.spacing[6],
  },
  emptyText: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
  loadingContainer: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
  },
}));
