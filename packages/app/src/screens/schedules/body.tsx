import type { ReactElement } from "react";
import { ScrollView, Text, View } from "react-native";
import { CalendarClock, Plus } from "lucide-react-native";
import { StyleSheet } from "react-native-unistyles";
import { ExternalLink } from "@/components/ui/external-link";
import { HostFilter } from "@/components/hosts/host-filter";
import { SchedulesTable, type ScheduleRowView } from "@/components/schedules/schedules-table";
import { Button } from "@/components/ui/button";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { SegmentedControl } from "@/components/ui/segmented-control";
import type {
  AggregateLoadState,
  AggregatedSchedule,
  ScheduleHostError,
} from "@/hooks/use-schedules";
import type { ScheduleBucket } from "@/schedules/schedule-derivation";
import type { HostProfile } from "@/types/host-connection";
import { resolveSchedulesScreenBodyState } from "../schedules-screen-state";

const STATUS_FILTER_OPTIONS: { value: ScheduleBucket; label: string; testID: string }[] = [
  { value: "runnable", label: "Active", testID: "schedules-filter-active" },
  { value: "ended", label: "Ended", testID: "schedules-filter-ended" },
];

export interface SchedulesScreenBodyProps {
  rows: ScheduleRowView[];
  loadState: AggregateLoadState<AggregatedSchedule>;
  hostErrors: ScheduleHostError[];
  showLoadError: boolean;
  statusFilter: ScheduleBucket;
  onStatusFilterChange: (value: ScheduleBucket) => void;
  showHostFilter: boolean;
  hosts: HostProfile[];
  selectedHost: string;
  onSelectHost: (serverId: string) => void;
  onRetry: () => void;
  onCreate: () => void;
  onEdit: (schedule: AggregatedSchedule) => void;
}

export function SchedulesScreenBody({
  rows,
  loadState,
  hostErrors,
  showLoadError,
  statusFilter,
  onStatusFilterChange,
  showHostFilter,
  hosts,
  selectedHost,
  onSelectHost,
  onRetry,
  onCreate,
  onEdit,
}: SchedulesScreenBodyProps): ReactElement {
  const bodyState = resolveSchedulesScreenBodyState({ loadState, showLoadError });

  if (bodyState.kind === "loading") {
    return (
      <View style={styles.centered}>
        <LoadingSpinner size="large" color={styles.spinner.color} />
      </View>
    );
  }

  if (bodyState.kind === "load-error") {
    return (
      <View style={styles.centered}>
        <Text style={styles.message}>Unable to load schedules</Text>
        <Button variant="ghost" onPress={onRetry} testID="schedules-retry">
          Try again
        </Button>
      </View>
    );
  }

  if (bodyState.kind === "empty") {
    return (
      <View style={styles.centered}>
        {hostErrors.length > 0 ? <ScheduleHostErrorsBanner errors={hostErrors} /> : null}
        <SchedulesEmptyState onCreate={onCreate} testID="schedules-empty" />
      </View>
    );
  }

  let schedulesContent: ReactElement;
  if (rows.length > 0) {
    schedulesContent = <SchedulesTable rows={rows} onEditSchedule={onEdit} />;
  } else if (statusFilter === "ended") {
    schedulesContent = <SchedulesEndedEmptyState />;
  } else {
    schedulesContent = (
      <View style={styles.filterEmpty}>
        <SchedulesEmptyState onCreate={onCreate} testID="schedules-empty" />
      </View>
    );
  }

  return (
    <View style={styles.body}>
      <View style={styles.filterRow}>
        <View style={styles.filterRowControls}>
          {showHostFilter ? (
            <HostFilter
              hosts={hosts}
              selectedHost={selectedHost}
              onSelectHost={onSelectHost}
              triggerTestID="schedules-host-filter-trigger"
            />
          ) : null}
          <SegmentedControl
            size="sm"
            value={statusFilter}
            onValueChange={onStatusFilterChange}
            options={STATUS_FILTER_OPTIONS}
            testID="schedules-status-filter"
          />
        </View>
        <Button
          variant="outline"
          leftIcon={Plus}
          onPress={onCreate}
          size="sm"
          testID="schedules-new"
        >
          New schedule
        </Button>
      </View>
      <ScrollView
        style={styles.scroll}
        contentContainerStyle={styles.scrollContent}
        showsVerticalScrollIndicator={false}
        keyboardShouldPersistTaps="handled"
        testID="schedules-list"
      >
        {hostErrors.length > 0 ? <ScheduleHostErrorsBanner errors={hostErrors} /> : null}
        {schedulesContent}
      </ScrollView>
    </View>
  );
}

