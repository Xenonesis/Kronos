import React, { memo } from "react";
import { Sparkles, Wand2, TestTube, FileText, Bug } from "lucide-react-native";
import { useSessionStore } from "@/stores/session-store";
import { useDraftStore } from "@/stores/draft-store";
import { useToast } from "@/contexts/toast-context";

export interface InlineCodeLensToolbarProps {
  visible: boolean;
  filename: string;
  selectedText: string;
  selectionCoords?: { top: number; left: number } | null;
  onClose?: () => void;
}

export const InlineCodeLensToolbar = memo(function InlineCodeLensToolbar({
  visible,
  filename,
  selectedText,
  selectionCoords,
  onClose,
}: InlineCodeLensToolbarProps) {
  const toast = useToast();

  if (!visible || !selectedText.trim()) {
    return null;
  }

  const handleAction = (action: "explain" | "refactor" | "test" | "types" | "fix") => {
    // Locate active agent from session store
    const sessions = useSessionStore.getState().sessions;
    const serverIds = Object.keys(sessions);
    const activeServerId = serverIds[0];
    const session = activeServerId ? sessions[activeServerId] : undefined;
    const activeAgent = session?.agents ? Array.from(session.agents.values())[0] : undefined;

    if (!activeServerId || !activeAgent) {
      toast.show("No active agent available to run action", { variant: "error" });
      return;
    }

    const draftKey = `agent:${activeServerId}:${activeAgent.id}`;
    let promptPrefix = "";
    switch (action) {
      case "explain":
        promptPrefix = `Please explain the following code from \`${filename}\` in detail:\n`;
        break;
      case "refactor":
        promptPrefix = `Please refactor and optimize the following code from \`${filename}\` for readability and performance:\n`;
        break;
      case "test":
        promptPrefix = `Please generate comprehensive unit tests with edge cases for the following code from \`${filename}\`:\n`;
        break;
      case "types":
        promptPrefix = `Please review and improve TypeScript type safety and docstrings for the following code from \`${filename}\`:\n`;
        break;
      case "fix":
        promptPrefix = `Please analyze and fix any subtle bugs, memory leaks, or race conditions in the following code from \`${filename}\`:\n`;
        break;
    }

    const fullPrompt = `${promptPrefix}\`\`\`\n${selectedText}\n\`\`\``;
    useDraftStore.getState().replaceDraftText({ draftKey, text: fullPrompt });
    toast.show(`Sent ${action} action to composer`, { variant: "success" });
    onClose?.();
  };

  const style: React.CSSProperties = {
    position: "absolute",
    top: Math.max(10, (selectionCoords?.top ?? 30) - 44),
    left: Math.max(16, selectionCoords?.left ?? 40),
    zIndex: 1000,
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "4px 8px",
    background: "#18181b",
    border: "1px solid #3f3f46",
    borderRadius: "8px",
    boxShadow: "0 4px 14px rgba(0, 0, 0, 0.4)",
    fontSize: "12px",
    fontFamily: "system-ui, -apple-system, sans-serif",
    userSelect: "none",
  };

  const buttonStyle: React.CSSProperties = {
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
    padding: "4px 8px",
    background: "transparent",
    border: "none",
    borderRadius: "4px",
    color: "#f4f4f5",
    cursor: "pointer",
    fontSize: "11px",
    fontWeight: 500,
    transition: "background 0.15s ease",
  };

  return (
    <div style={style} data-testid="inline-codelens-toolbar">
      <span style={{ color: "#a1a1aa", fontSize: "10px", fontWeight: 600, marginRight: "2px" }}>
        AI Ctrl+.
      </span>
      <button
        type="button"
        style={buttonStyle}
        onClick={() => handleAction("explain")}
        title="Explain selected code"
      >
        <Sparkles size={12} color="#a78bfa" />
        <span>Explain</span>
      </button>
      <button
        type="button"
        style={buttonStyle}
        onClick={() => handleAction("refactor")}
        title="Refactor & optimize"
      >
        <Wand2 size={12} color="#38bdf8" />
        <span>Refactor</span>
      </button>
      <button
        type="button"
        style={buttonStyle}
        onClick={() => handleAction("test")}
        title="Generate unit tests"
      >
        <TestTube size={12} color="#4ade80" />
        <span>Tests</span>
      </button>
      <button
        type="button"
        style={buttonStyle}
        onClick={() => handleAction("fix")}
        title="Find and fix bugs"
      >
        <Bug size={12} color="#f87171" />
        <span>Fix</span>
      </button>
    </div>
  );
});
