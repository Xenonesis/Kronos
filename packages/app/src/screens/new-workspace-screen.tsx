import type { CreateWorkspaceRequestOptions } from "@getpaseo/client/internal/daemon-client";
import type { CreationSnapshot, TerminalProfile } from "@getpaseo/protocol/messages";
import { getHostRuntimeStore } from "@/runtime/host-runtime";
import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { createNameId } from "mnemonic-id";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { FileDropZone } from "@/components/file-drop/file-drop-zone";
import { useIsCompactFormFactor } from "@/constants/layout";
import { useToast } from "@/contexts/toast-context";
import { useAgentInputDraft } from "@/composer/draft/input-draft";
import { useForgeSearchQuery } from "@/git/use-forge-search-query";
import { useCheckoutStatusQuery } from "@/git/use-status-query";
import { ensureCheckoutStatus } from "@/git/checkout-status-cache";
import { useDaemonConfig } from "@/hooks/use-daemon-config";
import { resolveTerminalProfiles } from "@getpaseo/protocol/terminal-profiles";
import { resolveLaunchTarget, type LaunchTarget } from "@/new-workspace-launch/target";
import { useTerminalComposerState } from "@/new-workspace-launch/composer-state";
import { runCreateTerminalWorkspace } from "./new-workspace-terminal";
import { useHostRuntimeClient, useHostRuntimeIsConnected } from "@/runtime/host-runtime";
import { useHostFeature } from "@/runtime/host-features";
import { navigateToWorkspace } from "@/stores/navigation-active-workspace-store";
import type { WorkspaceDescriptor } from "@/stores/session-store";
import { buildNewWorkspaceDraftKey, generateDraftId } from "@/stores/draft-keys";
import { useOpenAddProject } from "@/hooks/use-open-add-project";
import { isActiveCreateFlowForDraft, useCreateFlowStore } from "@/stores/create-flow-store";
import { useKeyboardActionHandler } from "@/hooks/use-keyboard-action-handler";
import type { KeyboardActionId } from "@/keyboard/keyboard-action-dispatcher";
import { useFormPreferences } from "@/hooks/use-form-preferences";
import { toErrorMessage } from "@/utils/error-messages";
import { useAddHost } from "@/hosts/add-host";
import { getWorktreeSupportForHostProject } from "@/projects/host-projects";
import { useProjectIcons } from "@/projects/icons";
import { useDraftWorkspaceAttachmentScopeKey } from "@/attachments/workspace-attachments-store";
import type { MessagePayload } from "@/composer/types";
import type { AgentAttachment, ForgeSearchItem } from "@getpaseo/protocol/messages";
import { isEmptyWorkspaceSubmission, runCreateEmptyWorkspace } from "./new-workspace-empty";
import {
  buildPickerOptionData,
  defaultBasePickerItem,
  pickerItemLabel,
  pickerItemToCheckoutRequest,
  type PickerItem,
  type PickerOptionData,
} from "./new-workspace-picker-item";
import {
  clearPickerPrAttachmentForTargetChange,
  initialPickerSelectionState,
  reducePickerSelection,
  syncPickerPrAttachment,
} from "./new-workspace-picker-state";
import { buildNewWorkspaceProjectIconTargets } from "./new-workspace/project-icon-targets";
import { useNewWorkspaceProjectPicker } from "./new-workspace/project-picker";
import { useNewWorkspaceForm } from "./new-workspace/form";
import { NewWorkspaceLayout } from "./new-workspace/layout";
import { NewWorkspaceChatComposer } from "./new-workspace/chat-composer";
import { NewWorkspaceTerminalComposer } from "./new-workspace/terminal-composer";
import { useImportSession } from "@/hooks/use-import-session";
import { useNewWorkspaceScreenPresence } from "./new-workspace/screen-presence";
import {
  buildComposerConfig,
  createMultiplicityWorkspace,
  runCreateChatAgent,
  usePendingWorkspaceDraftSetup,
  type SubmitOutcome,
  type WorkspaceCreationResult,
} from "./new-workspace/agent-submission";
import { normalizeBranchDetails, useWorkspaceIsolation } from "./new-workspace/ref-picker-state";
import { useNewWorkspaceInitialContext } from "./new-workspace/initial-context-hook";
import { useNewWorkspaceComposerEditing } from "./new-workspace/composer-editing";
import {
  buildTerminalsQueryKey,
  upsertCreatedTerminalPayload,
  type ListTerminalsPayload,
} from "@/screens/workspace/terminals/state";

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

