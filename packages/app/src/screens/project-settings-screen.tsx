import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Pressable, Text, View } from "react-native";
import { useFocusEffect } from "expo-router/react-navigation";
import { StyleSheet } from "react-native-unistyles";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Pencil } from "lucide-react-native";
import { ProjectIconView } from "@/components/project-icon-view";
import type {
  PaseoConfigRaw,
  PaseoConfigRevision,
  ProjectConfigRpcError,
} from "@getpaseo/protocol/messages";
import type { DaemonClient } from "@getpaseo/client/internal/daemon-client";
import { Button } from "@/components/ui/button";
import { LoadingSpinner } from "@/components/ui/loading-spinner";
import { ProjectEditSheet } from "@/components/project-edit-sheet";
import { useProjects } from "@/hooks/use-projects";
import type { ProjectEditFormSnapshot } from "@/projects/edit-form";
import { useProjectIcons } from "@/projects/icons";
import { createProjectIconTarget } from "@/projects/icon-target";
import { useHostRuntimeClient, useHostRuntimeSnapshot } from "@/runtime/host-runtime";
import { useHostFeature } from "@/runtime/host-features";
import { useToast } from "@/contexts/toast-context";
import { confirmDialog } from "@/utils/confirm-dialog";
import {
  applyDraftToConfig,
  configToDraft,
  type LifecycleOriginalKind,
  type MetadataPromptKey,
  type ProjectConfigDraft,
  type ProjectScriptDraft,
} from "@/utils/project-config-form";
import {
  getProjectHostEntry,
  getProjectSummaryForHostProject,
  type ProjectHostEntry,
  type ProjectSummary,
} from "@/utils/projects";
import { ProjectWorktreeSection } from "./settings/project/worktree-section";
import { ProjectMetadataSection } from "./settings/project/metadata-section";
import { ProjectScriptsSection } from "./settings/project/scripts-section";
import { ScriptEditModal } from "./settings/project/script-edit-modal";
import { validateScript } from "./settings/project/script-validation";
import { ProjectConfigSaveActions } from "./settings/project/save-actions";
import { ProjectConfigReadFailureCallout } from "./settings/project/read-failure-callout";

const ICON_SIZE = 14;

type ReadProjectConfigData = Awaited<ReturnType<DaemonClient["readProjectConfig"]>>;

export interface ProjectSettingsScreenProps {
  serverId: string;
  projectId: string;
  onBackToProjects: () => void;
  showBackToProjects: boolean;
}

export default function ProjectSettingsScreen({
  serverId,
  projectId,
  onBackToProjects,
  showBackToProjects,
}: ProjectSettingsScreenProps) {
  const { projects } = useProjects();
  const project = useMemo(
    () => getProjectSummaryForHostProject(projects, serverId, projectId),
    [projectId, projects, serverId],
  );
  const selectedHost = getProjectHostEntry(project, serverId, projectId);
  const selectedSnapshot = useHostRuntimeSnapshot(serverId);
  const isHostGone =
    Boolean(serverId) &&
    (selectedSnapshot?.connectionStatus === "offline" ||
      selectedSnapshot?.connectionStatus === "error");

  const client = useHostRuntimeClient(serverId);
  const canEdit =
    selectedHost?.isOnline === true &&
    selectedHost.serverId.trim().length > 0 &&
    selectedHost.repoRoot.trim().length > 0;

  if (!project || !selectedHost || !client || !canEdit) {
    return (
      <NoEditableTarget
        onBackToProjects={onBackToProjects}
        showBackToProjects={showBackToProjects}
      />
    );
  }

  return (
    <ProjectSettingsBody
      project={project}
      selectedHost={selectedHost}
      client={client}
      isHostGone={isHostGone}
      onBackToProjects={onBackToProjects}
      showBackToProjects={showBackToProjects}
    />
  );
}

