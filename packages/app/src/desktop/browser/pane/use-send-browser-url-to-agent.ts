import { useCallback } from "react";
import { resolveFocusedChatTarget } from "@/composer/focused-chat-target";
import { useDraftStore } from "@/stores/draft-store";
import { useWorkspaceLayoutStore } from "@/stores/workspace-layout-store";
import { useToast } from "@/contexts/toast-context";

const CONTEXT_PAYLOAD_PREFIXES = ["Attached browser context", "Inspected Element"];

interface ChatTarget {
  draftKey: string;
  tabId: string;
  workspaceKey: string;
}

/** The focused chat tab of the first workspace layout that has one. */
function findFocusedChatTarget(serverId: string): ChatTarget | null {
  const layoutStore = useWorkspaceLayoutStore.getState();
  for (const [key, layout] of Object.entries(layoutStore.layoutByWorkspace)) {
    if (!layout) continue;
    const target = resolveFocusedChatTarget({ serverId, layout });
    if (target) {
      return { draftKey: target.draftKey, tabId: target.tabId, workspaceKey: key };
    }
  }
  return null;
}

/** The first draft that is still active, used when no chat tab has focus. */
function findActiveDraftKey(): string | null {
  const draftStore = useDraftStore.getState();
  const activeDraftEntry = Object.entries(draftStore.drafts).find(
    ([, record]) => record.lifecycle === "active",
  );
  return activeDraftEntry ? activeDraftEntry[0] : null;
}

function isContextPayloadTitle(title?: string): boolean {
  if (typeof title !== "string") return false;
  return CONTEXT_PAYLOAD_PREFIXES.some((prefix) => title.startsWith(prefix));
}

/** The text appended to the prompt for the referenced page. */
function buildAppendedText(url: string, title?: string, isContextPayload = false): string {
  if (isContextPayload) return `\n\n${title}\n`;
  if (title) return `\nReferenced page: ${title} (${url})`;
  return `\n${url}`;
}

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
      const focusedTarget = findFocusedChatTarget(serverId);

      // If no open chat tab found in layouts, check any active draft in useDraftStore
      const targetDraftKey = focusedTarget?.draftKey ?? findActiveDraftKey();

      const isContextPayload = isContextPayloadTitle(title);
      const textToAppend = buildAppendedText(url, title, isContextPayload);

      if (targetDraftKey) {
        const currentInput = useDraftStore.getState().getDraftInput(targetDraftKey);
        const currentText = currentInput?.text ?? "";
        const nextText = currentText ? `${currentText}${textToAppend}` : textToAppend.trim();

        useDraftStore.getState().editDraftText({
          draftKey: targetDraftKey,
          text: nextText,
        });

        if (focusedTarget) {
          layoutStore.focusTab(focusedTarget.workspaceKey, focusedTarget.tabId);
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
