import React, { useState } from "react";
import { Modal, Platform, Pressable, ScrollView, Text, View } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import {
  X,
  FileCode,
  Terminal,
  Bot,
  User,
  Clock,
  Sparkles,
  ArrowRight,
  ChevronRight,
  Code2,
} from "lucide-react-native";
import { Button } from "@/components/ui/button";

export interface OmpTurnToolCall {
  id: string;
  name: string;
  args: unknown;
}

export interface OmpTurn {
  id: string;
  role: "user" | "assistant" | "toolResult" | "custom" | "system";
  text?: string;
  timestamp?: string;
  toolCalls?: OmpTurnToolCall[];
  toolResult?: {
    toolCallId: string;
    toolName?: string;
    content: unknown;
    isError?: boolean;
  };
}

export interface OmpFileDiffItem {
  path: string;
  tool: string;
  input?: unknown;
  status: "completed" | "error" | "pending";
}

export interface OmpSessionDetailData {
  id: string;
  filePath: string;
  cwd: string;
  title: string | null;
  createdAt: string | null;
  lastActivityAt: string | null;
  model: string | null;
  thinkingOptionId: string | null;
  turns: OmpTurn[];
  fileDiffs: OmpFileDiffItem[];
  stats: {
    totalTurns: number;
    totalToolCalls: number;
    filesModified: number;
  };
}

export interface OmpSessionInspectorModalProps {
  visible: boolean;
  session: OmpSessionDetailData | null;
  onClose: () => void;
  onResume: (session: OmpSessionDetailData) => void;
}

