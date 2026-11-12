import { i18n } from "@/i18n/i18next";
import {
  resolvePromptMentions,
  resolvePromptMentionsAsync,
  type ResolveMentionsOptions,
} from "@/composer/at-mentions/mention-resolver";
import { useCascadeFlowStore } from "@/stores/cascade-flow-store";
import { invokeTauri } from "@/utils/tauri-invoke";

export type AgentInputSubmitResult = "noop" | "queued" | "submitted" | "failed";

export interface AgentInputSubmitActionInput<TAttachment> {
  message: string;
  attachments: TAttachment[];
  hasExternalContent?: boolean;
  allowEmptySubmit?: boolean;
  submitBehavior?: "clear" | "preserve-and-lock";
  forceSend?: boolean;
  isAgentRunning: boolean;
  canSubmit: boolean;
  cwd?: string;
  client?: ResolveMentionsOptions["client"];
  queueMessage: (input: { message: string; attachments: TAttachment[] }) => void;
  submitMessage: (input: { message: string; attachments: TAttachment[] }) => Promise<void>;
  clearDraft: (lifecycle: "sent" | "abandoned") => void;
  setUserInput: (text: string) => void;
  setAttachments: (attachments: TAttachment[]) => void;
  setSendError: (message: string | null) => void;
  setIsProcessing: (isProcessing: boolean) => void;
  onSubmitError?: (error: unknown) => void;
  failedToSendMessage?: string;
}

function applyCascadeFlowContext(resolvedMessage: string): string {
  const flowContext = useCascadeFlowStore.getState().getFlowContextString();
  if (flowContext && !resolvedMessage.includes("[Cascade Flow")) {
    return `${resolvedMessage}\n\n${flowContext}`;
  }
  return resolvedMessage;
}

function isEmptySubmission<TAttachment>(
  trimmedMessage: string,
  input: AgentInputSubmitActionInput<TAttachment>,
): boolean {
  return (
    !trimmedMessage &&
    input.attachments.length === 0 &&
    !input.hasExternalContent &&
    !input.allowEmptySubmit
  );
}

export async function submitAgentInput<TAttachment>(
  input: AgentInputSubmitActionInput<TAttachment>,
): Promise<AgentInputSubmitResult> {
  const trimmedMessage = input.message.trim();
  if (isEmptySubmission(trimmedMessage, input) || !input.canSubmit) {
    return "noop";
  }

  const needsAsync = Boolean(
    input.client &&
    input.cwd &&
    (trimmedMessage.includes("@file:") || trimmedMessage.includes("@folder:")),
  );
  const resolvedMessage = needsAsync
    ? await resolvePromptMentionsAsync(trimmedMessage, {
        cwd: input.cwd,
        client: input.client,
      })
    : resolvePromptMentions(trimmedMessage);
  const effectiveMessage = applyCascadeFlowContext(resolvedMessage);
  const attachments = input.attachments;
  const shouldClearOnSubmit = input.submitBehavior !== "preserve-and-lock";

  if (input.isAgentRunning && !input.forceSend) {
    input.queueMessage({ message: effectiveMessage, attachments });
    if (shouldClearOnSubmit) {
      input.setUserInput("");
      input.setAttachments([]);
    }
    return "queued";
  }

  // Clear immediately so the submitted timeline row and composer state stay in sync.
  if (shouldClearOnSubmit) {
    input.setUserInput("");
    input.setAttachments([]);
  }
  input.setSendError(null);
  input.setIsProcessing(true);

  try {
    if (input.cwd) {
      // Non-blocking shadow git checkpoint before agent modifies files (FEAT-19)
      void invokeTauri("create_turn_checkpoint", {
        repoPath: input.cwd,
        turnId: `turn-${Date.now()}`,
      }).catch(() => undefined);
    }
    await input.submitMessage({ message: effectiveMessage, attachments });
    input.clearDraft("sent");
    input.setIsProcessing(false);
    return "submitted";
  } catch (error) {
    input.onSubmitError?.(error);
    if (shouldClearOnSubmit) {
      input.setUserInput(trimmedMessage);
      input.setAttachments(attachments);
    }
    input.setSendError(
      error instanceof Error
        ? error.message
        : (input.failedToSendMessage ?? i18n.t("composer.errors.failedToSend")),
    );
    input.setIsProcessing(false);
    return "failed";
  }
}
