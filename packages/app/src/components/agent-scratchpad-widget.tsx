import { memo, useState, useCallback } from "react";
import { View, Text, Pressable } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import {
  BrainCircuit,
  ChevronDown,
  ChevronUp,
  Target,
  Lightbulb,
  Search,
  CheckCircle2,
  AlertTriangle,
  Trash2,
} from "lucide-react-native";
import { useScratchpadStore } from "@/stores/scratchpad-store";

export interface AgentScratchpadWidgetProps {
  agentId: string;
  defaultExpanded?: boolean;
}

export const AgentScratchpadWidget = memo(function AgentScratchpadWidget({
  agentId,
  defaultExpanded = false,
}: AgentScratchpadWidgetProps) {
  const { theme } = useUnistyles();
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);
  const scratchpad = useScratchpadStore((state) => state.getScratchpad(agentId));
  const clearScratchpad = useScratchpadStore((state) => state.clearScratchpad);
  const removeBlocker = useScratchpadStore((state) => state.removeBlocker);

  const hasAnyContent = Boolean(
    scratchpad.objective ||
    scratchpad.hypotheses.length > 0 ||
    scratchpad.findings.length > 0 ||
    scratchpad.decisions.length > 0 ||
    scratchpad.blockers.length > 0,
  );

  const toggleExpanded = useCallback(() => {
    setIsExpanded((prev) => !prev);
  }, []);

  const handleClear = useCallback(() => {
    clearScratchpad(agentId);
  }, [clearScratchpad, agentId]);

  if (!hasAnyContent) {
    return null;
  }

  return (
    <View style={styles.container}>
      <Pressable onPress={toggleExpanded} style={styles.header}>
        <View style={styles.headerLeft}>
          <BrainCircuit size={16} color={theme.colors.accent} />
          <Text style={styles.headerTitle}>Cascade Scratchpad</Text>
          <View style={styles.badgeRow}>
            {scratchpad.findings.length > 0 && (
              <View style={styles.counterBadge}>
                <Text style={styles.counterBadgeText}>{scratchpad.findings.length} findings</Text>
              </View>
            )}
            {scratchpad.decisions.length > 0 && (
              <View style={styles.counterBadgeAccent}>
                <Text style={styles.counterBadgeAccentText}>
                  {scratchpad.decisions.length} decisions
                </Text>
              </View>
            )}
            {scratchpad.blockers.length > 0 && (
              <View style={styles.counterBadgeAlert}>
                <Text style={styles.counterBadgeAlertText}>
                  {scratchpad.blockers.length} blockers
                </Text>
              </View>
            )}
          </View>
        </View>

        <View style={styles.headerRight}>
          {isExpanded ? (
            <ChevronUp size={16} color={theme.colors.foregroundMuted} />
          ) : (
            <ChevronDown size={16} color={theme.colors.foregroundMuted} />
          )}
        </View>
      </Pressable>

      {isExpanded && (
        <View style={styles.content}>
          {/* Objective */}
          {scratchpad.objective ? (
            <View style={styles.section}>
              <View style={styles.sectionHeader}>
                <Target size={13} color={theme.colors.accent} />
                <Text style={styles.sectionTitle}>Objective</Text>
              </View>
              <Text style={styles.objectiveText}>{scratchpad.objective}</Text>
            </View>
          ) : null}

          {/* Hypotheses */}
          {scratchpad.hypotheses.length > 0 && (
            <View style={styles.section}>
              <View style={styles.sectionHeader}>
                <Lightbulb size={13} color="#eab308" />
                <Text style={styles.sectionTitle}>Working Hypotheses</Text>
              </View>
              <View style={styles.list}>
                {scratchpad.hypotheses.map((item, idx) => (
                  <Text key={idx} style={styles.listItemText}>
                    • {item}
                  </Text>
                ))}
              </View>
            </View>
          )}

          {/* Findings */}
          {scratchpad.findings.length > 0 && (
            <View style={styles.section}>
              <View style={styles.sectionHeader}>
                <Search size={13} color="#06b6d4" />
                <Text style={styles.sectionTitle}>Discovered Findings</Text>
              </View>
              <View style={styles.list}>
                {scratchpad.findings.map((item, idx) => (
                  <Text key={idx} style={styles.listItemText}>
                    • {item}
                  </Text>
                ))}
              </View>
            </View>
          )}

          {/* Decisions */}
          {scratchpad.decisions.length > 0 && (
            <View style={styles.section}>
              <View style={styles.sectionHeader}>
                <CheckCircle2 size={13} color={theme.colors.success ?? "#16a34a"} />
                <Text style={styles.sectionTitle}>Architectural Decisions</Text>
              </View>
              <View style={styles.list}>
                {scratchpad.decisions.map((item, idx) => (
                  <Text key={idx} style={styles.decisionItemText}>
                    ✓ {item}
                  </Text>
                ))}
              </View>
            </View>
          )}

          {/* Blockers */}
          {scratchpad.blockers.length > 0 && (
            <View style={[styles.section, styles.blockerSection]}>
              <View style={styles.sectionHeader}>
                <AlertTriangle size={13} color="#dc2626" />
                <Text style={styles.blockerTitle}>Active Blockers</Text>
              </View>
              <View style={styles.list}>
                {scratchpad.blockers.map((item, idx) => (
                  <View key={idx} style={styles.blockerRow}>
                    <Text style={styles.blockerItemText}>⚠ {item}</Text>
                    <Pressable
                      onPress={() => removeBlocker(agentId, idx)}
                      style={styles.resolveBtn}
                    >
                      <Trash2 size={12} color="#dc2626" />
                    </Pressable>
                  </View>
                ))}
              </View>
            </View>
          )}

          <View style={styles.footerRow}>
            <Pressable onPress={handleClear} style={styles.clearBtn}>
              <Text style={styles.clearBtnText}>Clear Scratchpad</Text>
            </Pressable>
          </View>
        </View>
      )}
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    backgroundColor: theme.colors.surface1,
    borderWidth: 1,
    borderColor: theme.colors.border,
    borderRadius: theme.borderRadius.md,
    marginVertical: theme.spacing[2],
    marginHorizontal: theme.spacing[3],
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface0,
  },
  headerLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    flex: 1,
  },
  headerTitle: {
    fontSize: theme.fontSize.sm,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  badgeRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    marginLeft: theme.spacing[1],
  },
  counterBadge: {
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: 6,
    paddingVertical: 1,
    borderRadius: 4,
  },
  counterBadgeText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
    fontWeight: "500",
  },
  counterBadgeAccent: {
    backgroundColor: "rgba(59, 130, 246, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 1,
    borderRadius: 4,
  },
  counterBadgeAccentText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.accent,
    fontWeight: "600",
  },
  counterBadgeAlert: {
    backgroundColor: "rgba(220, 38, 38, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 1,
    borderRadius: 4,
  },
  counterBadgeAlertText: {
    fontSize: theme.fontSize.code,
    color: "#dc2626",
    fontWeight: "600",
  },
  headerRight: {
    padding: theme.spacing[1],
  },
  content: {
    padding: theme.spacing[3],
    gap: theme.spacing[3],
    backgroundColor: theme.colors.surface1,
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  section: {
    gap: theme.spacing[1],
  },
  sectionHeader: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
  },
  sectionTitle: {
    fontSize: theme.fontSize.code,
    fontWeight: "600",
    color: theme.colors.foregroundMuted,
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  objectiveText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    fontWeight: "500",
    lineHeight: 20,
    backgroundColor: theme.colors.surface0,
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  list: {
    gap: 4,
    paddingLeft: theme.spacing[1],
  },
  listItemText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    lineHeight: 19,
  },
  decisionItemText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    fontWeight: "500",
    lineHeight: 19,
  },
  blockerSection: {
    backgroundColor: "rgba(220, 38, 38, 0.05)",
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: "rgba(220, 38, 38, 0.2)",
  },
  blockerTitle: {
    fontSize: theme.fontSize.code,
    fontWeight: "600",
    color: "#dc2626",
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  blockerRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  blockerItemText: {
    fontSize: theme.fontSize.sm,
    color: "#dc2626",
    flex: 1,
  },
  resolveBtn: {
    padding: 4,
  },
  footerRow: {
    flexDirection: "row",
    justifyContent: "flex-end",
    paddingTop: theme.spacing[1],
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  clearBtn: {
    paddingVertical: 2,
    paddingHorizontal: theme.spacing[1],
  },
  clearBtnText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
}));
