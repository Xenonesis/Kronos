import type { ForgeSearchItem } from "@getpaseo/protocol/messages";
import type { DraftCommandTarget } from "@/agent-commands";
import { Composer } from "@/composer";
import type { DraftAgentControlsProps } from "@/composer/agent-controls";
import type { AgentInputDraft } from "@/composer/draft/input-draft";
import type { ComposerEditingSession } from "@/composer/editing-session";
import type { MessagePayload } from "@/composer/types";

export interface NewWorkspaceChatComposerProps extends Pick<
  AgentInputDraft,
  "textSource" | "textReplacement" | "attachments"
> {
  editingSession: ComposerEditingSession;
  agentId: string;
  serverId: string;
  onSubmitMessage: (payload: MessagePayload) => Promise<void>;
  submitButtonAccessibilityLabel: string;
  isSubmitLoading: boolean;
  onChangeText: AgentInputDraft["editText"];
  attachmentScopeKeys: readonly string[];
  onChangeAttachments: AgentInputDraft["setAttachments"];
  onForgeChangeRequestDetected: () => void;
  onForgeChangeRequestAutoAttach: (item: ForgeSearchItem) => void;
  cwd: string;
  clearDraft: AgentInputDraft["clear"];
  autoFocusKey: string;
  commandDraft?: DraftCommandTarget;
  agentControls?: DraftAgentControlsProps;
}

export function NewWorkspaceChatComposer({
  editingSession,
  agentId,
  serverId,
  onSubmitMessage,
  submitButtonAccessibilityLabel,
  isSubmitLoading,
  textSource,
  onChangeText,
  textReplacement,
  attachments,
  attachmentScopeKeys,
  onChangeAttachments,
  onForgeChangeRequestDetected,
  onForgeChangeRequestAutoAttach,
  cwd,
  clearDraft,
  autoFocusKey,
  commandDraft,
  agentControls,
}: NewWorkspaceChatComposerProps) {
  return (
    <Composer
      key="chat"
      editingSession={editingSession}
      agentId={agentId}
      serverId={serverId}
      isPaneFocused={true}
      onSubmitMessage={onSubmitMessage}
      allowEmptySubmit={true}
      submitButtonAccessibilityLabel={submitButtonAccessibilityLabel}
      submitButtonTestID="workspace-create-submit"
      submitIcon="return"
      isSubmitLoading={isSubmitLoading}
      waitForForgeAutoAttachOnSubmit
      submitBehavior="preserve-and-lock"
      blurOnSubmit={true}
      textSource={textSource}
      onChangeText={onChangeText}
      textReplacement={textReplacement}
      attachments={attachments}
      attachmentScopeKeys={attachmentScopeKeys}
      onChangeAttachments={onChangeAttachments}
      onForgeChangeRequestDetected={onForgeChangeRequestDetected}
      onForgeChangeRequestAutoAttach={onForgeChangeRequestAutoAttach}
      cwd={cwd}
      clearDraft={clearDraft}
      autoFocus
      autoFocusKey={autoFocusKey}
      commandDraft={commandDraft}
      agentControls={agentControls}
    />
  );
}
