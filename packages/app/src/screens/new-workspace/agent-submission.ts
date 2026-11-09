import type {
  CreateAgentRequestOptions,
  CreateWorkspaceRequestOptions,
  DaemonClient,
} from "@getpaseo/client/internal/daemon-client";
import type {
  AgentAttachment,
  AgentSnapshotPayload,
  CreationSnapshot,
} from "@getpaseo/protocol/messages";
import type { AgentProvider } from "@getpaseo/protocol/agent-types";
import { encodeImages } from "@/utils/encode-images";
import { getHostRuntimeStore, type useHostRuntimeClient } from "@/runtime/host-runtime";
import {
  resolveComposerAttachmentSubmitFormat,
  splitComposerAttachmentsForSubmit,
} from "@/composer/attachments/submit";
import { navigateToWorkspace } from "@/stores/navigation-active-workspace-store";
import { normalizeWorkspaceDescriptor } from "@/stores/session-store";
import { generateDraftId } from "@/stores/draft-keys";
import { useCreateFlowStore } from "@/stores/create-flow-store";
import {
  useWorkspaceDraftSubmissionStore,
  type PendingWorkspaceDraftSetup,
} from "@/stores/workspace-draft-submission-store";
import type { CreateAgentInitialValues } from "@/hooks/use-agent-form-state";
import { getHostProjectId, type HostProjectListItem } from "@/projects/host-projects";
import type { ComposerAttachment } from "@/attachments/types";
import type { MessagePayload } from "@/composer/types";
import type { WorkspaceDraftTabSetup, WorkspaceTabTarget } from "@/workspace-tabs/model";
import {
  getWorkspaceNamingAttachments,
  remapDraftCwdToWorkspace,
} from "@/screens/new-workspace-fork-context";
import type { PickerCheckoutRequest } from "@/screens/new-workspace-picker-item";
import { captureWorkspaceDraftCleanup } from "./background-handoff";
import type { useAgentInputDraft } from "@/composer/draft/input-draft";

export type SubmitOutcome = "navigated" | "background";

export type NewWorkspaceComposerState = NonNullable<
  ReturnType<typeof useAgentInputDraft>["composerState"]
>;

export interface WorkspaceCreationResult {
  workspace: ReturnType<typeof normalizeWorkspaceDescriptor>;
  agent?: AgentSnapshotPayload;
}

export function buildFirstAgentContext(input: {
  prompt: string;
  attachments: AgentAttachment[];
}): { prompt?: string; attachments?: AgentAttachment[] } | undefined {
  const trimmedPrompt = input.prompt.trim();
  if (!trimmedPrompt && input.attachments.length === 0) {
    return undefined;
  }

  return {
    ...(trimmedPrompt ? { prompt: trimmedPrompt } : {}),
    attachments: input.attachments,
  };
}

export async function createMultiplicityWorkspace(input: {
  idempotencyKey: string;
  worktreeSlug: string;
  client: NonNullable<ReturnType<typeof useHostRuntimeClient>>;
  isolation: "local" | "worktree";
  project: HostProjectListItem;
  sourceDirectory: string;
  checkoutRequest: PickerCheckoutRequest | undefined;
  withInitialAgent: boolean;
  agent?: CreateWorkspaceRequestOptions["agent"];
  onEvent?: (snapshot: CreationSnapshot) => void;
  prompt: string;
  attachments: AgentAttachment[];
  mergeWorkspaces: (
    serverId: string,
    workspaces: ReturnType<typeof normalizeWorkspaceDescriptor>[],
  ) => void;
  serverId: string;
  createFailedMessage: string;
}): Promise<WorkspaceCreationResult> {
  const projectId = getHostProjectId(input.project, input.serverId);
  if (!projectId) throw new Error("Project is not available on the selected host");
  const isWorktree = input.isolation === "worktree";
  const firstAgentContext = buildFirstAgentContext({
    prompt: input.prompt,
    attachments: input.attachments,
  });
  const payload = await input.client.createWorkspace({
    idempotencyKey: input.idempotencyKey,
    agent: input.agent,
    onEvent: input.onEvent,
    source: isWorktree
      ? {
          kind: "worktree",
          cwd: input.sourceDirectory,
          projectId,
          worktreeSlug: input.worktreeSlug,
          ...input.checkoutRequest,
        }
      : {
          kind: "directory",
          path: input.sourceDirectory,
          projectId,
        },
    ...(firstAgentContext ? { firstAgentContext } : {}),
  });
  if (payload.error || !payload.workspace) {
    throw new Error(payload.error ?? input.createFailedMessage);
  }
  const normalizedWorkspace = normalizeWorkspaceDescriptor(payload.workspace);
  const workspaceForInitialMerge = input.withInitialAgent
    ? { ...normalizedWorkspace, status: "running" as const, statusEnteredAt: new Date() }
    : normalizedWorkspace;
  input.mergeWorkspaces(input.serverId, [workspaceForInitialMerge]);
  return { workspace: normalizedWorkspace, agent: payload.agent };
}

