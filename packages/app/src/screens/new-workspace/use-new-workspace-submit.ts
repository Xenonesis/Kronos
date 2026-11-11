import { useCallback, useState } from "react";
import type { QueryClient } from "@tanstack/react-query";
import type { CreateWorkspaceRequestOptions } from "@getpaseo/client/internal/daemon-client";
import type { CreationSnapshot, TerminalProfile } from "@getpaseo/protocol/messages";
import type { AgentAttachment } from "@getpaseo/protocol/messages";
import type { MessagePayload } from "@/composer/types";
import { ensureCheckoutStatus } from "@/git/checkout-status-cache";
import {
  defaultBasePickerItem,
  pickerItemToCheckoutRequest,
  type PickerItem,
} from "@/screens/new-workspace-picker-item";
import type { HostProjectListItem } from "@/projects/host-projects";
import type { useHostRuntimeClient } from "@/runtime/host-runtime";
import { navigateToWorkspace } from "@/stores/navigation-active-workspace-store";
import type { WorkspaceDescriptor } from "@/stores/session-store";
import { toErrorMessage } from "@/utils/error-messages";
import {
  createMultiplicityWorkspace,
  runCreateChatAgent,
  type CreateChatAgentInput,
  type PendingWorkspaceDraftSetup,
  type SubmitOutcome,
  type WorkspaceCreationResult,
} from "./agent-submission";
import { isEmptyWorkspaceSubmission, runCreateEmptyWorkspace } from "@/screens/new-workspace-empty";
import { runCreateTerminalWorkspace } from "@/screens/new-workspace-terminal";
import {
  buildTerminalsQueryKey,
  upsertCreatedTerminalPayload,
  type ListTerminalsPayload,
} from "@/screens/workspace/terminals/state";
import type { LaunchTarget } from "@/new-workspace-launch/target";

interface UseNewWorkspaceSubmitProps {
  selectedServerId: string;
  selectedProject: HostProjectListItem | null;
  selectedSourceDirectory: string | null;
  selectedItem: PickerItem | null;
  supportsWorkspaceMultiplicity: boolean;
  effectiveIsolation: "local" | "worktree";
  creationIdentity: { draftId: string; worktreeSlug: string };
  withConnectedClient: () => NonNullable<ReturnType<typeof useHostRuntimeClient>>;
  mergeWorkspaces: (targetServerId: string, workspaces: Iterable<WorkspaceDescriptor>) => void;
  queryClient: QueryClient;
  composerState: CreateChatAgentInput["composerState"];
  forkDraftSetup: PendingWorkspaceDraftSetup | null;
  draftKey: string;
  draftContextScopeKey: string;
  supportsForgeSearch: boolean;
  isStillOnCreateScreen: () => boolean;
  leaveFullscreen: () => void;
  toast: { error: (msg: string) => void };
  updateFormPreferences: (prefs: { launchTarget: LaunchTarget }) => Promise<unknown>;
  launchTarget: LaunchTarget;
  clearChatDraft: (lifecycle: "abandoned" | "sent") => void;
  selectedTerminalProfile: TerminalProfile | null;
  terminalPromptText: string;
  t: (key: string) => string;
}

export function useNewWorkspaceSubmit({
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
  clearChatDraft,
  selectedTerminalProfile,
  terminalPromptText,
  t,
}: UseNewWorkspaceSubmitProps) {
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [creationResult, setCreationResult] = useState<
    WorkspaceCreationResult | { workspace: null }
  >({ workspace: null });
  const [pendingAction, setPendingAction] = useState<"chat" | "empty" | "terminal" | null>(null);

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
          clearDraft: clearChatDraft,
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
      clearChatDraft,
      composerState,
      creationIdentity.draftId,
      draftContextScopeKey,
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
    leaveFullscreen,
    queryClient,
    selectedServerId,
    selectedSourceDirectory,
    selectedTerminalProfile,
    t,
    terminalPromptText,
    toast,
    updateFormPreferences,
    withConnectedClient,
  ]);

  return {
    errorMessage,
    creationResult,
    pendingAction,
    setPendingAction,
    ensureWorkspace,
    handleSubmitNewWorkspace,
    handleSubmitTerminalLaunch,
  };
}
