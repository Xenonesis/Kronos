import React, { memo } from "react";
import type { ForgeSearchItem } from "@getpaseo/protocol/messages";
import type { ComposerEditingSession } from "@/composer/editing-session";
import type { ComposerTextSource } from "@/composer/text-source";
import type { MessagePayload, TextReplacement } from "@/composer/types";
import type { AgentInputDraft } from "@/composer/draft/input-draft";
import type { DraftCommandTarget } from "@/agent-commands";
import type { DraftAgentControlsProps } from "@/composer/agent-controls";
import { NewWorkspaceTerminalComposer } from "./terminal-composer";
import { NewWorkspaceChatComposer } from "./chat-composer";

interface NewWorkspaceComposerViewProps {
  isTerminalLaunch: boolean;
  terminalEditingSession: ComposerEditingSession;
  chatEditingSession: ComposerEditingSession;
  terminalTakesPrompt: boolean;
  terminalPlaceholder: string;
  terminalSubmitLabel: string | undefined;
  agentId: string;
  serverId: string;
  onSubmitTerminalLaunch: () => Promise<void>;
  onSubmitNewWorkspace: (payload: MessagePayload) => Promise<void>;
  submitTerminalButtonAccessibilityLabel: string;
  submitChatButtonAccessibilityLabel: string;
  isPending: boolean;
  terminalTextSource: ComposerTextSource;
  setTerminalPromptText: (text: string) => void;
  terminalTextReplacement: TextReplacement;
  chatTextSource: ComposerTextSource;
  chatEditText: AgentInputDraft["editText"];
  chatTextReplacement: TextReplacement;
  chatAttachments: AgentInputDraft["attachments"];
  visibleDraftContextScopeKeys: readonly string[];
  chatSetAttachments: AgentInputDraft["setAttachments"];
  onForgeChangeRequestDetected: () => void;
  onForgeChangeRequestAutoAttach: (item: ForgeSearchItem) => void;
  cwd: string;
  onClearDraft: () => void;
  launchFocusKey: string;
  commandDraft?: DraftCommandTarget;
  agentControls?: DraftAgentControlsProps;
}

export const NewWorkspaceComposerView = memo(function NewWorkspaceComposerView({
  isTerminalLaunch,
  terminalEditingSession,
  chatEditingSession,
  terminalTakesPrompt,
  terminalPlaceholder,
  terminalSubmitLabel,
  agentId,
  serverId,
  onSubmitTerminalLaunch,
  onSubmitNewWorkspace,
  submitTerminalButtonAccessibilityLabel,
  submitChatButtonAccessibilityLabel,
  isPending,
  terminalTextSource,
  setTerminalPromptText,
  terminalTextReplacement,
  chatTextSource,
  chatEditText,
  chatTextReplacement,
  chatAttachments,
  visibleDraftContextScopeKeys,
  chatSetAttachments,
  onForgeChangeRequestDetected,
  onForgeChangeRequestAutoAttach,
  cwd,
  onClearDraft,
  launchFocusKey,
  commandDraft,
  agentControls,
}: NewWorkspaceComposerViewProps) {
  if (isTerminalLaunch) {
    return (
      <NewWorkspaceTerminalComposer
        key="terminal"
        editingSession={terminalEditingSession}
        readOnly={!terminalTakesPrompt}
        placeholder={terminalPlaceholder}
        submitLabel={terminalSubmitLabel}
        agentId={agentId}
        serverId={serverId}
        onSubmitMessage={onSubmitTerminalLaunch}
        submitButtonAccessibilityLabel={submitTerminalButtonAccessibilityLabel}
        isSubmitLoading={isPending}
        textSource={terminalTextSource}
        onChangeText={setTerminalPromptText}
        textReplacement={terminalTextReplacement}
        cwd={cwd}
        autoFocus={terminalTakesPrompt}
        autoFocusKey={launchFocusKey}
      />
    );
  }

  return (
    <NewWorkspaceChatComposer
      key="chat"
      editingSession={chatEditingSession}
      agentId={agentId}
      serverId={serverId}
      onSubmitMessage={onSubmitNewWorkspace}
      submitButtonAccessibilityLabel={submitChatButtonAccessibilityLabel}
      isSubmitLoading={isPending}
      textSource={chatTextSource}
      onChangeText={chatEditText}
      textReplacement={chatTextReplacement}
      attachments={chatAttachments}
      attachmentScopeKeys={visibleDraftContextScopeKeys}
      onChangeAttachments={chatSetAttachments}
      onForgeChangeRequestDetected={onForgeChangeRequestDetected}
      onForgeChangeRequestAutoAttach={onForgeChangeRequestAutoAttach}
      cwd={cwd}
      clearDraft={onClearDraft}
      autoFocusKey={launchFocusKey}
      commandDraft={commandDraft}
      agentControls={agentControls}
    />
  );
});