function NoEditableTarget({
  onBackToProjects,
  showBackToProjects,
}: {
  onBackToProjects: () => void;
  showBackToProjects: boolean;
}) {
  const { t } = useTranslation();
  return (
    <View style={styles.noTargetContainer}>
      {showBackToProjects ? <BackToProjectsButton onPress={onBackToProjects} /> : null}
      <Text style={styles.noTargetText}>{t("settings.project.noEditableTarget")}</Text>
      {showBackToProjects ? (
        <Button
          testID="project-settings-back-button"
          onPress={onBackToProjects}
          variant="secondary"
          size="md"
        >
          {t("settings.project.backToProjects")}
        </Button>
      ) : null}
    </View>
  );
}

function BackToProjectsButton({ onPress }: { onPress: () => void }) {
  const { t } = useTranslation();
  return (
    <Button
      testID="project-settings-back-link"
      accessibilityLabel={t("settings.project.backToProjects")}
      onPress={onPress}
      variant="ghost"
      size="sm"
      leftIcon={ArrowLeft}
      style={styles.backButton}
    >
      {t("settings.project.backToProjects")}
    </Button>
  );
}

interface ProjectSettingsBodyProps {
  project: ProjectSummary;
  selectedHost: ProjectHostEntry;
  client: DaemonClient;
  isHostGone: boolean;
  onBackToProjects: () => void;
  showBackToProjects: boolean;
}

function ProjectSettingsBody({
  project,
  selectedHost,
  client,
  isHostGone,
  onBackToProjects,
  showBackToProjects,
}: ProjectSettingsBodyProps) {
  const { t } = useTranslation();
  const [isEditSheetOpen, setIsEditSheetOpen] = useState(false);
  const [editSessionId, setEditSessionId] = useState(0);
  const openEditSheet = useCallback(() => {
    setEditSessionId((id) => id + 1);
    setIsEditSheetOpen(true);
  }, []);
  const closeEditSheet = useCallback(() => setIsEditSheetOpen(false), []);
  const queryKey = useMemo(
    () => ["project-config", selectedHost.serverId, selectedHost.repoRoot] as const,
    [selectedHost.serverId, selectedHost.repoRoot],
  );

  const readQuery = useQuery({
    queryKey,
    queryFn: () => client.readProjectConfig(selectedHost.repoRoot),
    retry: false,
  });
  const refetchProjectConfig = readQuery.refetch;
  useFocusEffect(
    useCallback(() => {
      void refetchProjectConfig();
    }, [refetchProjectConfig]),
  );

  const data = readQuery.data;
  const supportsCustomIcon = useHostFeature(selectedHost.serverId, "projectCustomIcon");
  const customIconRevision = selectedHost.customIconRevision ?? null;
  const projectIconTargets = useMemo(() => {
    const target = createProjectIconTarget({
      projectViewKey: project.viewKey,
      placement: { ...selectedHost, iconWorkingDir: selectedHost.repoRoot },
    });
    return target ? [target] : [];
  }, [project.viewKey, selectedHost]);
  const projectIcons = useProjectIcons({ projects: projectIconTargets });
  const projectIconDataUri = projectIcons.get(project.viewKey) ?? null;
  const editSnapshot = useMemo<ProjectEditFormSnapshot>(
    () => ({
      projectName: selectedHost.projectName,
      projectCustomName: selectedHost.projectCustomName,
      hasCustomIcon: customIconRevision !== null,
      currentIconDataUri: projectIconDataUri,
    }),
    [
      customIconRevision,
      projectIconDataUri,
      selectedHost.projectCustomName,
      selectedHost.projectName,
    ],
  );
  const loadedConfig: PaseoConfigRaw | null = data?.ok ? (data.config ?? {}) : null;
  const loadedRevision: PaseoConfigRevision | null = data?.ok ? data.revision : null;
  const hasUncommittedWorktreeSetupChanges =
    data?.ok === true && data.hasUncommittedWorktreeSetupChanges === true;
  const readError: ProjectConfigRpcError | null = data && !data.ok ? data.error : null;

  const handleReload = useCallback(() => {
    void readQuery.refetch();
  }, [readQuery]);

  return (
    <View role="main" style={styles.body}>
      {showBackToProjects ? <BackToProjectsButton onPress={onBackToProjects} /> : null}

      <View style={styles.headerBlock}>
        <View style={styles.titleRow}>
          <ProjectTitleIcon
            iconDataUri={projectIconDataUri}
            projectName={selectedHost.projectName}
            projectViewKey={project.viewKey}
          />
          <Text style={styles.projectTitle} numberOfLines={1}>
            {selectedHost.projectName}
          </Text>
          <Pressable
            testID="project-edit-button"
            accessibilityRole="button"
            accessibilityLabel={t("settings.project.edit.title")}
            onPress={openEditSheet}
            hitSlop={8}
            style={styles.editButton}
          >
            <Pencil size={ICON_SIZE} color={styles.iconColor.color} />
          </Pressable>
        </View>
      </View>

      <ProjectEditSheet
        // A new instance per open seeds the form from the project as it stands now.
        key={`${selectedHost.serverId}:${selectedHost.projectId}:${editSessionId}`}
        visible={isEditSheetOpen}
        onClose={closeEditSheet}
        serverId={selectedHost.serverId}
        projectId={selectedHost.projectId}
        projectViewKey={project.viewKey}
        client={client}
        supportsCustomIcon={supportsCustomIcon}
        snapshot={editSnapshot}
      />

      {renderContent({
        readQuery,
        loadedConfig,
        loadedRevision,
        hasUncommittedWorktreeSetupChanges,
        readError,
        selectedHost,
        queryKey,
        client,
        onReload: handleReload,
        isHostGone,
        onBackToProjects,
        showBackToProjects,
      })}
    </View>
  );
}

