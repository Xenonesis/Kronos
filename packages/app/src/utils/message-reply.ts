import { buildDraftStoreKey } from "@/stores/draft-keys";
import { useDraftStore } from "@/stores/draft-store";
import { isWeb } from "@/constants/platform";

/**
 * Formats a message content as a markdown blockquote for reply.
 */
export function formatQuoteReply(text: string): string {
  const trimmed = text.trim();
  if (!trimmed) {
    return "";
  }
  const lines = trimmed.split("\n");
  const quoted = lines.map((l) => `> ${l}`).join("\n");
  return `${quoted}\n\n`;
}

export function insertQuoteIntoDraft(input: {
  serverId?: string;
  agentId?: string;
  quoteText: string;
}): void {
  const quote = formatQuoteReply(input.quoteText);
  if (!quote) {
    return;
  }

  if (input.serverId && input.agentId) {
    const draftKey = buildDraftStoreKey({
      serverId: input.serverId,
      agentId: input.agentId,
    });
    const currentText = useDraftStore.getState().getDraftInput(draftKey)?.text ?? "";
    const nextText = currentText.trim() ? `${currentText.trim()}\n\n${quote}` : quote;
    useDraftStore.getState().editDraftText({ draftKey, text: nextText });
  }

  if (isWeb && typeof window !== "undefined") {
    window.dispatchEvent(
      new CustomEvent("paseo:composer-reply", {
        detail: {
          serverId: input.serverId,
          agentId: input.agentId,
          quote,
        },
      }),
    );
  }
}
