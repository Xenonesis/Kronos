import React from "react";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { useSessionStore } from "@/stores/session-store";
import { useCompactTimeAgo } from "@/hooks/use-time-ago";
import { formatCompactTimeAgoAsProse } from "@/utils/time";
import { AGENT_TOOLTIP_TITLE_MAX_LENGTH } from "./tab-geometry";

export function normalizeAgentTooltipTitle(title: string): string {
  return title.replace(/\s+/g, " ").trim();
}

export function formatAgentTooltipTitle(singleLineTitle: string): string {
  if (singleLineTitle.length <= AGENT_TOOLTIP_TITLE_MAX_LENGTH) return singleLineTitle;
  return `${singleLineTitle.slice(0, AGENT_TOOLTIP_TITLE_MAX_LENGTH - 1).trimEnd()}…`;
}

export function AgentTabTooltipBody({
  serverId,
  agentId,
  title,
}: {
  serverId: string;
  agentId: string;
  title: string;
}) {
  const lastActivityAt = useSessionStore((state) => {
    const session = state.sessions[serverId];
    const agent = session?.agents.get(agentId) ?? session?.agentDetails.get(agentId) ?? null;
    return state.agentLastActivity.get(agentId) ?? agent?.lastActivityAt ?? null;
  });
  const compactActivity = useCompactTimeAgo(lastActivityAt);
  const activity = formatCompactTimeAgoAsProse(compactActivity);

  return (
    <View style={styles.tooltipAgentContent}>
      <Text style={styles.agentTooltipTitle} numberOfLines={1} ellipsizeMode="tail">
        {title}
      </Text>
      <View style={styles.tooltipAgentMetadata}>
        <Text style={styles.tooltipAgentId}>{agentId.slice(0, 7)}</Text>
        {activity ? (
          <>
            <Text style={styles.tooltipAgentSeparator}>·</Text>
            <Text style={styles.tooltipAgentActivity}>{activity}</Text>
          </>
        ) : null}
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  tooltipAgentContent: {
    gap: theme.spacing[0.5],
    maxWidth: 420,
  },
  agentTooltipTitle: {
    color: theme.colors.foreground,
    fontSize: theme.fontSize.base,
  },
  tooltipAgentMetadata: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  tooltipAgentId: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.sm,
  },
  tooltipAgentSeparator: {
    color: theme.colors.foregroundExtraMuted,
    fontSize: theme.fontSize.sm,
  },
  tooltipAgentActivity: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.sm,
  },
}));
