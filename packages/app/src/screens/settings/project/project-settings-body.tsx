import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import { useFocusEffect } from "expo-router/react-navigation";
import { StyleSheet } from "react-native-unistyles";
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
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import { ProjectEditSheet } from "@/components/project-edit-sheet";
import type { ProjectEditFormSnapshot } from "@/projects/edit-form";
import { useProjectIcons } from "@/projects/icons";
import { createProjectIconTarget } from "@/projects/icon-target";
import { useHostFeature } from "@/runtime/host-features";
import { useQuery } from "@tanstack/react-query";
import type { ProjectHostEntry, ProjectSummary } from "@/utils/projects";
import { ProjectConfigReadFailureCallout } from "./read-failure-callout";
import { ProjectConfigForm } from "./project-config-form";

const ICON_SIZE = 14;

type ReadProjectConfigData = Awaited<ReturnType<DaemonClient["readProjectConfig"]>>;

export function BackToProjectsButton({ onPress }: { onPress: () => void }) {
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

export function NoEditableTarget({
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

function revisionToKey(revision: PaseoConfigRevision | null): string {
  if (!revision) return "none";
  return `${revision.mtimeMs}-${revision.size}`;
}

export interface ProjectSettingsBodyProps {
  project: ProjectSummary;
  selectedHost: ProjectHostEntry;
  client: DaemonClient;
  isHostGone: boolean;
  onBackToProjects: () => void;
  showBackToProjects: boolean;
}

export function ProjectSettingsBody({
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
          <ToolbarButton
            testID="project-edit-button"
            label={t("settings.project.edit.title")}
            onPress={openEditSheet}
            hitSlop={8}
            style={styles.editButton}
          >
            <Pencil size={ICON_SIZE} color={styles.iconColor.color} />
          </ToolbarButton>
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
        <LoadingSpinner color={styles.spinnerColor.color} />
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
        <LoadingSpinner color={styles.spinnerColor.color} />
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