function isNewWorkspacePending(input: {
  pendingAction: "chat" | "empty" | "terminal" | null;
  isDraftHandoffActive: boolean;
}): boolean {
  return input.pendingAction !== null || input.isDraftHandoffActive;
}

interface NewWorkspaceScreenProps {
  serverId: string;
  sourceDirectory?: string;
  projectId?: string;
  displayName?: string;
  draftId?: string;
}

// Stable reference so the keyboard-action handler doesn't re-register each render.
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
  // COMPAT(workspaceMultiplicity): added in v0.1.97, drop the gate when floor >= v0.1.97
  const supportsWorkspaceMultiplicity = useHostFeature(selectedServerId, "workspaceMultiplicity");
  const supportsForgeSearch = useHostFeature(selectedServerId, "forgeSearch");
  const [creationIdentity] = useState(() => ({
    draftId: draftId ?? generateDraftId(),
    worktreeSlug: createNameId(),
  }));
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [creationResult, setCreationResult] = useState<
    WorkspaceCreationResult | { workspace: null }
  >({ workspace: null });
  const [pendingAction, setPendingAction] = useState<"chat" | "empty" | "terminal" | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [projectPickerOpen, setProjectPickerOpen] = useState(false);
  const openAddProjectPicker = useOpenAddProject();
  const [isolationPickerOpen, setIsolationPickerOpen] = useState(false);
  const [pickerSearchQuery, setPickerSearchQuery] = useState("");
  const [debouncedPickerSearchQuery, setDebouncedPickerSearchQuery] = useState("");
  const pickerAnchorRef = useRef<View>(null);
  const projectPickerAnchorRef = useRef<View>(null);
  const isolationPickerAnchorRef = useRef<View>(null);
  const hostPickerAnchorRef = useRef<View | null>(null);
  const isDraftHandoffActive = useIsNewWorkspaceDraftHandoffActive({ draftId, selectedServerId });
  const isStillOnCreateScreen = useNewWorkspaceScreenPresence();

  // Launch target: what the composer submits to (chat agent, or a terminal
  // profile). Mirrors useWorkspaceIsolation's pattern below: the derived
  // value reads live from preferences until the user manually picks
  // something in this screen, so the async preferences load doesn't race a
  // frozen useState initializer.
  const { preferences: formPreferences, updatePreferences: updateFormPreferences } =
    useFormPreferences();
  const { config: daemonConfig } = useDaemonConfig(selectedServerId);
  const terminalProfiles: readonly TerminalProfile[] = useMemo(
    () => resolveTerminalProfiles(daemonConfig?.terminalProfiles),
    [daemonConfig?.terminalProfiles],
  );
  // Manual selection wins once the user picks something; until then the target
  // reads live from preferences so the async load can't race a frozen
  // initializer. Both go through `resolveLaunchTarget`, so a profile deleted
  // daemon-side falls back to chat rather than leaving a dead selection.
  const [manualLaunchTarget, setManualLaunchTarget] = useState<LaunchTarget | null>(null);
  const launchTarget = useMemo(
    () => resolveLaunchTarget(manualLaunchTarget ?? formPreferences.launchTarget, terminalProfiles),
    [manualLaunchTarget, formPreferences.launchTarget, terminalProfiles],
  );
  const [terminalPromptText, setTerminalPromptText] = useState("");
  const {
    isTerminalLaunch,
    selectedTerminalProfile,
    terminalTakesPrompt,
    terminalComposerValue,
    terminalPlaceholder,
    terminalSubmitLabel,
    launchFocusKey,
  } = useTerminalComposerState({ launchTarget, terminalProfiles, terminalPromptText });
  const terminalTextSource = useMemo(
    () => ({
      getSnapshot: () => terminalComposerValue,
      subscribe: (_listener: () => void) => () => {},
    }),
    [terminalComposerValue],
  );
  const terminalTextReplacement = useMemo(
    () => ({ kind: "replace" as const, key: launchFocusKey, text: terminalComposerValue }),
    [launchFocusKey, terminalComposerValue],
  );

  useEffect(() => {
    const trimmed = pickerSearchQuery.trim();
    const timer = setTimeout(() => setDebouncedPickerSearchQuery(trimmed), 180);
    return () => clearTimeout(timer);
  }, [pickerSearchQuery]);

  const { workspace } = creationResult;
  const client = useHostRuntimeClient(selectedServerId);
  const isConnected = useHostRuntimeIsConnected(selectedServerId);
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
  const chatDraft = useAgentInputDraft({
    draftKey,
    composer: buildComposerConfig({
      serverId: selectedServerId,
      workspaceDirectory: workspace?.workspaceDirectory ?? null,
      sourceDirectory: selectedSourceDirectory,
      initialSetup: forkDraftSetup?.setup,
    }),
  });
  const composerState = chatDraft.composerState;
  const [pickerSelection, dispatchPickerSelection] = useReducer(
    reducePickerSelection,
    initialPickerSelectionState,
  );
  const selectedItem = pickerSelection.selectedItem;

  const handleForgeChangeRequestDetected = useCallback(() => {
    dispatchPickerSelection({ type: "pr-detected" });
  }, []);

  const handleForgeChangeRequestAutoAttach = useCallback((item: ForgeSearchItem) => {
    dispatchPickerSelection({
      type: "pr-added",
      item: { kind: "github-pr", item },
    });
  }, []);

  const withConnectedClient = useCallback(() => {
    const connectedClient = getHostRuntimeStore().getClient(selectedServerId);
    if (!connectedClient?.isConnected) {
      throw new Error(t("newWorkspace.errors.hostDisconnected"));
    }
    return connectedClient;
  }, [selectedServerId, t]);

  const clientReady = isConnected && Boolean(client);
  const hasSelectedSourceDirectory = selectedSourceDirectory !== null;
  const pickerQueryEnabled = pickerOpen && clientReady && hasSelectedSourceDirectory;

  const { status: checkoutStatus } = useCheckoutStatusQuery({
    serverId: selectedServerId,
    cwd: selectedSourceDirectory ?? "",
  });

  const worktreeSupport = selectedProject
    ? getWorktreeSupportForHostProject({ project: selectedProject, serverId: selectedServerId })
    : "unsupported";
  const isPending = isNewWorkspacePending({ pendingAction, isDraftHandoffActive });
  const { effectiveIsolation, setIsolation, canCreateWorktree, showRefPicker } =
    useWorkspaceIsolation({
      supportsMultiplicity: supportsWorkspaceMultiplicity,
      worktreeSupport,
    });

  const branchSuggestionsQuery = useQuery({
    queryKey: [
      "branch-suggestions",
      selectedServerId,
      selectedSourceDirectory,
      debouncedPickerSearchQuery,
    ],
    queryFn: async () => {
      if (!selectedSourceDirectory) {
        throw new Error("Choose a project");
      }
      const connectedClient = withConnectedClient();
      return connectedClient.getBranchSuggestions({
        cwd: selectedSourceDirectory,
        query: debouncedPickerSearchQuery || undefined,
        limit: 20,
      });
    },
    enabled: pickerQueryEnabled,
    staleTime: 15_000,
  });

  const githubPrSearchQuery = useForgeSearchQuery({
    client,
    serverId: selectedServerId,
    cwd: selectedSourceDirectory ?? "",
    query: debouncedPickerSearchQuery,
    kinds: ["change_request"],
    supportsForgeSearch,
    enabled: pickerQueryEnabled,
  });

  const branchDetails = useMemo(
    () => normalizeBranchDetails(branchSuggestionsQuery.data),
    [branchSuggestionsQuery.data],
  );
  const forgeSearchAuthenticated =
    !githubPrSearchQuery.data || githubPrSearchQuery.data.authState === "authenticated";
  const prItems: ForgeSearchItem[] = useMemo(() => {
    if (!forgeSearchAuthenticated) return [];
    return githubPrSearchQuery.data?.items ?? [];
  }, [forgeSearchAuthenticated, githubPrSearchQuery.data?.items]);

  const baseItem = useMemo(
    () => selectedItem ?? (checkoutStatus ? defaultBasePickerItem(checkoutStatus) : null),
    [checkoutStatus, selectedItem],
  );
  const { options, itemById, selectedOptionId }: PickerOptionData = useMemo(
    () =>
      buildPickerOptionData({
        branchDetails,
        prItems,
        baseItem,
      }),
    [baseItem, branchDetails, prItems],
  );
  const triggerLabel = useMemo(() => {
    const displayItem = itemById.get(selectedOptionId);
    return displayItem ? pickerItemLabel(displayItem) : "main";
  }, [itemById, selectedOptionId]);
  const selectPickerItem = useCallback(
    (item: PickerItem) => {
      const nextAttachments = syncPickerPrAttachment({
        attachments: chatDraft.attachments,
        item,
      });

      dispatchPickerSelection({ type: "picker-selected", item });
      chatDraft.setAttachments(nextAttachments);
      setPickerOpen(false);
    },
    [chatDraft],
  );

  const handleSelectOption = useCallback(
    (id: string) => {
      const item = itemById.get(id);
      if (!item) return;
      selectPickerItem(item);
    },
    [itemById, selectPickerItem],
  );

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

  const openPicker = useCallback(() => {
    setPickerOpen(true);
  }, []);

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

  const handleClearDraft = useCallback(() => {
    // No-op: screen navigates away on success, text should stay for retry on error
  }, []);

  const handlePickerOpenChange = useCallback((nextOpen: boolean) => {
    setPickerOpen(nextOpen);
    if (!nextOpen) {
      setPickerSearchQuery("");
    }
  }, []);

  const handleProjectPickerOpenChange = useCallback((nextOpen: boolean) => {
    setProjectPickerOpen(nextOpen);
  }, []);

  const ensureWorkspace = useCallback(
    async (input: {
      cwd: string;
      prompt: string;
      attachments: AgentAttachment[];
      withInitialAgent: boolean;
      agent?: CreateWorkspaceRequestOptions["agent"];
      onEvent?: (snapshot: CreationSnapshot) => void;
    }) => {
      if (creationResult.workspace) {
        return creationResult;
      }
      if (!selectedProject) {
        throw new Error("Choose a project");
      }
      if (!selectedSourceDirectory) {
        throw new Error("Choose a host for this project");
      }
      const connectedClient = withConnectedClient();
      const createsWorktree = !supportsWorkspaceMultiplicity || effectiveIsolation === "worktree";
      const checkoutStatusForCreate = createsWorktree
        ? await ensureCheckoutStatus({
            queryClient,
            client: connectedClient,
            serverId: selectedServerId,
            cwd: selectedSourceDirectory,
          })
        : null;
      const checkoutRequest = checkoutStatusForCreate
        ? pickerItemToCheckoutRequest(
            selectedItem ?? defaultBasePickerItem(checkoutStatusForCreate),
          )
        : undefined;
      const normalizedWorkspace = await createMultiplicityWorkspace({
        idempotencyKey: creationIdentity.draftId,
        worktreeSlug: creationIdentity.worktreeSlug,
        client: connectedClient,
        isolation: createsWorktree ? "worktree" : "local",
        project: selectedProject,
        sourceDirectory: selectedSourceDirectory,
        checkoutRequest,
        withInitialAgent: input.withInitialAgent,
        prompt: input.prompt,
        attachments: input.attachments,
        agent: input.agent,
        onEvent: input.onEvent,
        mergeWorkspaces,
        serverId: selectedServerId,
        createFailedMessage: t("newWorkspace.errors.createWorktreeFailed"),
      });
      setCreationResult(normalizedWorkspace);
      return normalizedWorkspace;
    },
    [
      creationIdentity,
      creationResult,
      effectiveIsolation,
      mergeWorkspaces,
      queryClient,
      selectedItem,
      selectedProject,
      selectedServerId,
      selectedSourceDirectory,
      supportsWorkspaceMultiplicity,
      t,
      withConnectedClient,
    ],
  );

  const handleSubmitNewWorkspace = useCallback(
    async (payload: MessagePayload) => {
      try {
        setErrorMessage(null);
        await composerState?.persistFormPreferences();
        await updateFormPreferences({ launchTarget });
        if (isEmptyWorkspaceSubmission(payload)) {
          setPendingAction("empty");
          let outcome: SubmitOutcome = "background";
          await runCreateEmptyWorkspace({
            payload,
            ensureWorkspace: async (request) => (await ensureWorkspace(request)).workspace,
            serverId: selectedServerId,
            navigate: (targetServerId, workspaceId) => {
              if (!isStillOnCreateScreen()) {
                return;
              }
              outcome = "navigated";
              leaveFullscreen();
              navigateToWorkspace({ serverId: targetServerId, workspaceId });
            },
          });
          if (outcome === "background") {
            setPendingAction(null);
          }
          return;
        }

        setPendingAction("chat");
        const outcome = await runCreateChatAgent({
          payload,
          composerState,
          forkDraftSetup,
          ensureWorkspace,
          serverId: selectedServerId,
          clearDraft: chatDraft.clear,
          draftKey,
          draftId: creationIdentity.draftId,
          draftContextScopeKey,
          supportsForgeSearch,
          resolveClient: withConnectedClient,
          isStillOnCreateScreen,
          labels: {
            composerStateRequired: t("newWorkspace.errors.composerStateRequired"),
            selectModel: t("newWorkspace.errors.selectModel"),
          },
        });
        if (outcome === "background") {
          setPendingAction(null);
        }
      } catch (error) {
        const message = toErrorMessage(error);
        setPendingAction(null);
        setErrorMessage(message);
        toast.error(message);
      }
    },
    [
      composerState,
      draftContextScopeKey,
      creationIdentity,
      chatDraft.clear,
      draftKey,
      ensureWorkspace,
      forkDraftSetup,
      isStillOnCreateScreen,
      launchTarget,
      leaveFullscreen,
      selectedServerId,
      supportsForgeSearch,
      t,
      toast,
      updateFormPreferences,
      withConnectedClient,
    ],
  );

  const handleSubmitTerminalLaunch = useCallback(async () => {
    try {
      setErrorMessage(null);
      await updateFormPreferences({ launchTarget });
      setPendingAction("terminal");
      let outcome: SubmitOutcome = "background";
      await runCreateTerminalWorkspace({
        cwd: selectedSourceDirectory ?? "",
        prompt: terminalPromptText,
        profile: selectedTerminalProfile,
        profileName: selectedTerminalProfile?.name,
        ensureWorkspace: async (request) => (await ensureWorkspace(request)).workspace,
        createTerminal: async (input) => {
          const connectedClient = withConnectedClient();
          const createdTerminal = await connectedClient.createTerminal(
            input.workspaceDirectory,
            input.name,
            undefined,
            { command: input.command, args: input.args, workspaceId: input.workspaceId },
          );
          const terminal = createdTerminal.terminal;
          if (!terminal) {
            throw new Error(createdTerminal.error ?? t("newWorkspace.errors.createWorktreeFailed"));
          }
          queryClient.setQueryData<ListTerminalsPayload>(
            buildTerminalsQueryKey(selectedServerId, input.workspaceDirectory, input.workspaceId),
            (current: ListTerminalsPayload | undefined) =>
              upsertCreatedTerminalPayload({
                current,
                terminal,
                workspaceDirectory: input.workspaceDirectory,
              }),
          );
          return { terminalId: terminal.id };
        },
        sendTerminalInput: (terminalId, data) => {
          withConnectedClient().sendTerminalInput(terminalId, { type: "input", data });
        },
        serverId: selectedServerId,
        navigate: (targetServerId, workspaceId, target) => {
          if (!isStillOnCreateScreen()) {
            return;
          }
          outcome = "navigated";
          leaveFullscreen();
          navigateToWorkspace({ serverId: targetServerId, workspaceId, target });
        },
      });
      if (outcome === "background") {
        setPendingAction(null);
      }
    } catch (error) {
      const message = toErrorMessage(error);
      setPendingAction(null);
      setErrorMessage(message);
      toast.error(message);
    }
  }, [
    ensureWorkspace,
    isStillOnCreateScreen,
    launchTarget,
    queryClient,
    selectedServerId,
    selectedSourceDirectory,
    selectedTerminalProfile,
    t,
    terminalPromptText,
    toast,
    leaveFullscreen,
    updateFormPreferences,
    withConnectedClient,
  ]);

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

  const pickerEmptyText =
    branchSuggestionsQuery.isFetching || githubPrSearchQuery.isFetching
      ? t("newWorkspace.refPicker.searching")
      : t("newWorkspace.refPicker.noMatchingRefs");

  const formStack = useNewWorkspaceForm({
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
  const composer = isTerminalLaunch ? (
    <NewWorkspaceTerminalComposer
      key="terminal"
      editingSession={composerEditing.terminal}
      readOnly={!terminalTakesPrompt}
      placeholder={terminalPlaceholder}
      submitLabel={terminalSubmitLabel}
      agentId={draftKey}
      serverId={selectedServerId}
      onSubmitMessage={handleSubmitTerminalLaunch}
      submitButtonAccessibilityLabel={t("newWorkspace.launch.submit")}
      isSubmitLoading={isPending}
      textSource={terminalTextSource}
      onChangeText={setTerminalPromptText}
      textReplacement={terminalTextReplacement}
      cwd={selectedSourceDirectory ?? ""}
      autoFocus={terminalTakesPrompt}
      autoFocusKey={launchFocusKey}
    />
  ) : (
    <NewWorkspaceChatComposer
      key="chat"
      editingSession={composerEditing.chat}
      agentId={draftKey}
      serverId={selectedServerId}
      onSubmitMessage={handleSubmitNewWorkspace}
      submitButtonAccessibilityLabel={t("newWorkspace.create")}
      isSubmitLoading={isPending}
      textSource={chatDraft.textSource}
      onChangeText={chatDraft.editText}
      textReplacement={chatDraft.textReplacement}
      attachments={chatDraft.attachments}
      attachmentScopeKeys={visibleDraftContextScopeKeys}
      onChangeAttachments={chatDraft.setAttachments}
      onForgeChangeRequestDetected={handleForgeChangeRequestDetected}
      onForgeChangeRequestAutoAttach={handleForgeChangeRequestAutoAttach}
      cwd={selectedSourceDirectory ?? ""}
      clearDraft={handleClearDraft}
      autoFocusKey={launchFocusKey}
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