interface RenderContentInput {
  readQuery: ReturnType<typeof useQuery<ReadProjectConfigData>>;
  loadedConfig: PaseoConfigRaw | null;
  loadedRevision: PaseoConfigRevision | null;
  hasUncommittedWorktreeSetupChanges: boolean;
  readError: ProjectConfigRpcError | null;
  selectedHost: ProjectHostEntry;
  queryKey: readonly [string, string, string];
  client: DaemonClient;
  onReload: () => void;
  isHostGone: boolean;
  onBackToProjects: () => void;
  showBackToProjects: boolean;
}

function renderContent({
  readQuery,
  loadedConfig,
  loadedRevision,
  hasUncommittedWorktreeSetupChanges,
  readError,
  selectedHost,
  queryKey,
  client,
  onReload,
  isHostGone,
  onBackToProjects,
  showBackToProjects,
}: RenderContentInput) {
  if (readQuery.isLoading) {
    return (
      <View style={styles.centered}>
        <LoadingSpinner color={ResolveSpinnerColor()} />
      </View>
    );
  }

  if (readQuery.isError) {
    return (
      <ProjectConfigReadFailureCallout
        kind="transport"
        error={readQuery.error}
        onReload={onReload}
      />
    );
  }

  if (readError) {
    return (
      <ProjectConfigReadFailureCallout kind={readError.code} error={null} onReload={onReload} />
    );
  }

  if (isHostGone) {
    return (
      <NoEditableTarget
        onBackToProjects={onBackToProjects}
        showBackToProjects={showBackToProjects}
      />
    );
  }

  if (!loadedConfig) {
    return (
      <View style={styles.centered}>
        <LoadingSpinner color={ResolveSpinnerColor()} />
      </View>
    );
  }

  const formKey = `${selectedHost.serverId}::${selectedHost.repoRoot}::${revisionToKey(loadedRevision)}`;
  return (
    <ProjectConfigForm
      key={formKey}
      baseConfig={loadedConfig}
      revision={loadedRevision}
      hasUncommittedWorktreeSetupChanges={hasUncommittedWorktreeSetupChanges}
      repoRoot={selectedHost.repoRoot}
      queryKey={queryKey}
      client={client}
      onReload={onReload}
    />
  );
}

function revisionToKey(revision: PaseoConfigRevision | null): string {
  if (!revision) return "none";
  return `${revision.mtimeMs}-${revision.size}`;
}

