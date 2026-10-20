import { useCallback } from "react";
import { resolveFocusedChatTarget } from "@/composer/focused-chat-target";
import { useDraftStore } from "@/stores/draft-store";
import { useWorkspaceLayoutStore } from "@/stores/workspace-layout-store";
import { useToast } from "@/contexts/toast-context";

export function useSendBrowserUrlToAgent(input?: {
  serverId?: string;
  workspaceId?: string | null;
}) {
  const toast = useToast();
  const serverId = input?.serverId ?? "default";

  const sendToAgent = useCallback(
    async (url: string, title?: string) => {
      // 1. Look for focused or active chat in all workspace layouts
      const layoutStore = useWorkspaceLayoutStore.getState();
      const layouts = Object.values(layoutStore.layoutByWorkspace);

      let targetDraftKey: string | null = null;
      let targetTabId: string | null = null;
      let targetWorkspaceKey: string | null = null;

      for (const [key, layout] of Object.entries(layoutStore.layoutByWorkspace)) {
        if (!layout) continue;
        const target = resolveFocusedChatTarget({ serverId, layout });
        if (target) {
          targetDraftKey = target.draftKey;
          targetTabId = target.tabId;
          targetWorkspaceKey = key;
          break;
        }
      }

      // If no open chat tab found in layouts, check any active draft in useDraftStore
      if (!targetDraftKey) {
        const draftStore = useDraftStore.getState();
        const activeDraftEntry = Object.entries(draftStore.drafts).find(
          ([, record]) => record.lifecycle === "active",
        );
        if (activeDraftEntry) {
          targetDraftKey = activeDraftEntry[0];
        }
      }

      const isContextPayload =
        typeof title === "string" &&
        (title.startsWith("Attached browser context") || title.startsWith("Inspected Element"));

      const textToAppend = isContextPayload
        ? `\n\n${title}\n`
        : title
          ? `\nReferenced page: ${title} (${url})`
          : `\n${url}`;

      if (targetDraftKey) {
        const currentInput = useDraftStore.getState().getDraftInput(targetDraftKey);
        const currentText = currentInput?.text ?? "";
        const nextText = currentText ? `${currentText}${textToAppend}` : textToAppend.trim();

        useDraftStore.getState().editDraftText({
          draftKey: targetDraftKey,
          text: nextText,
        });

        if (targetWorkspaceKey && targetTabId) {
          layoutStore.focusTab(targetWorkspaceKey, targetTabId);
        }

        const toastSubject = isContextPayload ? "browser context" : url;
        toast.show(`Appended ${toastSubject} to active prompt`);
      } else {
        // Fallback: save to default scratch draft and notify user
        const defaultKey = `draft:${serverId}:scratch`;
        const currentInput = useDraftStore.getState().getDraftInput(defaultKey);
        const currentText = currentInput?.text ?? "";
        const nextText = currentText ? `${currentText}${textToAppend}` : textToAppend.trim();

        useDraftStore.getState().saveDraftInput({
          draftKey: defaultKey,
          draft: { text: nextText, attachments: [] },
        });

        const toastSubject = isContextPayload ? "browser context" : url;
        toast.show(`Saved ${toastSubject} to agent draft`);
      }
    },
    [serverId, toast],
  );

  return { sendToAgent };
}
