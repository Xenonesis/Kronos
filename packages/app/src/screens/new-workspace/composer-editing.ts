import { useCallback } from "react";
import { useComposerEditingSession, useComposerFullscreen } from "@/composer/editing-session";

/** Terminal and chat launches edit in separate sessions; the screen follows the mounted one. */
export function useNewWorkspaceComposerEditing() {
  const terminal = useComposerEditingSession("terminal");
  const chat = useComposerEditingSession("chat");
  const isTerminalFullscreen = useComposerFullscreen(terminal);
  const isChatFullscreen = useComposerFullscreen(chat);
  // The sessions outlive the screen (its draft keeps them), so a launch that leaves
  // the screen also leaves fullscreen; the next draft starts inline.
  const leaveFullscreen = useCallback(() => {
    terminal.setFullscreen(false);
    chat.setFullscreen(false);
  }, [chat, terminal]);
  return {
    terminal,
    chat,
    leaveFullscreen,
    isFullscreen: (isTerminalLaunch: boolean) =>
      isTerminalLaunch ? isTerminalFullscreen : isChatFullscreen,
  };
}
