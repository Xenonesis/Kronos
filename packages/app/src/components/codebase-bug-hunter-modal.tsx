import React, { memo, useState } from "react";
import { View, Text, Pressable, ScrollView } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import {
  ShieldAlert,
  ShieldCheck,
  Bug,
  AlertTriangle,
  CheckCircle,
  Sparkles,
  X,
} from "lucide-react-native";
import { useSessionStore } from "@/stores/session-store";
import { useDraftStore } from "@/stores/draft-store";
import { useToast } from "@/contexts/toast-context";

export interface BugAuditItem {
  id: string;
  filePath: string;
  lineNumber: number;
  severity: "Critical" | "High" | "Medium" | "Low";
  category: string;
  title: string;
  description: string;
  suggestedFix: string;
}

export interface CodebaseBugHunterModalProps {
  visible: boolean;
  serverId?: string;
  agentId?: string;
  onClose: () => void;
  items?: BugAuditItem[];
}

export const CodebaseBugHunterModal = memo(function CodebaseBugHunterModal({
  visible,
  serverId: propServerId,
  agentId: propAgentId,
  onClose,
  items = [],
}: CodebaseBugHunterModalProps) {
  const { theme } = useUnistyles();
  const toast = useToast();
  const [filterSeverity, setFilterSeverity] = useState<string | null>(null);

  if (!visible) return null;

  const criticalCount = items.filter((i) => i.severity === "Critical").length;
  const highCount = items.filter((i) => i.severity === "High").length;
  const mediumCount = items.filter((i) => i.severity === "Medium").length;

  const displayedItems = filterSeverity
    ? items.filter((i) => i.severity === filterSeverity)
    : items;

  const handleFixIssue = (item: BugAuditItem) => {
    const sessions = useSessionStore.getState().sessions;
    const serverId = propServerId ?? Object.keys(sessions)[0];
    const session = serverId ? sessions[serverId] : undefined;
    const activeAgentId =
      propAgentId ?? (session?.agents ? Array.from(session.agents.values())[0]?.id : undefined);

    if (!serverId || !activeAgentId) {
      toast.show("No active agent available to apply fix", { variant: "error" });
      return;
    }

    const draftKey = `agent:${serverId}:${activeAgentId}`;
    const fixPrompt =
      `### 🛡️ Kronos Security / Bug Fix Request\n` +
      `File: \`${item.filePath}:${item.lineNumber}\`\n` +
      `Category: **${item.category}** (Severity: ${item.severity})\n` +
      `Issue: ${item.title}\n\n` +
      `**Description:** ${item.description}\n` +
      `**Recommended Fix:** ${item.suggestedFix}\n\n` +
      `Please apply this fix cleanly and verify that all types and tests pass.`;

    useDraftStore.getState().replaceDraftText({ draftKey, text: fixPrompt });
    toast.show("Fix instructions sent to composer", { variant: "success" });
    onClose();
  };

  return (
    <View style={styles.overlay}>
      <View style={styles.modal}>
        {/* Header */}
        <View style={styles.header}>
          <View style={styles.headerTitleRow}>
            <ShieldAlert size={20} color={theme.colors.destructive} />
            <Text style={styles.headerTitle}>Codebase Security & Bug Hunter</Text>
          </View>
          <Pressable onPress={onClose} style={styles.closeButton}>
            <X size={18} color={theme.colors.foregroundMuted} />
          </Pressable>
        </View>

        {/* Severity Metrics Bar */}
        <View style={styles.metricsBar}>
          <Pressable
            onPress={() => setFilterSeverity(null)}
            style={[styles.metricCard, !filterSeverity && styles.metricCardActive]}
          >
            <Text style={styles.metricLabel}>Total Issues</Text>
            <Text style={styles.metricValue}>{items.length}</Text>
          </Pressable>
          <Pressable
            onPress={() => setFilterSeverity("Critical")}
            style={[styles.metricCard, filterSeverity === "Critical" && styles.metricCardActive]}
          >
            <Text style={[styles.metricLabel, { color: theme.colors.destructive }]}>Critical</Text>
            <Text style={[styles.metricValue, { color: theme.colors.destructive }]}>
              {criticalCount}
            </Text>
          </Pressable>
          <Pressable
            onPress={() => setFilterSeverity("High")}
            style={[styles.metricCard, filterSeverity === "High" && styles.metricCardActive]}
          >
            <Text style={[styles.metricLabel, { color: "#f97316" }]}>High</Text>
            <Text style={[styles.metricValue, { color: "#f97316" }]}>{highCount}</Text>
          </Pressable>
          <Pressable
            onPress={() => setFilterSeverity("Medium")}
            style={[styles.metricCard, filterSeverity === "Medium" && styles.metricCardActive]}
          >
            <Text style={[styles.metricLabel, { color: "#eab308" }]}>Medium</Text>
            <Text style={[styles.metricValue, { color: "#eab308" }]}>{mediumCount}</Text>
          </Pressable>
        </View>

        {/* Issues List */}
        <ScrollView style={styles.issuesScroll}>
          {displayedItems.length === 0 ? (
            <View style={{ paddingVertical: 24, alignItems: "center", gap: 6 }}>
              <ShieldCheck size={28} color="#22c55e" />
              <Text style={styles.issueTitle}>No security or bug issues detected</Text>
              <Text style={styles.issueDescription}>
                Static audit across workspace source files completed cleanly.
              </Text>
            </View>
          ) : null}
          {displayedItems.map((item) => (
            <View key={item.id} style={styles.issueCard}>
              <View style={styles.issueHeader}>
                <View style={styles.badgeRow}>
                  <Text
                    style={[
                      styles.severityBadge,
                      item.severity === "Critical"
                        ? styles.badgeCritical
                        : item.severity === "High"
                          ? styles.badgeHigh
                          : styles.badgeMedium,
                    ]}
                  >
                    {item.severity}
                  </Text>
                  <Text style={styles.categoryBadge}>{item.category}</Text>
                </View>
                <Pressable
                  onPress={() => handleFixIssue(item)}
                  style={styles.fixButton}
                  testID={`fix-issue-${item.id}`}
                >
                  <Sparkles size={12} color="#ffffff" />
                  <Text style={styles.fixButtonText}>Fix with Kronos</Text>
                </Pressable>
              </View>

              <Text style={styles.issueTitle}>{item.title}</Text>
              <Text style={styles.issueLocation}>
                {item.filePath}:{item.lineNumber}
              </Text>
              <Text style={styles.issueDescription}>{item.description}</Text>
              <View style={styles.fixRecommendation}>
                <Text style={styles.fixLabel}>Suggested Fix:</Text>
                <Text style={styles.fixText}>{item.suggestedFix}</Text>
              </View>
            </View>
          ))}
        </ScrollView>
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  overlay: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: "rgba(0, 0, 0, 0.7)",
    justifyContent: "center",
    alignItems: "center",
    zIndex: 1100,
  },
  modal: {
    width: "90%",
    maxWidth: 680,
    maxHeight: "85%",
    backgroundColor: theme.colors.surface0,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: 20,
    boxShadow: "0 10px 30px rgba(0,0,0,0.5)",
  },
  header: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: 16,
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,
  },
  headerTitle: {
    fontSize: 16,
    fontWeight: "700",
    color: theme.colors.foreground,
  },
  closeButton: {
    padding: 6,
    borderRadius: 6,
  },
  metricsBar: {
    flexDirection: "row",
    gap: 8,
    marginBottom: 16,
  },
  metricCard: {
    flex: 1,
    padding: 10,
    backgroundColor: theme.colors.surface1,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "transparent",
    alignItems: "center",
  },
  metricCardActive: {
    borderColor: theme.colors.accent,
  },
  metricLabel: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
    fontWeight: "500",
  },
  metricValue: {
    fontSize: 18,
    fontWeight: "700",
    color: theme.colors.foreground,
    marginTop: 2,
  },
  issuesScroll: {
    maxHeight: 450,
  },
  issueCard: {
    backgroundColor: theme.colors.surface1,
    borderRadius: 8,
    padding: 14,
    marginBottom: 10,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  issueHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: 8,
  },
  badgeRow: {
    flexDirection: "row",
    gap: 6,
    alignItems: "center",
  },
  severityBadge: {
    fontSize: 10,
    fontWeight: "700",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  badgeCritical: {
    backgroundColor: "rgba(239, 68, 68, 0.2)",
    color: "#ef4444",
  },
  badgeHigh: {
    backgroundColor: "rgba(249, 115, 22, 0.2)",
    color: "#f97316",
  },
  badgeMedium: {
    backgroundColor: "rgba(234, 179, 8, 0.2)",
    color: "#eab308",
  },
  categoryBadge: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  fixButton: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: theme.colors.accent,
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 6,
  },
  fixButtonText: {
    color: "#ffffff",
    fontSize: 11,
    fontWeight: "600",
  },
  issueTitle: {
    fontSize: 13,
    fontWeight: "600",
    color: theme.colors.foreground,
    marginBottom: 4,
  },
  issueLocation: {
    fontSize: 11,
    fontFamily: "monospace",
    color: theme.colors.foregroundMuted,
    marginBottom: 6,
  },
  issueDescription: {
    fontSize: 12,
    color: theme.colors.foreground,
    marginBottom: 8,
  },
  fixRecommendation: {
    backgroundColor: "rgba(0,0,0,0.2)",
    padding: 8,
    borderRadius: 6,
  },
  fixLabel: {
    fontSize: 10,
    fontWeight: "700",
    color: theme.colors.foregroundMuted,
    marginBottom: 2,
  },
  fixText: {
    fontSize: 11,
    color: theme.colors.foreground,
  },
}));