interface SchedulesEmptyStateProps {
  onCreate: () => void;
  testID?: string;
}

function SchedulesEmptyState({ onCreate, testID }: SchedulesEmptyStateProps): ReactElement {
  return (
    <View style={styles.emptyState} testID={testID}>
      <CalendarClock size={styles.emptyIcon.width} color={styles.emptyIcon.color} />
      <View style={styles.emptyTextStack}>
        <Text style={styles.emptyTitle}>No active schedules</Text>
        <Text style={styles.emptyDescription}>Schedules run agents on a cadence.</Text>
        <ExternalLink href="https://paseo.sh/docs/schedules" label="See docs" />
      </View>
      <Button variant="outline" leftIcon={Plus} onPress={onCreate} testID="schedules-empty-new">
        New schedule
      </Button>
    </View>
  );
}

function SchedulesEndedEmptyState(): ReactElement {
  return (
    <View style={styles.filterEmpty}>
      <View style={styles.endedEmptyState}>
        <CalendarClock size={styles.emptyIcon.width} color={styles.emptyIcon.color} />
        <Text style={styles.emptyTitle}>No ended schedules</Text>
      </View>
    </View>
  );
}

interface ScheduleHostErrorsBannerProps {
  errors: ScheduleHostError[];
}

function ScheduleHostErrorsBanner({ errors }: ScheduleHostErrorsBannerProps): ReactElement {
  return (
    <View style={styles.errorsBannerWrap}>
      <View style={styles.errorsBanner} testID="schedules-host-errors">
        {errors.map((error) => (
          <Text key={error.serverId} style={styles.errorsBannerText}>
            {`${error.serverName}: Could not load schedules`}
          </Text>
        ))}
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  body: {
    flex: 1,
    minHeight: 0,
  },
  centered: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
    gap: theme.spacing[6],
    padding: theme.spacing[6],
  },
  filterRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    gap: theme.spacing[3],
    paddingHorizontal: { xs: theme.spacing[3], md: theme.spacing[6] },
    paddingTop: theme.spacing[4],
  },
  filterRowControls: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[3],
    flexShrink: 1,
    flexWrap: "wrap",
  },
  scroll: {
    flex: 1,
    minHeight: 0,
  },
  scrollContent: {
    flexGrow: 1,
    gap: theme.spacing[3],
    paddingTop: theme.spacing[4],
    paddingBottom: theme.spacing[6],
  },
  errorsBannerWrap: {
    paddingHorizontal: { xs: theme.spacing[3], md: theme.spacing[6] },
  },
  errorsBanner: {
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.borderRadius.lg,
    padding: theme.spacing[3],
    gap: theme.spacing[1],
  },
  errorsBannerText: {
    color: theme.colors.palette.red[300],
    fontSize: theme.fontSize.sm,
  },
  filterEmpty: {
    flexGrow: 1,
    paddingHorizontal: { xs: theme.spacing[3], md: theme.spacing[6] },
    paddingVertical: theme.spacing[6],
    alignItems: "center",
    justifyContent: "center",
  },
  emptyState: {
    alignItems: "center",
    gap: theme.spacing[4],
    maxWidth: 420,
    width: "100%",
  },
  endedEmptyState: {
    alignItems: "center",
    gap: theme.spacing[3],
  },
  emptyTextStack: {
    alignItems: "center",
    gap: theme.spacing[2],
  },
  emptyTitle: {
    color: theme.colors.foreground,
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.normal,
    textAlign: "center",
  },
  emptyDescription: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
    textAlign: "center",
  },
  message: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
    textAlign: "center",
  },
  // Static color holder read by the spinner; keeps the muted token without
  // useUnistyles (banned in new code).
  spinner: {
    color: theme.colors.foregroundMuted,
  },
  emptyIcon: {
    color: theme.colors.foregroundMuted,
    width: theme.iconSize.lg,
  },
}));
