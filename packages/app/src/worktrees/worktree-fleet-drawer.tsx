import React, { useCallback, useEffect, useMemo, useState } from "react";
import { Pressable, Text, View } from "react-native";
import { useTranslation } from "react-i18next";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Archive, Bot, Folder, GitBranch, Plus, RotateCw, Trash2 } from "lucide-react-native";
import type { Theme } from "@/styles/theme";
import { AdaptiveModalSheet, type SheetHeader } from "@/components/adaptive-modal-sheet";
import { Button } from "@/components/ui/button";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { StatusBadge, type StatusBadgeVariant } from "@/components/ui/status-badge";
import { Field, FormTextInput } from "@/components/ui/form-field";
import { confirmDialog } from "@/utils/confirm-dialog";
import { navigateToWorkspace } from "@/stores/navigation-active-workspace-store";
import { useSessionStore } from "@/stores/session-store";
import { useHostRuntimeClient } from "@/runtime/host-runtime";
import { useToast } from "@/contexts/toast-context";
import {
  type AgentSummaryInput,
  type WorktreeClient,
  type WorktreeFleetItem,
  type WorktreeFleetStatus,
  transformWorktrees,
} from "./types";

export interface WorktreeFleetDrawerProps {
  visible: boolean;
  onClose: () => void;
  cwd: string;
  serverId?: string;
  client?: WorktreeClient | null;
  onSwitchWorktree?: (worktree: WorktreeFleetItem) => void;
  testID?: string;
}

const foregroundMutedColor = (theme: Theme) => ({
  color: theme.colors.foregroundMuted,
});

const ThemedGitBranch = withUnistyles(GitBranch);
const ThemedFolder = withUnistyles(Folder);
const ThemedBot = withUnistyles(Bot);
const ThemedPlus = withUnistyles(Plus);
const ThemedRotateCw = withUnistyles(RotateCw);
const ThemedTrash = withUnistyles(Trash2);

function resolveStatusVariant(status: WorktreeFleetStatus): StatusBadgeVariant {
  switch (status) {
    case "active":
      return "success";
    case "running":
      return "warning";
    case "error":
      return "error";
    case "dirty":
      return "warning";
    case "idle":
    case "ready":
    default:
      return "muted";
  }
}