interface ProjectConfigFormProps {
  baseConfig: PaseoConfigRaw;
  revision: PaseoConfigRevision | null;
  hasUncommittedWorktreeSetupChanges: boolean;
  repoRoot: string;
  queryKey: readonly [string, string, string];
  client: DaemonClient;
  onReload: () => void;
}

function ProjectConfigForm({
  baseConfig,
  revision,
  hasUncommittedWorktreeSetupChanges,
  repoRoot,
  queryKey,
  client,
  onReload,
}: ProjectConfigFormProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const toast = useToast();

  const [draft, setDraft] = useState<ProjectConfigDraft>(() => configToDraft(baseConfig));
  const [writeError, setWriteError] = useState<ProjectConfigRpcError | null>(null);
  const [editingScriptId, setEditingScriptId] = useState<string | null>(null);

  const saveMutation = useMutation({
    mutationFn: async (input: {
      config: PaseoConfigRaw;
      expectedRevision: PaseoConfigRevision | null;
    }) => {
      return client.writeProjectConfig({
        repoRoot,
        config: input.config,
        expectedRevision: input.expectedRevision,
      });
    },
    onSuccess: (result) => {
      if (result.ok) {
        queryClient.setQueryData<ReadProjectConfigData>(queryKey, {
          ok: true,
          config: result.config,
          revision: result.revision,
          requestId: "local-cache",
          repoRoot,
          ...(result.hasUncommittedWorktreeSetupChanges === undefined
            ? {}
            : {
                hasUncommittedWorktreeSetupChanges: result.hasUncommittedWorktreeSetupChanges,
              }),
        });
        setWriteError(null);
        queryClient.invalidateQueries({ queryKey: ["projects"] });
        toast.show(t("settings.project.actions.saved"), { variant: "success" });
      } else {
        setWriteError(result.error);
      }
    },
  });

  const handleSave = useCallback(() => {
    if (writeError?.code === "stale_project_config") return;
    const config = applyDraftToConfig({ draft, base: baseConfig });
    saveMutation.mutate({ config, expectedRevision: revision });
  }, [draft, baseConfig, revision, writeError, saveMutation]);

  const handleReload = useCallback(() => {
    setWriteError(null);
    onReload();
  }, [onReload]);

  const updateDraft = useCallback((updater: (draft: ProjectConfigDraft) => ProjectConfigDraft) => {
    setDraft((prev) => updater(prev));
  }, []);

  const handleSetupChange = useCallback(
    (text: string) => updateDraft((d) => ({ ...d, setupText: text })),
    [updateDraft],
  );
  const handleTeardownChange = useCallback(
    (text: string) => updateDraft((d) => ({ ...d, teardownText: text })),
    [updateDraft],
  );

  const handleMetadataPromptChange = useCallback(
    (key: MetadataPromptKey, text: string) =>
      updateDraft((d) => ({
        ...d,
        metadataPrompts: { ...d.metadataPrompts, [key]: text },
      })),
    [updateDraft],
  );

  const handleRemoveScript = useCallback(
    async (script: ProjectScriptDraft) => {
      const ok = await confirmDialog({
        title: t("settings.project.scripts.removeTitle"),
        message: t("settings.project.scripts.removeMessage", {
          name: script.name || t("settings.project.scripts.removeFallbackName"),
        }),
        confirmLabel: t("settings.project.scripts.actions.remove"),
        cancelLabel: t("settings.project.actions.cancel"),
        destructive: true,
      });
      if (!ok) return;
      updateDraft((d) => ({
        ...d,
        scripts: d.scripts.filter((entry) => entry.id !== script.id),
      }));
    },
    [t, updateDraft],
  );

  const handleEditScript = useCallback((script: ProjectScriptDraft) => {
    setEditingScriptId(script.id);
  }, []);

  const handleAddScript = useCallback(() => {
    const id = `script-draft-new-${Date.now()}`;
    updateDraft((d) => ({
      ...d,
      scripts: [
        ...d.scripts,
        {
          id,
          name: "",
          commandText: "",
          commandOriginalKind: "missing" satisfies LifecycleOriginalKind,
          type: "",
          portText: "",
          rawEntry: {},
        },
      ],
    }));
    setEditingScriptId(id);
  }, [updateDraft]);

  const handleEditingDraftChange = useCallback(
    (next: ProjectScriptDraft) => {
      updateDraft((d) => ({
        ...d,
        scripts: d.scripts.map((entry) => (entry.id === next.id ? next : entry)),
      }));
    },
    [updateDraft],
  );

  const handleCancelEditing = useCallback(() => {
    if (!editingScriptId) {
      return;
    }
    updateDraft((d) => {
      const entry = d.scripts.find((row) => row.id === editingScriptId);
      if (!entry) return d;
      const isEmpty =
        entry.name.trim().length === 0 &&
        entry.commandText.trim().length === 0 &&
        entry.type.trim().length === 0 &&
        entry.portText.trim().length === 0;
      if (!isEmpty) return d;
      return { ...d, scripts: d.scripts.filter((row) => row.id !== editingScriptId) };
    });
    setEditingScriptId(null);
  }, [editingScriptId, updateDraft]);

  const handleSaveEditing = useCallback(() => {
    setEditingScriptId(null);
  }, []);

  const editingScript = draft.scripts.find((entry) => entry.id === editingScriptId);

  const hasInvalidScripts = useMemo(
    () => draft.scripts.some((script) => validateScript(script, t).hasErrors),
    [draft.scripts, t],
  );

  return (
    <View>
      <ProjectWorktreeSection
        setupText={draft.setupText}
        teardownText={draft.teardownText}
        hasUncommittedWorktreeSetupChanges={hasUncommittedWorktreeSetupChanges}
        onSetupChange={handleSetupChange}
        onTeardownChange={handleTeardownChange}
      />

      <ProjectScriptsSection
        scripts={draft.scripts}
        onAdd={handleAddScript}
        onEdit={handleEditScript}
        onRemove={handleRemoveScript}
      />

      <ProjectMetadataSection
        metadataPrompts={draft.metadataPrompts}
        onChange={handleMetadataPromptChange}
      />

      <ProjectConfigSaveActions
        writeError={writeError}
        isPending={saveMutation.isPending}
        hasInvalidScripts={hasInvalidScripts}
        onSave={handleSave}
        onReload={handleReload}
      />

      {editingScript ? (
        <ScriptEditModal
          script={editingScript}
          onChange={handleEditingDraftChange}
          onCancel={handleCancelEditing}
          onSave={handleSaveEditing}
        />
      ) : null}
    </View>
  );
}

