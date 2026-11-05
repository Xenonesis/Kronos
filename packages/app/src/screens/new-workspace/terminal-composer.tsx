import type { UserComposerAttachment } from "@/attachments/types";
import { Composer } from "@/composer";
import type { ComposerEditingSession } from "@/composer/editing-session";
import type { ComposerTextSource } from "@/composer/text-source";
import type { TextReplacement } from "@/composer/types";

export interface NewWorkspaceTerminalComposerProps {
  editingSession: ComposerEditingSession;
  readOnly: boolean;
  placeholder: string;
  submitLabel: string | undefined;
  agentId: string;
  serverId: string;
  onSubmitMessage: () => Promise<void>;
  submitButtonAccessibilityLabel: string;
  isSubmitLoading: boolean;
  textSource: ComposerTextSource;
  onChangeText: (text: string) => void;
  textReplacement: TextReplacement;
  cwd: string;
  autoFocus: boolean;
  autoFocusKey: string;
}

// A terminal launch sends argv, not a message: there is nothing to attach and
// no draft to persist, so the composer's attachment and draft seams are inert.
const NO_TERMINAL_ATTACHMENTS: UserComposerAttachment[] = [];
function noopChangeAttachments() {}
function noopClearDraft() {}

export function NewWorkspaceTerminalComposer({
  editingSession,
  readOnly,
  placeholder,
  submitLabel,
  agentId,
  serverId,
  onSubmitMessage,
  submitButtonAccessibilityLabel,
  isSubmitLoading,
  textSource,
  onChangeText,
  textReplacement,
  cwd,
  autoFocus,
  autoFocusKey,
}: NewWorkspaceTerminalComposerProps) {
  return (
    <Composer
      key="terminal"
      inputMode="terminal"
      editingSession={editingSession}
      readOnly={readOnly}
      placeholder={placeholder}
      submitLabel={submitLabel}
      agentId={agentId}
      serverId={serverId}
      isPaneFocused={true}
      onSubmitMessage={onSubmitMessage}
      allowEmptySubmit={true}
      submitButtonAccessibilityLabel={submitButtonAccessibilityLabel}
      submitButtonTestID="new-workspace-launch-submit"
      isSubmitLoading={isSubmitLoading}
      submitBehavior="preserve-and-lock"
      blurOnSubmit={true}
      textSource={textSource}
      onChangeText={onChangeText}
      textReplacement={textReplacement}
      attachments={NO_TERMINAL_ATTACHMENTS}
      onChangeAttachments={noopChangeAttachments}
      cwd={cwd}
      clearDraft={noopClearDraft}
      autoFocus={autoFocus}
      autoFocusKey={autoFocusKey}
    />
  );
}
