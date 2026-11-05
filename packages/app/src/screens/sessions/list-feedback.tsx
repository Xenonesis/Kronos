import type { ReactElement } from "react";
import { View, Text } from "react-native";
import { ChevronLeft, Import } from "lucide-react-native";
import { StyleSheet } from "react-native-unistyles";
import type { TFunction } from "i18next";
import { Button } from "@/components/ui/button";
import type { AgentHistoryHostError } from "@/hooks/use-agent-history";

export interface SessionHostErrorsBannerProps {
  errors: AgentHistoryHostError[];
  t: TFunction;
}

/**
 * A host that failed while others answered. Without this the list silently
 * under-reports, and under a query "No sessions match" becomes a claim the app
 * has no basis for.
 */
export function SessionHostErrorsBanner({ errors, t }: SessionHostErrorsBannerProps): ReactElement {
  return (
    <View style={styles.errorsBannerWrap}>
      <View style={styles.errorsBanner} testID="sessions-host-errors">
        {errors.map((error) => (
          <Text key={error.serverId} style={styles.errorsBannerText}>
            {t("sessions.hostLoadFailed", { host: error.serverName })}
          </Text>
        ))}
      </View>
    </View>
  );
}

export interface SessionsEmptyStateProps {
  emptyText: string;
  isSearching: boolean;
  onClearSearch: () => void;
  onBack: () => void;
  onImport: () => void;
  t: TFunction;
}

/** The empty state a search-narrowed list shows, with the way out of the query. */
export function SessionsEmptyState({
  emptyText,
  isSearching,
  onClearSearch,
  onBack,
  onImport,
  t,
}: SessionsEmptyStateProps): ReactElement {
  return (
    <View style={styles.emptyContainer} testID="sessions-empty">
      <Text style={styles.emptyText}>{emptyText}</Text>
      {isSearching ? (
        <Button variant="ghost" onPress={onClearSearch}>
          {t("sessions.actions.clearSearch")}
        </Button>
      ) : (
        <Button variant="ghost" leftIcon={ChevronLeft} onPress={onBack}>
          Back
        </Button>
      )}
      <Button variant="ghost" leftIcon={Import} onPress={onImport}>
        {t("importSession.title")}
      </Button>
    </View>
  );
}

export interface SessionsListFooterProps {
  hasMore: boolean;
  isLoadingMore: boolean;
  isSearchTruncated: boolean;
  onLoadMore: () => void;
  t: TFunction;
}

export function SessionsListFooter({
  hasMore,
  isLoadingMore,
  isSearchTruncated,
  onLoadMore,
  t,
}: SessionsListFooterProps): ReactElement | null {
  // COMPAT(historyPagination): old daemons return a truncated relevance page.
  // Added in v0.8.0; remove after 2027-03-16 once the daemon floor supports search pagination.
  if (isSearchTruncated) {
    return (
      <View style={styles.footer}>
        <Text style={styles.footerHint}>{t("sessions.tooManyMatches")}</Text>
      </View>
    );
  }
  if (!hasMore) {
    return null;
  }
  return (
    <View style={styles.footer}>
      <Button variant="ghost" onPress={onLoadMore} disabled={isLoadingMore}>
        {isLoadingMore ? "Loading..." : t("sessions.actions.loadMore")}
      </Button>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
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
  footer: {
    alignItems: "center",
    paddingVertical: theme.spacing[4],
  },
  footerHint: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
  errorsBannerWrap: {
    paddingHorizontal: {
      xs: theme.spacing[3],
      md: theme.spacing[6],
    },
    paddingTop: theme.spacing[3],
  },
  errorsBanner: {
    borderWidth: theme.borderWidth[1],
    borderColor: theme.colors.border,
    borderRadius: theme.borderRadius.lg,
    padding: theme.spacing[3],
    gap: theme.spacing[1],
  },
  errorsBannerText: {
    color: theme.colors.palette.red[300],
    fontSize: theme.fontSize.sm,
  },
}));