export function OmpSessionInspectorModal({
  visible,
  session,
  onClose,
  onResume,
}: OmpSessionInspectorModalProps) {
  const { theme } = useUnistyles();
  const [activeTab, setActiveTab] = useState<"timeline" | "files">("timeline");

  if (!visible || !session) return null;

  return (
    <Modal visible={visible} transparent animationType="fade" onRequestClose={onClose}>
      <View style={styles.backdrop}>
        <View style={styles.container}>
          {/* Header */}
          <View style={styles.header}>
            <View style={styles.headerTitleCol}>
              <View style={styles.badgeRow}>
                <View style={styles.ompBadge}>
                  <Bot size={12} color="#10b981" />
                  <Text style={styles.ompBadgeText}>OMP Session</Text>
                </View>
                {session.model ? (
                  <View style={styles.modelBadge}>
                    <Sparkles size={11} color={theme.colors.foregroundMuted} />
                    <Text style={styles.modelBadgeText}>{session.model}</Text>
                  </View>
                ) : null}
              </View>
              <Text style={styles.title} numberOfLines={1}>
                {session.title || session.id}
              </Text>
              <Text style={styles.subtitle} numberOfLines={1}>
                {session.cwd}
              </Text>
            </View>

            <View style={styles.headerActions}>
              <Button
                variant="default"
                size="sm"
                onPress={() => onResume(session)}
                testID="resume-omp-session-btn"
              >
                <Text style={styles.resumeBtnText}>Resume in Paseo</Text>
              </Button>
              <Pressable
                onPress={onClose}
                style={styles.closeBtn}
                accessibilityRole="button"
                accessibilityLabel="Close"
              >
                <X size={18} color={theme.colors.foregroundMuted} />
              </Pressable>
            </View>
          </View>

          {/* Tab Selector */}
          <View style={styles.tabBar}>
            <Pressable
              style={[styles.tab, activeTab === "timeline" && styles.tabActive]}
              onPress={() => setActiveTab("timeline")}
            >
              <Terminal
                size={14}
                color={
                  activeTab === "timeline" ? theme.colors.foreground : theme.colors.foregroundMuted
                }
              />
              <Text style={[styles.tabText, activeTab === "timeline" && styles.tabTextActive]}>
                Timeline ({session.turns.length})
              </Text>
            </Pressable>

            <Pressable
              style={[styles.tab, activeTab === "files" && styles.tabActive]}
              onPress={() => setActiveTab("files")}
            >
              <FileCode
                size={14}
                color={
                  activeTab === "files" ? theme.colors.foreground : theme.colors.foregroundMuted
                }
              />
              <Text style={[styles.tabText, activeTab === "files" && styles.tabTextActive]}>
                Files Changed ({session.fileDiffs.length})
              </Text>
            </Pressable>
          </View>

          {/* Content Area */}
          <ScrollView style={styles.contentScroll} contentContainerStyle={styles.contentInner}>
            {activeTab === "timeline" ? (
              <View style={styles.timelineList}>
                {session.turns.map((turn, index) => (
                  <View key={turn.id || `turn-${index}`} style={styles.turnCard}>
                    <View style={styles.turnHeader}>
                      <View style={styles.turnRolePill}>
                        {turn.role === "user" ? (
                          <User size={13} color="#3b82f6" />
                        ) : turn.role === "assistant" ? (
                          <Bot size={13} color="#10b981" />
                        ) : (
                          <Terminal size={13} color="#a855f7" />
                        )}
                        <Text style={styles.turnRoleText}>{turn.role.toUpperCase()}</Text>
                      </View>
                      {turn.timestamp ? (
                        <View style={styles.timestampRow}>
                          <Clock size={11} color={theme.colors.foregroundMuted} />
                          <Text style={styles.timestampText}>
                            {new Date(turn.timestamp).toLocaleTimeString([], {
                              hour: "2-digit",
                              minute: "2-digit",
                            })}
                          </Text>
                        </View>
                      ) : null}
                    </View>

                    {turn.text ? <Text style={styles.turnBodyText}>{turn.text}</Text> : null}

                    {turn.toolCalls && turn.toolCalls.length > 0 ? (
                      <View style={styles.toolCallsWrap}>
                        {turn.toolCalls.map((tc) => (
                          <View key={tc.id} style={styles.toolCallItem}>
                            <Code2 size={12} color="#f59e0b" />
                            <Text style={styles.toolCallName}>{tc.name}</Text>
                          </View>
                        ))}
                      </View>
                    ) : null}

                    {turn.toolResult ? (
                      <View style={styles.toolResultBox}>
                        <Text style={styles.toolResultLabel}>
                          Result: {turn.toolResult.toolName || turn.toolResult.toolCallId}
                        </Text>
                        <Text style={styles.toolResultSnippet} numberOfLines={3}>
                          {typeof turn.toolResult.content === "string"
                            ? turn.toolResult.content
                            : JSON.stringify(turn.toolResult.content)}
                        </Text>
                      </View>
                    ) : null}
                  </View>
                ))}
              </View>
            ) : (
              <View style={styles.filesList}>
                {session.fileDiffs.length === 0 ? (
                  <View style={styles.emptyFilesBox}>
                    <Text style={styles.emptyFilesText}>
                      No file modifications recorded in this session.
                    </Text>
                  </View>
                ) : (
                  session.fileDiffs.map((diff, index) => (
                    <View key={diff.path || `diff-${index}`} style={styles.diffItem}>
                      <View style={styles.diffHeader}>
                        <FileCode size={16} color="#3b82f6" />
                        <Text style={styles.diffPathText}>{diff.path}</Text>
                        <View style={styles.diffToolTag}>
                          <Text style={styles.diffToolText}>{diff.tool}</Text>
                        </View>
                      </View>
                    </View>
                  ))
                )}
              </View>
            )}
          </ScrollView>
        </View>
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create((theme) => ({
  backdrop: {
    flex: 1,
    backgroundColor: "rgba(0, 0, 0, 0.7)",
    justifyContent: "center",
    alignItems: "center",
    padding: theme.spacing[4],
  },
  container: {
    width: "100%",
    maxWidth: 780,
    maxHeight: "85%",
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.xl,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 12 },
    shadowOpacity: 0.35,
    shadowRadius: 24,
    elevation: 20,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[3],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface2,
  },
  headerTitleCol: {
    flex: 1,
    marginRight: theme.spacing[3],
  },
  badgeRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    marginBottom: 4,
  },
  ompBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: "rgba(16, 185, 129, 0.15)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: theme.borderRadius.full,
    borderWidth: 1,
    borderColor: "rgba(16, 185, 129, 0.3)",
  },
  ompBadgeText: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.semibold,
    color: "#10b981",
  },
  modelBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: theme.colors.surface3,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: theme.borderRadius.full,
  },
  modelBadgeText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  title: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.bold,
    color: theme.colors.foreground,
  },
  subtitle: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    marginTop: 2,
  },
  headerActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  resumeBtnText: {
    color: "#ffffff",
    fontWeight: theme.fontWeight.semibold,
    fontSize: theme.fontSize.sm,
  },
  closeBtn: {
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.md,
  },
  tabBar: {
    flexDirection: "row",
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    backgroundColor: theme.colors.surface1,
    paddingHorizontal: theme.spacing[3],
  },
  tab: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingVertical: theme.spacing[2],
    paddingHorizontal: theme.spacing[3],
    borderBottomWidth: 2,
    borderBottomColor: "transparent",
  },
  tabActive: {
    borderBottomColor: theme.colors.primary,
  },
  tabText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    fontWeight: theme.fontWeight.medium,
  },
  tabTextActive: {
    color: theme.colors.foreground,
    fontWeight: theme.fontWeight.semibold,
  },
  contentScroll: {
    flex: 1,
  },
  contentInner: {
    padding: theme.spacing[4],
  },
  timelineList: {
    gap: theme.spacing[3],
  },
  turnCard: {
    backgroundColor: theme.colors.surface2,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[3],
    gap: theme.spacing[2],
  },
  turnHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",
  },
  turnRolePill: {
    flexDirection: "row",
    alignItems: "center",
    gap: 5,
  },
  turnRoleText: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.bold,
    color: theme.colors.foreground,
  },
  timestampRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  timestampText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  turnBodyText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    lineHeight: 20,
    fontFamily: Platform.OS === "web" ? "monospace" : undefined,
  },
  toolCallsWrap: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 6,
    marginTop: 4,
  },
  toolCallItem: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: "rgba(245, 158, 11, 0.12)",
    paddingHorizontal: 8,
    paddingVertical: 3,
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: "rgba(245, 158, 11, 0.25)",
  },
  toolCallName: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.medium,
    color: "#f59e0b",
  },
  toolResultBox: {
    backgroundColor: theme.colors.surface3,
    borderRadius: theme.borderRadius.md,
    padding: theme.spacing[2],
    marginTop: 4,
  },
  toolResultLabel: {
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.semibold,
    color: theme.colors.foregroundMuted,
    marginBottom: 2,
  },
  toolResultSnippet: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    fontFamily: Platform.OS === "web" ? "monospace" : undefined,
  },
  filesList: {
    gap: theme.spacing[2],
  },
  emptyFilesBox: {
    padding: theme.spacing[6],
    alignItems: "center",
  },
  emptyFilesText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  diffItem: {
    backgroundColor: theme.colors.surface2,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[3],
  },
  diffHeader: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  diffPathText: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    fontWeight: theme.fontWeight.medium,
    color: theme.colors.foreground,
  },
  diffToolTag: {
    backgroundColor: "rgba(59, 130, 246, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: theme.borderRadius.sm,
  },
  diffToolText: {
    fontSize: theme.fontSize.sm,
    color: "#3b82f6",
    fontWeight: theme.fontWeight.semibold,
  },
}));