export function WorktreeFleetDrawer({
  visible,
  onClose,
  cwd,
  serverId = "",
  client: propClient,
  onSwitchWorktree,
  testID = "worktree-fleet-drawer",
}: WorktreeFleetDrawerProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const runtimeClient = useHostRuntimeClient(serverId);
  const client = propClient ?? (runtimeClient as unknown as WorktreeClient | null);

  // Read active session data to correlate workspaces and agents
  const workspacesMap = useSessionStore((state) => state.sessions[serverId]?.workspaces);
  const agentsMap = useSessionStore((state) => state.sessions[serverId]?.agents);

  const [isLoading, setIsLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [rawWorktrees, setRawWorktrees] = useState<
    Array<{
      worktreePath: string;
      createdAt: string;
      branchName?: string | null;
      head?: string | null;
    }>
  >([]);

  // New worktree form state
  const [isCreating, setIsCreating] = useState(false);
  const [worktreeSlug, setWorktreeSlug] = useState("");
  const [refName, setRefName] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  // In-flight archiving path
  const [archivingPath, setArchivingPath] = useState<string | null>(null);

  const sheetHeader = useMemo<SheetHeader>(
    () => ({
      title: t("worktrees.fleetTitle", "Worktree Fleet"),
    }),
    [t],
  );

  const fetchWorktrees = useCallback(async () => {
    if (!client || !cwd) {
      return;
    }

    setIsLoading(true);
    setLoadError(null);

    try {
      const listFn = client.listPaseoWorktrees ?? client.getPaseoWorktreeList;
      if (!listFn) {
        throw new Error("Worktree client does not support listing worktrees");
      }

      const result = await listFn.call(client, { cwd });
      if (result.error) {
        const errorMsg =
          typeof result.error === "string"
            ? result.error
            : result.error && typeof result.error === "object" && "message" in result.error
              ? String(result.error.message)
              : "Failed to list worktrees";
        setLoadError(errorMsg);
      } else {
        setRawWorktrees(result.worktrees ?? []);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : "Error listing worktrees";
      setLoadError(message);
    } finally {
      setIsLoading(false);
    }
  }, [client, cwd]);

  useEffect(() => {
    if (visible) {
      void fetchWorktrees();
    }
  }, [visible, fetchWorktrees]);

  // Transform raw worktree data into rich fleet items
  const worktreeItems = useMemo<WorktreeFleetItem[]>(() => {
    const workspacesList = workspacesMap ? Array.from(workspacesMap.values()) : [];
    const agentsList: AgentSummaryInput[] = agentsMap
      ? Array.from(agentsMap.values()).map((a) => ({
          id: a.id,
          cwd: a.cwd ?? "",
          workspaceId: a.workspaceId,
          status: typeof a.status === "string" ? a.status : "idle",
          title: a.title ?? null,
          provider: a.provider ?? undefined,
          turn: undefined,
          model: a.model ?? null,
        }))
      : [];

    return transformWorktrees({
      rawWorktrees,
      currentCwd: cwd,
      workspaces: workspacesList,
      agents: agentsList,
    });
  }, [rawWorktrees, cwd, workspacesMap, agentsMap]);

  const handleCreateWorktree = useCallback(async () => {
    const trimmedSlug = worktreeSlug.trim();
    if (!trimmedSlug) {
      setCreateError(t("worktrees.slugRequired", "Worktree name / slug is required"));
      return;
    }

    if (!client) {
      setCreateError("Client unavailable");
      return;
    }

    setIsSubmitting(true);
    setCreateError(null);

    try {
      const trimmedRef = refName.trim() || undefined;
      const result = await client.createPaseoWorktree({
        cwd,
        worktreeSlug: trimmedSlug,
        refName: trimmedRef,
      });

      if (result.error) {
        setCreateError(result.error);
      } else {
        toast.show(
          t("worktrees.createdSuccess", {
            defaultValue: `Worktree "${trimmedSlug}" created successfully`,
            slug: trimmedSlug,
          }),
        );
        setWorktreeSlug("");
        setRefName("");
        setIsCreating(false);
        await fetchWorktrees();
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Failed to create worktree";
      setCreateError(msg);
    } finally {
      setIsSubmitting(false);
    }
  }, [client, cwd, worktreeSlug, refName, t, toast, fetchWorktrees]);

  const handleSwitchWorktree = useCallback(
    (item: WorktreeFleetItem) => {
      if (onSwitchWorktree) {
        onSwitchWorktree(item);
      } else if (item.workspaceId && serverId) {
        navigateToWorkspace({ serverId, workspaceId: item.workspaceId });
      }
      onClose();
    },
    [onSwitchWorktree, serverId, onClose],
  );

  const handleArchiveWorktree = useCallback(
    async (item: WorktreeFleetItem) => {
      if (!client) return;

      const confirmed = await confirmDialog({
        title: t("worktrees.archiveConfirmTitle", "Archive Worktree"),
        message: t("worktrees.archiveConfirmMessage", {
          defaultValue: `Are you sure you want to archive worktree "${item.branchName}"? This will clean up the worktree path.`,
          branch: item.branchName,
        }),
        confirmLabel: t("common.actions.archive", "Archive"),
        destructive: true,
      });

      if (!confirmed) {
        return;
      }

      setArchivingPath(item.worktreePath);

      try {
        const result = await client.archivePaseoWorktree({
          worktreePath: item.worktreePath,
          repoRoot: cwd,
          branchName: item.branchName,
          workspaceId: item.workspaceId,
        });

        if (result.success) {
          toast.show(
            t("worktrees.archiveSuccess", {
              defaultValue: `Worktree "${item.branchName}" archived`,
              branch: item.branchName,
            }),
          );
          await fetchWorktrees();
        } else {
          const errMessage =
            result.error && typeof result.error === "object" && "message" in result.error
              ? String(result.error.message)
              : "Failed to archive worktree";
          toast.show(errMessage);
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : "Error archiving worktree";
        toast.show(msg);
      } finally {
        setArchivingPath(null);
      }
    },
    [client, cwd, t, toast, fetchWorktrees],
  );

  return (
    <AdaptiveModalSheet
      header={sheetHeader}
      visible={visible}
      onClose={onClose}
      desktopMaxWidth={680}
      testID={testID}
    >
      <View style={styles.container}>
        {/* Top Controls Bar */}
        <View style={styles.topBar}>
          <View style={styles.titleSection}>
            <Text style={styles.subheadingText}>
              {t("worktrees.fleetDescription", "Active repository worktrees and agent workspaces")}
            </Text>
          </View>
          <View style={styles.topActions}>
            <Button
              size="sm"
              variant="outline"
              leftIcon={<ThemedRotateCw size={14} uniProps={foregroundMutedColor} />}
              onPress={fetchWorktrees}
              loading={isLoading}
              testID="refresh-worktrees-button"
            >
              {t("common.actions.refresh", "Refresh")}
            </Button>
            <Button
              size="sm"
              variant="default"
              leftIcon={<ThemedPlus size={14} uniProps={foregroundMutedColor} />}
              onPress={() => {
                setIsCreating((prev) => !prev);
                setCreateError(null);
              }}
              testID="new-worktree-button"
            >
              {isCreating
                ? t("common.actions.cancel", "Cancel")
                : t("worktrees.newWorktree", "New Worktree")}
            </Button>
          </View>
        </View>

        {/* New Worktree Creation Form */}
        {isCreating ? (
          <View style={styles.createFormCard} testID="new-worktree-form">
            <Text style={styles.createFormTitle}>
              {t("worktrees.createFormTitle", "Create New Worktree")}
            </Text>

            <Field
              label={t("worktrees.slugLabel", "Worktree Name / Slug")}
              hint={t("worktrees.slugHint", "Identifier used for branch and directory")}
              testID="worktree-slug-field"
            >
              <FormTextInput
                initialValue={worktreeSlug}
                onChangeText={setWorktreeSlug}
                placeholder="e.g. feature-login, review-pr-42"
                testID="worktree-slug-input"
              />
            </Field>

            <Field
              label={t("worktrees.refLabel", "Base Branch / Ref (Optional)")}
              hint={t("worktrees.refHint", "Source ref to branch off from (defaults to HEAD)")}
              testID="worktree-ref-field"
            >
              <FormTextInput
                initialValue={refName}
                onChangeText={setRefName}
                placeholder="e.g. main"
                testID="worktree-ref-input"
              />
            </Field>

            {createError ? (
              <View style={styles.errorBox} testID="create-worktree-error">
                <Text style={styles.errorText}>{createError}</Text>
              </View>
            ) : null}

            <View style={styles.createFormActions}>
              <Button
                size="sm"
                variant="ghost"
                onPress={() => {
                  setIsCreating(false);
                  setCreateError(null);
                }}
                disabled={isSubmitting}
                testID="create-worktree-cancel-button"
              >
                {t("common.actions.cancel", "Cancel")}
              </Button>
              <Button
                size="sm"
                variant="default"
                onPress={handleCreateWorktree}
                loading={isSubmitting}
                testID="create-worktree-submit-button"
              >
                {t("worktrees.createButton", "Create Worktree")}
              </Button>
            </View>
          </View>
        ) : null}

        {/* Worktree Items List */}
        {isLoading && worktreeItems.length === 0 ? (
          <View style={styles.loadingContainer}>
            <LoadingSpinner size="small" color="#888" />
            <Text style={styles.loadingText}>
              {t("worktrees.loading", "Loading active worktrees...")}
            </Text>
          </View>
        ) : loadError ? (
          <View style={styles.errorContainer}>
            <Text style={styles.errorText}>{loadError}</Text>
            <Button size="sm" variant="outline" onPress={fetchWorktrees}>
              {t("common.actions.retry", "Retry")}
            </Button>
          </View>
        ) : worktreeItems.length === 0 ? (
          <View style={styles.emptyContainer} testID="worktrees-empty-state">
            <ThemedGitBranch size={32} uniProps={foregroundMutedColor} />
            <Text style={styles.emptyTitle}>
              {t("worktrees.noWorktrees", "No active worktrees found")}
            </Text>
            <Text style={styles.emptySubtitle}>
              {t(
                "worktrees.emptyDescription",
                "Create a separate worktree to develop features or run agents in isolated checkouts.",
              )}
            </Text>
          </View>
        ) : (
          <View style={styles.itemsList}>
            {worktreeItems.map((item) => {
              const statusVariant = resolveStatusVariant(item.status);
              const isArchiving = archivingPath === item.worktreePath;

              return (
                <View
                  key={item.worktreePath}
                  style={[styles.worktreeCard, item.isCurrent && styles.worktreeCardCurrent]}
                  testID={`worktree-item-${item.worktreePath}`}
                >
                  <View style={styles.cardHeader}>
                    <View style={styles.branchHeaderLeft}>
                      <ThemedGitBranch size={16} uniProps={foregroundMutedColor} />
                      <Text
                        style={styles.branchNameText}
                        testID={`worktree-branch-${item.branchName}`}
                        numberOfLines={1}
                      >
                        {item.branchName}
                      </Text>
                      {item.isCurrent ? (
                        <StatusBadge
                          label={t("worktrees.currentBadge", "Current")}
                          variant="success"
                        />
                      ) : null}
                    </View>
                    <StatusBadge label={item.status.toUpperCase()} variant={statusVariant} />
                  </View>

                  <View style={styles.pathRow}>
                    <ThemedFolder size={12} uniProps={foregroundMutedColor} />
                    <Text
                      style={styles.pathText}
                      numberOfLines={1}
                      testID={`worktree-path-${item.worktreePath}`}
                    >
                      {item.worktreePath}
                    </Text>
                  </View>

                  {/* Associated Agent Indicators */}
                  <View
                    style={styles.agentsContainer}
                    testID={`worktree-agents-${item.worktreePath}`}
                  >
                    {item.agents.length > 0 ? (
                      item.agents.map((agent) => (
                        <View key={agent.id} style={styles.agentChip}>
                          <ThemedBot size={12} uniProps={foregroundMutedColor} />
                          <Text style={styles.agentTitleText} numberOfLines={1}>
                            {agent.title || agent.id.slice(0, 8)}
                          </Text>
                          <View
                            style={[
                              styles.agentStatusDot,
                              agent.status === "running" || (agent.turn && agent.turn !== "idle")
                                ? styles.agentStatusRunning
                                : styles.agentStatusIdle,
                            ]}
                          />
                        </View>
                      ))
                    ) : (
                      <Text style={styles.noAgentsText}>
                        {t("worktrees.noAgents", "No active agents")}
                      </Text>
                    )}
                  </View>

                  {/* Actions Row */}
                  <View style={styles.cardActions}>
                    <Button
                      size="sm"
                      variant={item.isCurrent ? "outline" : "default"}
                      disabled={item.isCurrent}
                      onPress={() => handleSwitchWorktree(item)}
                      testID={`switch-worktree-button-${item.worktreePath}`}
                    >
                      {item.isCurrent
                        ? t("worktrees.activeCheckout", "Current Checkout")
                        : t("worktrees.switchToWorktree", "Switch to Worktree")}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      leftIcon={<ThemedTrash size={14} uniProps={foregroundMutedColor} />}
                      onPress={() => handleArchiveWorktree(item)}
                      loading={isArchiving}
                      testID={`archive-worktree-button-${item.worktreePath}`}
                    >
                      {t("common.actions.archive", "Archive")}
                    </Button>
                  </View>
                </View>
              );
            })}
          </View>
        )}
      </View>
    </AdaptiveModalSheet>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    paddingHorizontal: theme.spacing[4],
    paddingBottom: theme.spacing[6],
    gap: theme.spacing[4],
  },
  topBar: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingBottom: theme.spacing[2],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  titleSection: {
    flex: 1,
    marginRight: theme.spacing[2],
  },
  subheadingText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  topActions: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  createFormCard: {
    backgroundColor: theme.colors.surface2,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[4],
    gap: theme.spacing[3],
  },
  createFormTitle: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.semibold,
    color: theme.colors.foreground,
  },
  createFormActions: {
    flexDirection: "row",
    justifyContent: "flex-end",
    alignItems: "center",
    gap: theme.spacing[2],
    marginTop: theme.spacing[2],
  },
  errorBox: {
    backgroundColor: "rgba(239, 68, 68, 0.1)",
    borderColor: theme.colors.palette.red[300],
    borderWidth: 1,
    borderRadius: theme.borderRadius.md,
    padding: theme.spacing[2],
  },
  errorText: {
    color: theme.colors.palette.red[300],
    fontSize: theme.fontSize.sm,
  },
  loadingContainer: {
    alignItems: "center",
    justifyContent: "center",
    paddingVertical: theme.spacing[8],
    gap: theme.spacing[2],
  },
  loadingText: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.sm,
  },
  errorContainer: {
    alignItems: "center",
    justifyContent: "center",
    paddingVertical: theme.spacing[6],
    gap: theme.spacing[3],
  },
  emptyContainer: {
    alignItems: "center",
    justifyContent: "center",
    paddingVertical: theme.spacing[8],
    gap: theme.spacing[2],
  },
  emptyTitle: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.medium,
    color: theme.colors.foreground,
  },
  emptySubtitle: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    textAlign: "center",
    maxWidth: 400,
  },
  itemsList: {
    gap: theme.spacing[3],
  },
  worktreeCard: {
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.lg,
    borderWidth: 1,
    borderColor: theme.colors.border,
    padding: theme.spacing[4],
    gap: theme.spacing[2],
  },
  worktreeCardCurrent: {
    borderColor: theme.colors.borderAccent,
    backgroundColor: theme.colors.surface2,
  },
  cardHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  branchHeaderLeft: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    flex: 1,
  },
  branchNameText: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.semibold,
    color: theme.colors.foreground,
    maxWidth: "70%",
  },
  pathRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1.5],
  },
  pathText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    fontFamily: "monospace",
  },
  agentsContainer: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: theme.spacing[1.5],
    alignItems: "center",
  },
  agentChip: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: theme.colors.surface3,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 2,
    borderRadius: theme.borderRadius.full,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  agentTitleText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    maxWidth: 140,
  },
  agentStatusDot: {
    width: 6,
    height: 6,
    borderRadius: 3,
  },
  agentStatusRunning: {
    backgroundColor: "#10b981",
  },
  agentStatusIdle: {
    backgroundColor: theme.colors.foregroundMuted,
  },
  noAgentsText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    fontStyle: "italic",
  },
  cardActions: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "flex-end",
    gap: theme.spacing[2],
    paddingTop: theme.spacing[1],
  },
}));
