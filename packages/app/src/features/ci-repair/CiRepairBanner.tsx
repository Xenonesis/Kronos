import React, { memo, useState, useCallback, useEffect } from "react";
import { View, Text, Pressable } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import {
  AlertTriangle,
  Zap,
  ChevronDown,
  ChevronUp,
  X,
  FileCode,
  GitBranch,
} from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";
import { Button } from "@/components/ui/button";
import { useToast } from "@/contexts/toast-context";
import { useActiveWorkspaceSelection } from "@/stores/navigation-active-workspace-store";
import { useSessionStore } from "@/stores/session-store";
import { useCiStore } from "./use-ci-status";

const ThemedAlert = withUnistyles(AlertTriangle, (theme) => ({
  color: "#f59e0b",
}));
const ThemedClose = withUnistyles(X, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

export interface CiRepairBannerProps {
  onPopulateComposer?: (prompt: string) => void;
  cwd?: string;
  branch?: string;
}

export const CiRepairBanner = memo(function CiRepairBanner({
  onPopulateComposer,
  cwd: explicitCwd,
  branch: explicitBranch,
}: CiRepairBannerProps) {
  const toast = useToast();
  const [detailsExpanded, setDetailsExpanded] = useState(false);

  const {
    ciStatus,
    isLoading,
    isRepairing,
    isDismissed,
    checkStatus,
    dismiss,
    repairWithAgent,
  } = useCiStore();

  const activeWorkspace = useActiveWorkspaceSelection();
  const workspaceSession = useSessionStore((state) =>
    activeWorkspace
      ? state.sessions[activeWorkspace.serverId]?.workspaces.get(activeWorkspace.workspaceId)
      : null,
  );

  const effectiveCwd = explicitCwd || workspaceSession?.workspaceDirectory;
  const effectiveBranch = explicitBranch || workspaceSession?.gitRuntime?.currentBranch || "main";

  // Check CI status whenever active workspace changes
  useEffect(() => {
    if (effectiveCwd) {
      void checkStatus(effectiveCwd, effectiveBranch);
    }
  }, [checkStatus, effectiveBranch, effectiveCwd]);

  const handleRepair = useCallback(async () => {
    if (isRepairing || !ciStatus) return;
    try {
      await repairWithAgent({
        onPopulateComposer,
        cwd: effectiveCwd,
      });
      toast.show("CI Failure Repair prompt dispatched to agent!");
    } catch {
      toast.show("Failed to initiate CI repair");
    }
  }, [ciStatus, effectiveCwd, isRepairing, onPopulateComposer, repairWithAgent, toast]);

  if (!ciStatus || !ciStatus.hasFailure || isDismissed) {
    return null;
  }

  return (
    <View style={styles.bannerContainer} testID="ci-repair-banner">
      <View style={styles.bannerHeader}>
        <View style={styles.leftInfo}>
          <View style={styles.iconCircle}>
            <ThemedAlert size={16} />
          </View>
          <View style={styles.titleColumn}>
            <Text style={styles.bannerTitle} numberOfLines={1}>
              ❌ CI Failure: Workflow &apos;{ciStatus.workflowName}&apos; failed on branch &apos;
              {ciStatus.branch}&apos; (Run #{ciStatus.runId})
            </Text>
            {ciStatus.failedJobName ? (
              <View style={styles.subInfoRow}>
                <Text style={styles.subInfoText}>
                  Job: {ciStatus.failedJobName}
                  {ciStatus.failureStep ? ` • Step: ${ciStatus.failureStep}` : ""}
                </Text>
              </View>
            ) : null}
          </View>
        </View>

        <View style={styles.rightActions}>
          <PressHighlight
            style={styles.expandToggle}
            onPress={() => setDetailsExpanded((prev) => !prev)}
          >
            {detailsExpanded ? (
              <ChevronUp size={14} color="#f59e0b" />
            ) : (
              <ChevronDown size={14} color="#f59e0b" />
            )}
            <Text style={styles.expandToggleText}>
              {detailsExpanded ? "Hide Details" : "View Logs"}
            </Text>
          </PressHighlight>

          <Button
            size="sm"
            onPress={handleRepair}
            disabled={isRepairing}
            style={styles.repairButton}
          >
            <Zap size={13} color="#ffffff" />
            <Text style={styles.repairButtonText}>
              {isRepairing ? "Preparing Repair..." : "⚡ 1-Click Repair with Agent"}
            </Text>
          </Button>

          <PressHighlight style={styles.closeBtn} onPress={dismiss} accessibilityLabel="Dismiss">
            <ThemedClose size={16} />
          </PressHighlight>
        </View>
      </View>

      {/* Expanded failure details & log snippet */}
      {detailsExpanded ? (
        <View style={styles.detailsBody}>
          {ciStatus.commitMessage ? (
            <Text style={styles.commitLine}>
              Commit: <Text style={styles.commitSha}>{ciStatus.commitSha || "HEAD"}</Text> -{" "}
              {ciStatus.commitMessage}
            </Text>
          ) : null}
          <View style={styles.logBox}>
            <Text style={styles.logText}>
              {ciStatus.errorSnippet ||
                "Failed test assertion in suite. Run vitest / npm test to observe full output."}
            </Text>
          </View>
        </View>
      ) : null}
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  bannerContainer: {
    marginHorizontal: theme.spacing[3],
    marginVertical: theme.spacing[2],
    backgroundColor: "rgba(245, 158, 11, 0.08)",
    borderWidth: theme.borderWidth[1],
    borderColor: "rgba(245, 158, 11, 0.35)",
    borderRadius: theme.borderRadius.md,
    overflow: "hidden",
    shadowColor: "#f59e0b",
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.12,
    shadowRadius: 8,
    elevation: 3,
  },
  bannerHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    gap: theme.spacing[2],
  },
  leftInfo: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    flex: 1,
    minWidth: 0,
  },
  iconCircle: {
    width: 28,
    height: 28,
    borderRadius: 14,
    backgroundColor: "rgba(245, 158, 11, 0.18)",
    justifyContent: "center",
    alignItems: "center",
  },
  titleColumn: {
    flex: 1,
    minWidth: 0,
  },
  bannerTitle: {
    fontSize: 13,
    fontWeight: "600",
    color: "#f59e0b",
  },
  subInfoRow: {
    flexDirection: "row",
    alignItems: "center",
    marginTop: 1,
  },
  subInfoText: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  rightActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  expandToggle: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: "rgba(245, 158, 11, 0.12)",
  },
  expandToggleText: {
    fontSize: 11,
    fontWeight: "500",
    color: "#f59e0b",
  },
  repairButton: {
    backgroundColor: "#d97706",
  },
  repairButtonText: {
    fontSize: 12,
    fontWeight: "600",
    color: "#ffffff",
  },
  closeBtn: {
    padding: 4,
  },
  detailsBody: {
    paddingHorizontal: theme.spacing[3],
    paddingBottom: theme.spacing[3],
    paddingTop: theme.spacing[1],
    borderTopWidth: theme.borderWidth[1],
    borderTopColor: "rgba(245, 158, 11, 0.15)",
    gap: theme.spacing[2],
  },
  commitLine: {
    fontSize: 11,
    color: theme.colors.foregroundMuted,
  },
  commitSha: {
    fontFamily: "monospace",
    color: theme.colors.foreground,
    fontWeight: "600",
  },
  logBox: {
    backgroundColor: "rgba(0, 0, 0, 0.45)",
    borderRadius: theme.borderRadius.sm,
    padding: theme.spacing[2],
    borderWidth: theme.borderWidth[1],
    borderColor: "rgba(255, 255, 255, 0.08)",
  },
  logText: {
    fontFamily: "monospace",
    fontSize: 11,
    lineHeight: 16,
    color: "#f87171",
  },
}));
