import { useMemo } from "react";
import { useProjects } from "@/hooks/use-projects";
import { useHostRuntimeClient, useHostRuntimeSnapshot } from "@/runtime/host-runtime";
import { getProjectHostEntry, getProjectSummaryForHostProject } from "@/utils/projects";
import { NoEditableTarget, ProjectSettingsBody } from "./settings/project/project-settings-body";

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