function ResolveSpinnerColor(): string {
  return styles.spinnerColor.color;
}

function ProjectTitleIcon({
  iconDataUri,
  projectName,
  projectViewKey,
}: {
  iconDataUri: string | null;
  projectName: string;
  projectViewKey: string;
}) {
  const initial = projectName.trim().charAt(0).toUpperCase() || "?";
  return (
    <ProjectIconView
      iconDataUri={iconDataUri}
      initial={initial}
      projectViewKey={projectViewKey}
      size={28}
      textStyle={styles.titleIconFallbackText}
    />
  );
}

const styles = StyleSheet.create((theme) => ({
  noTargetContainer: {
    padding: theme.spacing[4],
    alignItems: "flex-start",
    gap: theme.spacing[3],
  },
  noTargetText: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
  body: {
    padding: theme.spacing[4],
    gap: theme.spacing[2],
  },
  backButton: {
    alignSelf: "flex-start",
    paddingHorizontal: 0,
  },
  headerBlock: {
    marginTop: theme.spacing[2],
    marginBottom: theme.spacing[4],
    gap: theme.spacing[2],
  },
  titleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[3],
  },
  projectTitle: {
    color: theme.colors.foreground,
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.medium,
    flexShrink: 1,
  },
  editButton: {
    padding: theme.spacing[1],
  },
  titleIconFallbackText: {
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.medium,
  },
  iconColor: {
    color: theme.colors.foregroundMuted,
  },
  centered: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    padding: theme.spacing[6],
  },
  spinnerColor: {
    color: theme.colors.foregroundMuted,
  },
}));
