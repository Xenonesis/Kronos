import { memo, type ReactElement, type ReactNode } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import type { TFunction } from "i18next";
import { Button } from "@/components/ui/button";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { AgentList } from "@/components/agent-list";
import { SearchField } from "@/components/ui/search-field";
import { HostFilter } from "@/components/hosts/host-filter";
import type { HostPickerHost } from "@/components/hosts/host-picker";
import type { AggregatedAgent } from "@/hooks/use-aggregated-agents";
import type { AgentHistoryHostError } from "@/hooks/use-agent-history";
import type { Theme } from "@/styles/theme";
import { SessionHostErrorsBanner, SessionsEmptyState } from "./list-feedback";

const ThemedLoadingSpinner = withUnistyles(LoadingSpinner);
const foregroundMutedColorMapping = (theme: Theme) => ({
  color: theme.colors.foregroundMuted,
});

interface SessionsAgentViewProps {
  t: TFunction;
  hosts: HostPickerHost[];
  selectedHost: string;
  onSelectHost: (hostId: string) => void;
  sessionsHostOptionTestID: (serverId: string) => string;
  showHostFilter: boolean;
  isSearchSupported: boolean;
  searchInput: string;
  onSearchChange: (text: string) => void;
  onClearSearch: () => void;
  hostErrors: AgentHistoryHostError[];
  isInitialLoad: boolean;
  showLoadError: boolean;
  handleRefresh: () => void;
  agents: AggregatedAgent[];
  emptyText: string;
  isSearching: boolean;
  search: string;
  isManualRefresh: boolean;
  handleBack: () => void;
  onImport: () => void;
  listFooterComponent: ReactElement | null;
  importSessionSheet: ReactNode;
}

export const SessionsAgentView = memo(function SessionsAgentView({
  t,
  hosts,
  selectedHost,
  onSelectHost,
  sessionsHostOptionTestID,
  showHostFilter,
  isSearchSupported,
  searchInput,
  onSearchChange,
  onClearSearch,
  hostErrors,
  isInitialLoad,
  showLoadError,
  handleRefresh,
  agents,
  emptyText,
  isSearching,
  search,
  isManualRefresh,
  handleBack,
  onImport,
  listFooterComponent,
  importSessionSheet,
}: SessionsAgentViewProps) {
  const showFilterRow = showHostFilter || isSearchSupported;

  return (
    <>
      {showFilterRow ? (
        <View style={styles.filterContainer}>
          {isSearchSupported ? (
            <SearchField
              value={searchInput}
              onChangeText={onSearchChange}
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
              onSelectHost={onSelectHost}
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
          onClearSearch={onClearSearch}
          onBack={handleBack}
          onImport={onImport}
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
      {importSessionSheet}
    </>
  );
});

const styles = StyleSheet.create((theme) => ({
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