export interface CreateChatAgentInput {
  payload: MessagePayload;
  composerState: ReturnType<typeof useAgentInputDraft>["composerState"];
  forkDraftSetup?: PendingWorkspaceDraftSetup | null;
  ensureWorkspace: (input: {
    cwd: string;
    prompt: string;
    attachments: AgentAttachment[];
    withInitialAgent: boolean;
    agent?: CreateWorkspaceRequestOptions["agent"];
    onEvent?: (snapshot: CreationSnapshot) => void;
  }) => Promise<WorkspaceCreationResult>;
  serverId: string;
  draftKey: string;
  clearDraft: (lifecycle: "sent" | "abandoned") => void;
  draftId?: string;
  draftContextScopeKey: string | null;
  supportsForgeSearch: boolean;
  resolveClient: () => DaemonClient;
  isStillOnCreateScreen: () => boolean;
  labels: {
    composerStateRequired: string;
    selectModel: string;
  };
}

export function buildWorkspaceDraftSetupFromComposer(input: {
  cwd: string;
  provider: AgentProvider;
  composerState: NewWorkspaceComposerState;
}): WorkspaceDraftTabSetup {
  return {
    provider: input.provider,
    cwd: input.cwd,
    modeId: input.composerState.effectiveModeId || null,
    model: input.composerState.effectiveModelId || null,
    thinkingOptionId: input.composerState.effectiveThinkingOptionId || null,
    featureValues: input.composerState.featureValues ?? {},
  };
}

export function buildWorkspaceDraftSetupForCreatedWorkspace(input: {
  forkDraftSetup: PendingWorkspaceDraftSetup | null | undefined;
  workspaceDirectory: string;
  provider: AgentProvider;
  composerState: NewWorkspaceComposerState;
}): WorkspaceDraftTabSetup | undefined {
  if (!input.forkDraftSetup) {
    return undefined;
  }
  return buildWorkspaceDraftSetupFromComposer({
    cwd: remapDraftCwdToWorkspace({
      cwd: input.forkDraftSetup.setup.cwd,
      sourceDirectory: input.forkDraftSetup.sourceDirectory,
      workspaceDirectory: input.workspaceDirectory,
    }),
    provider: input.provider,
    composerState: input.composerState,
  });
}

export function buildComposerInitialValues(input: {
  initialSetup?: WorkspaceDraftTabSetup | null;
}): CreateAgentInitialValues | undefined {
  if (input.initialSetup) {
    return {
      provider: input.initialSetup.provider,
      modeId: input.initialSetup.modeId,
      model: input.initialSetup.model,
      thinkingOptionId: input.initialSetup.thinkingOptionId,
    };
  }
  return undefined;
}

const pendingWorkspaceSubmissions = new Map<string, Promise<SubmitOutcome>>();

export function runCreateChatAgent(input: CreateChatAgentInput): Promise<SubmitOutcome> {
  const key = JSON.stringify([input.serverId, input.draftId]);
  const pending = pendingWorkspaceSubmissions.get(key);
  if (pending) return pending;
  const submission = Promise.resolve().then(() => createWorkspaceChatAgent(input));
  pendingWorkspaceSubmissions.set(key, submission);
  void submission.finally(() => pendingWorkspaceSubmissions.delete(key)).catch(() => undefined);
  return submission;
}

