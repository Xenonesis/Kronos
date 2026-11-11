export interface RawChatMessage {
  id?: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp?: string;
}

export interface CompactionResult {
  summary: string;
  condensedMessages: RawChatMessage[];
  tokensFreedEstimate: number;
}

export function compactSessionHistory(messages: RawChatMessage[]): CompactionResult {
  if (messages.length <= 4) {
    return {
      summary: "Session too short to require compaction.",
      condensedMessages: messages,
      tokensFreedEstimate: 0,
    };
  }

  // Preserve the initial user instruction/system prompt
  const initialMessage = messages[0];
  // Preserve the last 3 most recent turns (recency bias)
  const recentMessages = messages.slice(-3);
  // Messages to condense
  const middleMessages = messages.slice(1, -3);

  // Extract key file touches and architectural mentions
  const touchedFiles = new Set<string>();
  const decisions: string[] = [];
  let estimatedTokens = 0;

  for (const msg of middleMessages) {
    estimatedTokens += Math.ceil(msg.content.length / 4);

    // Look for file path mentions
    const fileMatches = msg.content.match(/(?:[a-zA-Z0-9_\-./\\]+\.[a-zA-Z0-9]{1,6})/g);
    if (fileMatches) {
      for (const f of fileMatches) {
        if (!f.includes("http") && !f.includes("github.com") && f.length > 3) {
          touchedFiles.add(f);
        }
      }
    }

    // Look for decisions or actions
    if (
      msg.role === "assistant" &&
      (msg.content.includes("Created") ||
        msg.content.includes("Modified") ||
        msg.content.includes("Fixed"))
    ) {
      const firstLine = msg.content.split("\n")[0].trim();
      if (firstLine.length < 120 && !decisions.includes(firstLine)) {
        decisions.push(firstLine);
      }
    }
  }

  const fileList = Array.from(touchedFiles).slice(0, 15).join(", ");
  const decisionSummary = decisions.slice(0, 5).join("; ");

  const summary = `### [Compacted Context Summary (${middleMessages.length} turns condensed)]
- Files modified/referenced: ${fileList || "Multiple workspace files"}
- Key actions executed: ${decisionSummary || "Iterative development and verification"}
- Original conversation compacted to optimize context window headroom.`;

  const condensedSystemNotice: RawChatMessage = {
    role: "assistant",
    content: summary,
    timestamp: new Date().toISOString(),
  };

  const condensedMessages: RawChatMessage[] = [
    initialMessage,
    condensedSystemNotice,
    ...recentMessages,
  ];

  return {
    summary,
    condensedMessages,
    tokensFreedEstimate: Math.max(0, Math.floor(estimatedTokens * 0.75)),
  };
}
