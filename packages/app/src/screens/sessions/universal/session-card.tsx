import React, { memo, useMemo, useCallback } from "react";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { ChevronDown, ChevronRight, MoreHorizontal } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { useTimeAgo } from "@/hooks/use-time-ago";
import type { UniversalIndexedSessionItem } from "./session-types";
import { getProviderTheme, ProviderGlyph } from "./provider-theme";

interface SessionItemCardProps {
  item: UniversalIndexedSessionItem;
  isExpanded: boolean;
  onToggleExpand: (id: string) => void;
  onOpenDetails: (session: UniversalIndexedSessionItem) => void;
}

export const SessionItemCard = memo(function SessionItemCard({
  item,
  isExpanded,
  onToggleExpand,
  onOpenDetails,
}: SessionItemCardProps) {
  const providerMeta = getProviderTheme(item.provider);
  const dateObj = useMemo(() => {
    const raw = item.updated_at || item.created_at;
    return raw ? new Date(raw) : null;
  }, [item.updated_at, item.created_at]);
  const timeAgo = useTimeAgo(dateObj);

  const handleToggle = useCallback(() => {
    onToggleExpand(item.id);
  }, [item.id, onToggleExpand]);

  const handleDetails = useCallback(() => {
    onOpenDetails(item);
  }, [item, onOpenDetails]);

  return (
    <PressHighlight
      style={[styles.sessionCard, isExpanded && styles.sessionCardActive]}
      onPress={handleToggle}
    >
      <View style={styles.cardHeaderRow}>
        <Text style={styles.cardTitle} numberOfLines={isExpanded ? undefined : 1}>
          {item.title || item.id}
        </Text>
        <View style={styles.cardActionsRow}>
          <PressHighlight hitSlop={8} onPress={handleToggle} style={styles.chevronButton}>
            {isExpanded ? (
              <ChevronDown size={14} color="#94a3b8" />
            ) : (
              <ChevronRight size={14} color="#64748b" />
            )}
          </PressHighlight>
          <PressHighlight hitSlop={8} onPress={handleDetails} style={styles.moreButton}>
            <MoreHorizontal size={14} color="#64748b" />
          </PressHighlight>
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
    </PressHighlight>
  );
});

const styles = StyleSheet.create({
  sessionCard: {
    backgroundColor: "#13141a",
    borderRadius: 8,
    padding: 12,
    borderWidth: 1,
    borderColor: "rgba(255, 255, 255, 0.05)",
  },
  sessionCardActive: {
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
});