async function createWorkspaceChatAgent(input: CreateChatAgentInput): Promise<SubmitOutcome> {
  const { payload, composerState, ensureWorkspace, serverId, clearDraft } = input;
  const clearConsumedDraft = captureWorkspaceDraftCleanup(input);
  const { text, attachments, cwd } = payload;
  if (!composerState) {
    throw new Error(input.labels.composerStateRequired);
  }
  const provider = composerState.selectedProvider;
  if (!provider) {
    throw new Error(input.labels.selectModel);
  }
  const attachmentSubmitFormat = resolveComposerAttachmentSubmitFormat({
    supportsForgeAttachments: input.supportsForgeSearch,
  });
  const { attachments: reviewAttachments } = splitComposerAttachmentsForSubmit(attachments, {
    format: attachmentSubmitFormat,
  });
  const workspaceNamingAttachments = getWorkspaceNamingAttachments(reviewAttachments);
  const wirePayload = splitComposerAttachmentsForSubmit(attachments, {
    format: attachmentSubmitFormat,
  });
  const images = await encodeImages(wirePayload.images);
  let navigated = false;
  let outcome: SubmitOutcome = "background";
  const initialAgent: NonNullable<CreateWorkspaceRequestOptions["agent"]> = {
    config: {
      provider,
      cwd,
      modeId: composerState.effectiveModeId || undefined,
      model: composerState.effectiveModelId || undefined,
      thinkingOptionId: composerState.effectiveThinkingOptionId || undefined,
      featureValues: composerState.featureValues,
    },
    initialPrompt: text,
    clientMessageId: `${input.draftId}:initial-message`,
    images: images?.length ? images : undefined,
    attachments: wirePayload.attachments?.length ? wirePayload.attachments : undefined,
  };
  const execute = async (requestedAgent = initialAgent): Promise<AgentSnapshotPayload> => {
    const { agent } = await ensureWorkspace({
      cwd,
      prompt: text,
      attachments: workspaceNamingAttachments,
      withInitialAgent: true,
      agent: requestedAgent,
      onEvent: (snapshot) => {
        if (!snapshot.workspace || navigated) return;
        navigated = true;
        if (!input.isStillOnCreateScreen()) return;
        const workspace = normalizeWorkspaceDescriptor(snapshot.workspace);
        getHostRuntimeStore().acceptWorkspaceSnapshots(serverId, [
          { ...workspace, status: "running" },
        ]);
        const initialSetup = buildWorkspaceDraftSetupForCreatedWorkspace({
          forkDraftSetup: input.forkDraftSetup,
          workspaceDirectory: workspace.workspaceDirectory,
          provider,
          composerState,
        });
        outcome = submitWorkspaceDraft({
          clearConsumedDraft,
          draftKey: input.draftKey,
          draftContextScopeKey: input.draftContextScopeKey,
          resolveClient: input.resolveClient,
          isStillOnCreateScreen: input.isStillOnCreateScreen,
          serverId,
          clearDraft,
          draftId: input.draftId,
          initialSetup,
          workspaceId: workspace.id,
          workspaceDirectory: workspace.workspaceDirectory,
          text,
          attachments,
          provider,
          composerState,
          supportsForgeSearch: input.supportsForgeSearch,
          agentCreation,
        });
      },
    });
    if (!agent) throw new Error("Workspace creation returned no agent");
    return agent;
  };
  const agentCreation = {
    result: Promise.resolve().then(() => execute()),
    retry: (request: CreateAgentRequestOptions) =>
      execute({
        ...initialAgent,
        config: { ...request.config!, cwd },
        initialPrompt: request.initialPrompt ?? "",
        clientMessageId: initialAgent.clientMessageId,
        images: request.images,
        attachments: request.attachments,
      }),
  };
  await agentCreation.result;
  if (outcome === "background") clearConsumedDraft();
  return outcome;
}

export function buildComposerConfig(input: {
  serverId: string;
  workspaceDirectory: string | null;
  sourceDirectory: string | null;
  initialSetup?: WorkspaceDraftTabSetup | null;
}): Parameters<typeof useAgentInputDraft>[0]["composer"] {
  const { serverId, workspaceDirectory, sourceDirectory, initialSetup } = input;
  const workingDir = workspaceDirectory || sourceDirectory || undefined;
  return {
    initialServerId: serverId || null,
    initialValues: buildComposerInitialValues({ initialSetup }),
    initialFeatureValues: initialSetup?.featureValues,
    isVisible: true,
    lockedWorkingDir: workingDir,
  };
}

export function usePendingWorkspaceDraftSetup(
  draftId: string | undefined,
): PendingWorkspaceDraftSetup | null {
  const normalizedDraftId = draftId?.trim() ?? "";
  return useWorkspaceDraftSubmissionStore((state) => {
    if (!normalizedDraftId) {
      return null;
    }
    return state.setupByDraftId[normalizedDraftId] ?? null;
  });
}

