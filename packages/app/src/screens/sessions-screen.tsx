import { useMemo, useState, useCallback, useEffect, type ReactNode } from "react";
import { View } from "react-native";
import { useIsFocused } from "expo-router/react-navigation";
import { router } from "expo-router";
import { StyleSheet } from "react-native-unistyles";
import { OmpSessionsView } from "@/desktop/omp";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { MenuHeader } from "@/components/headers/menu-header";
import { ALL_HOSTS_OPTION_ID } from "@/components/hosts/host-picker";
import { useAgentHistory } from "@/hooks/use-agent-history";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { useImportSession } from "@/hooks/use-import-session";
import { useHosts } from "@/runtime/host-runtime";
import { buildOpenProjectRoute } from "@/utils/host-routes";
import { useActiveWorkspaceSelection } from "@/stores/navigation-active-workspace-store";
import { useWorkspaceDirectory } from "@/stores/session-store-hooks";
import { SessionViewModeTabs, type SessionViewMode } from "./sessions/view-mode-tabs";
import { UniversalSessionHistory } from "./sessions/universal-session-history";
import { SessionsListFooter } from "./sessions/list-feedback";
import { SessionsAgentView } from "./sessions/sessions-agent-view";

const SEARCH_DEBOUNCE_MS = 200;

const sessionsHostOptionTestID = (serverId: string) => `sessions-host-filter-item-${serverId}`;

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

  const emptyText = resolveEmptyText({
    t,
    isSearching,
    isAllHosts: selectedHost === ALL_HOSTS_OPTION_ID,
  });
  const showHostFilter = hosts.length > 1;
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

  let activeContent: ReactNode = null;
  if (activeViewMode === "indexed") {
    activeContent = (
      <UniversalSessionHistory
        currentProjectCwd={currentProjectCwd ?? undefined}
        onSelectResumeSession={handleSelectResumeSession}
      />
    );
  } else if (activeViewMode === "omp") {
    activeContent = <OmpSessionsView onSelectResumeSession={handleSelectResumeSession} />;
  } else {
    activeContent = (
      <SessionsAgentView
        t={t}
        hosts={hosts}
        selectedHost={selectedHost}
        onSelectHost={setSelectedHost}
        sessionsHostOptionTestID={sessionsHostOptionTestID}
        showHostFilter={showHostFilter}
        isSearchSupported={isSearchSupported}
        searchInput={searchInput}
        onSearchChange={setSearchInput}
        onClearSearch={handleClearSearch}
        hostErrors={hostErrors}
        isInitialLoad={isInitialLoad}
        showLoadError={showLoadError}
        handleRefresh={handleRefresh}
        agents={agents}
        emptyText={emptyText}
        isSearching={isSearching}
        search={search}
        isManualRefresh={isManualRefresh}
        handleBack={handleBack}
        onImport={importSession.open}
        listFooterComponent={listFooterComponent}
        importSessionSheet={importSession.sheet}
      />
    );
  }

  return (
    <View style={styles.container}>
      <MenuHeader title={t("sessions.title")} />
      <SessionViewModeTabs
        activeViewMode={activeViewMode}
        onSelectViewMode={handleSelectViewMode}
      />
      {activeContent}
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
  },
}));
