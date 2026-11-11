import { useCallback, useMemo, useReducer, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Text, type View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { createNameId } from "mnemonic-id";
import { useQueryClient } from "@tanstack/react-query";
import { FileDropZone } from "@/components/file-drop/file-drop-zone";
import { useIsCompactFormFactor } from "@/constants/layout";
import { useToast } from "@/contexts/toast-context";
import { useAgentInputDraft } from "@/composer/draft/input-draft";
import {
  getHostRuntimeStore,
  useHostRuntimeClient,
  useHostRuntimeIsConnected,
} from "@/runtime/host-runtime";
import { useHostFeature } from "@/runtime/host-features";
import type { WorkspaceDescriptor } from "@/stores/session-store";
import { buildNewWorkspaceDraftKey, generateDraftId } from "@/stores/draft-keys";
import { useOpenAddProject } from "@/hooks/use-open-add-project";
import { isActiveCreateFlowForDraft, useCreateFlowStore } from "@/stores/create-flow-store";
import { useKeyboardActionHandler } from "@/hooks/use-keyboard-action-handler";
import type { KeyboardActionId } from "@/keyboard/keyboard-action-dispatcher";
import { useAddHost } from "@/hosts/add-host";
import { getWorktreeSupportForHostProject } from "@/projects/host-projects";
import { useProjectIcons } from "@/projects/icons";
import { useDraftWorkspaceAttachmentScopeKey } from "@/attachments/workspace-attachments-store";
import type { ForgeSearchItem } from "@getpaseo/protocol/messages";
import {
  clearPickerPrAttachmentForTargetChange,
  initialPickerSelectionState,
  reducePickerSelection,
} from "./new-workspace-picker-state";
import { buildNewWorkspaceProjectIconTargets } from "./new-workspace/project-icon-targets";
import { useNewWorkspaceProjectPicker } from "./new-workspace/project-picker";
import { useNewWorkspaceFormStack } from "./new-workspace/use-new-workspace-form-stack";
import { NewWorkspaceLayout } from "./new-workspace/layout";
import { NewWorkspaceComposerView } from "./new-workspace/new-workspace-composer-view";
import { useImportSession } from "@/hooks/use-import-session";
import { useNewWorkspaceScreenPresence } from "./new-workspace/screen-presence";
import {
  buildComposerConfig,
  usePendingWorkspaceDraftSetup,
} from "./new-workspace/agent-submission";
import { useWorkspaceIsolation } from "./new-workspace/ref-picker-state";
import { useNewWorkspaceInitialContext } from "./new-workspace/initial-context-hook";
import { useNewWorkspaceComposerEditing } from "./new-workspace/composer-editing";
import { useNewWorkspaceLaunchTarget } from "./new-workspace/use-new-workspace-launch-target";
import { useNewWorkspacePicker } from "./new-workspace/use-new-workspace-picker";
import { useNewWorkspaceSubmit } from "./new-workspace/use-new-workspace-submit";

function useIsNewWorkspaceDraftHandoffActive(input: {
  draftId: string | undefined;
  selectedServerId: string;
}): boolean {
  const normalizedDraftId = input.draftId?.trim() ?? "";
  return useCreateFlowStore((state) =>
    isActiveCreateFlowForDraft({
      draftId: normalizedDraftId,
      serverId: input.selectedServerId,
      pending: normalizedDraftId ? state.pendingByDraftId[normalizedDraftId] : null,
    }),
  );
}

function resolveVisibleDraftContextScopeKeys(input: {
  isDraftHandoffActive: boolean;
  draftContextScopeKey: string;
}): readonly string[] {
  if (input.isDraftHandoffActive || !input.draftContextScopeKey) {
    return [];
  }
  return [input.draftContextScopeKey];
}

interface NewWorkspaceScreenProps {
  serverId: string;
  sourceDirectory?: string;
  projectId?: string;
  displayName?: string;
  draftId?: string;
}

const PROJECT_PICK_ACTIONS: readonly KeyboardActionId[] = ["workspace.project.pick"];

export function NewWorkspaceScreen({
  serverId,
  sourceDirectory: sourceDirectoryProp,
  projectId,
  displayName: displayNameProp,
  draftId,
}: NewWorkspaceScreenProps) {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const isCompact = useIsCompactFormFactor();
  const toast = useToast();
  const composerEditing = useNewWorkspaceComposerEditing();
  const { leaveFullscreen } = composerEditing;

  const mergeWorkspaces = useCallback(
    (targetServerId: string, workspaces: Iterable<WorkspaceDescriptor>) => {
      getHostRuntimeStore().acceptWorkspaceSnapshots(targetServerId, Array.from(workspaces));
    },
    [],
  );

  const {
    allHosts,
    selectedServerId,
    hostPickerOpen,
    handleSelectHost,
    handleHostPickerOpenChange,
    openHostPicker,
    projects,
    routeProject,
    routeProjectContextViewKey,
    lastActiveProject,
  } = useNewWorkspaceInitialContext({
    serverId,
    sourceDirectory: sourceDirectoryProp,
    projectId,
    displayName: displayNameProp,
  });

  const supportsWorkspaceMultiplicity = useHostFeature(selectedServerId, "workspaceMultiplicity");
  const supportsForgeSearch = useHostFeature(selectedServerId, "forgeSearch");
  const [creationIdentity] = useState(() => ({
    draftId: draftId ?? generateDraftId(),
    worktreeSlug: createNameId(),
  }));

  const projectPickerAnchorRef = useRef<View>(null);
  const isolationPickerAnchorRef = useRef<View>(null);
  const hostPickerAnchorRef = useRef<View | null>(null);
  const isDraftHandoffActive = useIsNewWorkspaceDraftHandoffActive({ draftId, selectedServerId });
  const isStillOnCreateScreen = useNewWorkspaceScreenPresence();

  const {
    launchTarget,
    setManualLaunchTarget,
    terminalProfiles,
    terminalPromptText,
    setTerminalPromptText,
    isTerminalLaunch,
    selectedTerminalProfile,
    terminalTakesPrompt,
    terminalPlaceholder,
    terminalSubmitLabel,
    launchFocusKey,
    terminalTextSource,
    terminalTextReplacement,
    updateFormPreferences,
  } = useNewWorkspaceLaunchTarget(selectedServerId);

  const client = useHostRuntimeClient(selectedServerId);
  const isConnected = useHostRuntimeIsConnected(selectedServerId);
  const clientReady = isConnected && Boolean(client);

  const withConnectedClient = useCallback(() => {
    const connectedClient = getHostRuntimeStore().getClient(selectedServerId);
    if (!connectedClient?.isConnected) {
      throw new Error(t("newWorkspace.errors.hostDisconnected"));
    }
    return connectedClient;
  }, [selectedServerId, t]);

  const {
    selectedProject,
    selectedSourceDirectory,
    projectPickerOptions,
    projectByOptionId,
    selectedProjectOptionId,
    projectTriggerLabel,
    handleSelectProjectOption: selectProjectOption,
  } = useNewWorkspaceProjectPicker({
    selectedServerId,
    projects,
    routeProject,
    routeProjectContextViewKey,
    lastActiveProject,
    allowAllProjects: supportsWorkspaceMultiplicity,
  });

  const projectIconTargets = useMemo(
    () => buildNewWorkspaceProjectIconTargets(projects, selectedServerId),
    [projects, selectedServerId],
  );
  const projectIconDataByProjectViewKey = useProjectIcons({
    projects: projectIconTargets,
  });

  const draftKey = buildNewWorkspaceDraftKey(draftId);
  const forkDraftSetup = usePendingWorkspaceDraftSetup(draftId);
  const draftContextScopeKey = useDraftWorkspaceAttachmentScopeKey(draftId);
  const visibleDraftContextScopeKeys = useMemo(
    () => resolveVisibleDraftContextScopeKeys({ isDraftHandoffActive, draftContextScopeKey }),
    [draftContextScopeKey, isDraftHandoffActive],
  );

  const [pickerSelection, dispatchPickerSelection] = useReducer(
    reducePickerSelection,
    initialPickerSelectionState,
  );
  const selectedItem = pickerSelection.selectedItem;

  const [projectPickerOpen, setProjectPickerOpen] = useState(false);
  const [isolationPickerOpen, setIsolationPickerOpen] = useState(false);
  const openAddProjectPicker = useOpenAddProject();

  const worktreeSupport = selectedProject
    ? getWorktreeSupportForHostProject({ project: selectedProject, serverId: selectedServerId })
    : "unsupported";
  const { effectiveIsolation, setIsolation, canCreateWorktree, showRefPicker } =
    useWorkspaceIsolation({
      supportsMultiplicity: supportsWorkspaceMultiplicity,
      worktreeSupport,
    });

  const chatDraft = useAgentInputDraft({
    draftKey,
    composer: buildComposerConfig({
      serverId: selectedServerId,
      workspaceDirectory: null,
      sourceDirectory: selectedSourceDirectory,
      initialSetup: forkDraftSetup?.setup,
    }),
  });
  const composerState = chatDraft.composerState;

  const { errorMessage, pendingAction, handleSubmitNewWorkspace, handleSubmitTerminalLaunch } =
    useNewWorkspaceSubmit({
      selectedServerId,
      selectedProject,
      selectedSourceDirectory,
      selectedItem,
      supportsWorkspaceMultiplicity,
      effectiveIsolation,
      creationIdentity,
      withConnectedClient,
      mergeWorkspaces,
      queryClient,
      composerState,
      forkDraftSetup,
      draftKey,
      draftContextScopeKey,
      supportsForgeSearch,
      isStillOnCreateScreen,
      leaveFullscreen,
      toast,
      updateFormPreferences,
      launchTarget,
      clearChatDraft: chatDraft.clear,
      selectedTerminalProfile,
      terminalPromptText,
      t,
    });

  const isPending = pendingAction !== null || isDraftHandoffActive;

  const {
    pickerOpen,
    openPicker,
    handlePickerOpenChange,
    pickerAnchorRef,
    setPickerSearchQuery,
    options,
    itemById,
    selectedOptionId,
    triggerLabel,
    handleSelectOption,
    pickerEmptyText,
  } = useNewWorkspacePicker({
    selectedServerId,
    selectedSourceDirectory,
    client,
    clientReady,
    withConnectedClient,
    supportsForgeSearch,
    selectedItem,
    dispatchPickerSelection,
    chatAttachments: chatDraft.attachments,
    setChatAttachments: chatDraft.setAttachments,
    t,
  });

  const handleForgeChangeRequestDetected = useCallback(() => {
    dispatchPickerSelection({ type: "pr-detected" });
  }, []);

  const handleForgeChangeRequestAutoAttach = useCallback((item: ForgeSearchItem) => {
    dispatchPickerSelection({
      type: "pr-added",
      item: { kind: "github-pr", item },
    });
  }, []);

  const clearPickerSelectionForTargetChange = useCallback(
    (currentTargetId: string, nextTargetId: string) => {
      const nextAttachments = clearPickerPrAttachmentForTargetChange({
        attachments: chatDraft.attachments,
        currentTargetId,
        nextTargetId,
      });
      if (nextAttachments === chatDraft.attachments) return;
      chatDraft.setAttachments(nextAttachments);
      dispatchPickerSelection({ type: "target-changed" });
    },
    [chatDraft],
  );

  const handleSelectProjectOption = useCallback(
    (id: string) => {
      selectProjectOption(id);
      setProjectPickerOpen(false);
      clearPickerSelectionForTargetChange(selectedProjectOptionId, id);
    },
    [clearPickerSelectionForTargetChange, selectProjectOption, selectedProjectOptionId],
  );

  const handleSelectWorkspaceHost = useCallback(
    (id: string) => {
      handleSelectHost(id);
      clearPickerSelectionForTargetChange(selectedServerId, id);
    },
    [clearPickerSelectionForTargetChange, handleSelectHost, selectedServerId],
  );

  const handleAddProject = useCallback(() => {
    setProjectPickerOpen(false);
    openAddProjectPicker(selectedServerId);
  }, [openAddProjectPicker, selectedServerId]);

  const handleAddHost = useAddHost();

  const openProjectPicker = useCallback(() => {
    setProjectPickerOpen(true);
  }, []);

  const handleProjectPick = useCallback(() => {
    openProjectPicker();
    return true;
  }, [openProjectPicker]);

  useKeyboardActionHandler({
    handlerId: "new-workspace-project-pick",
    actions: PROJECT_PICK_ACTIONS,
    enabled: projectPickerOptions.length > 0,
    priority: 0,
    handle: handleProjectPick,
  });

  const openIsolationPicker = useCallback(() => {
    setIsolationPickerOpen(true);
  }, []);

  const handleIsolationPickerOpenChange = useCallback((nextOpen: boolean) => {
    setIsolationPickerOpen(nextOpen);
  }, []);

  const handleProjectPickerOpenChange = useCallback((nextOpen: boolean) => {
    setProjectPickerOpen(nextOpen);
  }, []);

  const handleClearDraft = useCallback(() => {
    // No-op: screen navigates away on success, text should stay for retry on error
  }, []);

  const agentControlsWithDisabled = useMemo(
    () =>
      composerState
        ? {
            ...composerState.agentControls,
            disabled: isPending,
          }
        : undefined,
    [composerState, isPending],
  );

  const formStack = useNewWorkspaceFormStack({
    isCompact,
    isPending,
    project: {
      anchorRef: projectPickerAnchorRef,
      open: openProjectPicker,
      openState: projectPickerOpen,
      onOpenChange: handleProjectPickerOpenChange,
      options: projectPickerOptions,
      projectByOptionId,
      iconDataByProjectViewKey: projectIconDataByProjectViewKey,
      selectedProject,
      selectedOptionId: selectedProjectOptionId,
      triggerLabel: projectTriggerLabel,
      selectedServerId,
      supportsWorkspaceMultiplicity,
      onSelect: handleSelectProjectOption,
      onAddProject: handleAddProject,
    },
    host: {
      anchorRef: hostPickerAnchorRef,
      open: openHostPicker,
      openState: hostPickerOpen,
      onOpenChange: handleHostPickerOpenChange,
      allHosts,
      selectedServerId,
      onSelect: handleSelectWorkspaceHost,
      onAddHost: handleAddHost,
    },
    isolation: {
      anchorRef: isolationPickerAnchorRef,
      open: openIsolationPicker,
      openState: isolationPickerOpen,
      onOpenChange: handleIsolationPickerOpenChange,
      effectiveIsolation,
      canCreateWorktree,
      onSelect: setIsolation,
    },
    base: {
      anchorRef: pickerAnchorRef,
      open: openPicker,
      openState: pickerOpen,
      onOpenChange: handlePickerOpenChange,
      selectedSourceDirectory,
      selectedItem,
      triggerLabel,
      options,
      itemById,
      selectedOptionId,
      onSelect: handleSelectOption,
      setSearchQuery: setPickerSearchQuery,
      emptyText: pickerEmptyText,
      showRefPicker,
    },
    launch: {
      serverId: selectedServerId,
      target: launchTarget,
      onChange: setManualLaunchTarget,
      profiles: terminalProfiles,
      disabled: isPending,
    },
  });

  const importSession = useImportSession({ serverId: selectedServerId });
  const isComposerFullscreen = composerEditing.isFullscreen(isTerminalLaunch);

  const composer = (
    <NewWorkspaceComposerView
      isTerminalLaunch={isTerminalLaunch}
      terminalEditingSession={composerEditing.terminal}
      chatEditingSession={composerEditing.chat}
      terminalTakesPrompt={terminalTakesPrompt}
      terminalPlaceholder={terminalPlaceholder}
      terminalSubmitLabel={terminalSubmitLabel}
      agentId={draftKey}
      serverId={selectedServerId}
      onSubmitTerminalLaunch={handleSubmitTerminalLaunch}
      onSubmitNewWorkspace={handleSubmitNewWorkspace}
      submitTerminalButtonAccessibilityLabel={t("newWorkspace.launch.submit")}
      submitChatButtonAccessibilityLabel={t("newWorkspace.create")}
      isPending={isPending}
      terminalTextSource={terminalTextSource}
      setTerminalPromptText={setTerminalPromptText}
      terminalTextReplacement={terminalTextReplacement}
      chatTextSource={chatDraft.textSource}
      chatEditText={chatDraft.editText}
      chatTextReplacement={chatDraft.textReplacement}
      chatAttachments={chatDraft.attachments}
      visibleDraftContextScopeKeys={visibleDraftContextScopeKeys}
      chatSetAttachments={chatDraft.setAttachments}
      onForgeChangeRequestDetected={handleForgeChangeRequestDetected}
      onForgeChangeRequestAutoAttach={handleForgeChangeRequestAutoAttach}
      cwd={selectedSourceDirectory ?? ""}
      onClearDraft={handleClearDraft}
      launchFocusKey={launchFocusKey}
      commandDraft={composerState?.commandDraft}
      agentControls={agentControlsWithDisabled}
    />
  );

  return (
    <FileDropZone style={styles.container}>
      <NewWorkspaceLayout
        isCompact={isCompact}
        fullscreen={isComposerFullscreen}
        title={t("newWorkspace.title")}
        formStack={formStack}
        onImportSession={importSession.open}
      >
        {composer}
        {errorMessage ? <Text style={styles.errorText}>{errorMessage}</Text> : null}
      </NewWorkspaceLayout>
      {importSession.sheet}
    </FileDropZone>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    userSelect: "none",
  },
  errorText: {
    fontSize: theme.fontSize.base,
    color: theme.colors.destructive,
    lineHeight: 20,
  },
}));