export interface WorkspaceDraftSubmissionConfig {
  cwd: string;
  provider: AgentProvider;
  modeId: string | null;
  model: string | null;
  thinkingOptionId: string | null;
  featureValues: Record<string, unknown> | undefined;
  target: WorkspaceTabTarget;
}

export function resolveWorkspaceDraftSubmissionConfig(input: {
  draftId: string;
  workspaceDirectory: string;
  provider: AgentProvider;
  composerState: NewWorkspaceComposerState;
  initialSetup?: WorkspaceDraftTabSetup;
}): WorkspaceDraftSubmissionConfig {
  const { draftId, workspaceDirectory, provider, composerState, initialSetup } = input;
  if (initialSetup) {
    return {
      cwd: initialSetup.cwd,
      provider: initialSetup.provider,
      modeId: initialSetup.modeId,
      model: initialSetup.model,
      thinkingOptionId: initialSetup.thinkingOptionId,
      featureValues: initialSetup.featureValues,
      target: { kind: "draft", draftId, setup: initialSetup },
    };
  }
  return {
    cwd: workspaceDirectory,
    provider,
    modeId: composerState.effectiveModeId || null,
    model: composerState.effectiveModelId || null,
    thinkingOptionId: composerState.effectiveThinkingOptionId || null,
    featureValues: composerState.featureValues,
    target: { kind: "draft", draftId },
  };
}

export interface SubmitDraftInput {
  clearConsumedDraft: () => void;
  agentCreation?: {
    result: Promise<AgentSnapshotPayload>;
    retry: (input: CreateAgentRequestOptions) => Promise<AgentSnapshotPayload>;
  };
  serverId: string;
  draftKey: string;
  clearDraft: (lifecycle: "sent" | "abandoned") => void;
  draftId?: string;
  draftContextScopeKey: string | null;
  initialSetup?: WorkspaceDraftTabSetup;
  workspaceId: string;
  workspaceDirectory: string;
  text: string;
  attachments: ComposerAttachment[];
  provider: AgentProvider;
  composerState: NewWorkspaceComposerState;
  supportsForgeSearch: boolean;
  resolveClient: () => DaemonClient;
  isStillOnCreateScreen: () => boolean;
}

export function submitWorkspaceDraft(input: SubmitDraftInput): SubmitOutcome {
  const {
    serverId,
    clearDraft,
    draftId: draftIdInput,
    workspaceId,
    workspaceDirectory,
    text,
    attachments,
    provider,
    composerState,
    initialSetup,
  } = input;
  const draftId = draftIdInput?.trim() || generateDraftId();
  const clientMessageId = `${draftId}:initial-message`;
  const timestamp = Date.now();
  const wirePayload = splitComposerAttachmentsForSubmit(attachments, {
    format: resolveComposerAttachmentSubmitFormat({
      supportsForgeAttachments: input.supportsForgeSearch,
    }),
  });
  const submission = resolveWorkspaceDraftSubmissionConfig({
    draftId,
    workspaceDirectory,
    provider,
    composerState,
    initialSetup,
  });
  const started = useCreateFlowStore.getState().trySetPending({
    serverId,
    draftId,
    workspaceId,
    agentId: null,
    clientMessageId,
    text: text.trim(),
    timestamp,
    ...(wirePayload.images.length > 0 ? { images: wirePayload.images } : {}),
    ...(wirePayload.attachments.length > 0 ? { attachments: wirePayload.attachments } : {}),
  });
  if (!started) return "navigated";
  useWorkspaceDraftSubmissionStore.getState().setPending({
    serverId,
    workspaceId,
    draftId,
    text: text.trim(),
    attachments,
    cwd: submission.cwd,
    provider: submission.provider,
    clientMessageId,
    timestamp,
    ...(submission.modeId ? { modeId: submission.modeId } : {}),
    ...(submission.model ? { model: submission.model } : {}),
    ...(submission.thinkingOptionId ? { thinkingOptionId: submission.thinkingOptionId } : {}),
    ...(submission.featureValues ? { featureValues: submission.featureValues } : {}),
    allowEmptyText: true,
    agentCreation: input.agentCreation,
  });
  clearDraft("sent");
  navigateToWorkspace({
    serverId,
    workspaceId,
    target: submission.target,
  });
  return "navigated";
}
