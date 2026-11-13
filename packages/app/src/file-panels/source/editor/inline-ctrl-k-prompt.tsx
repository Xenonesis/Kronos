import React, { memo, useState, useCallback, useEffect, useRef } from "react";
import { Sparkles, Check, X, CornerDownLeft, Loader2 } from "lucide-react-native";
import { useToast } from "@/contexts/toast-context";
import { invokeTauri } from "@/utils/tauri-invoke";

export interface InlineCtrlKPromptProps {
  visible: boolean;
  filename: string;
  selectedText: string;
  cwd?: string;
  selectionCoords?: { top: number; left: number } | null;
  onAccept: (newCode: string) => void;
  onClose: () => void;
}

export const InlineCtrlKPrompt = memo(function InlineCtrlKPrompt({
  visible,
  filename,
  selectedText,
  cwd,
  selectionCoords,
  onAccept,
  onClose,
}: InlineCtrlKPromptProps) {
  const [prompt, setPrompt] = useState("");
  const [isGenerating, setIsGenerating] = useState(false);
  const [generatedCode, setGeneratedCode] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const toast = useToast();

  useEffect(() => {
    if (visible) {
      setPrompt("");
      setGeneratedCode(null);
      setIsGenerating(false);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [visible]);

  const handleGenerate = useCallback(async () => {
    if (!prompt.trim() || isGenerating) return;

    setIsGenerating(true);
    try {
      const response = await invokeTauri<{
        provider: string;
        model: string;
        text: string;
        latencyMs: number;
      }>("run_inline_code_edit", {
        filename,
        selectedCode: selectedText,
        instruction: prompt.trim(),
        cwd,
      });
      if (!response?.text) {
        toast.show(
          "No active local/CLI model returned an inline edit. Start Ollama/LM Studio or configure Claude/Codex.",
          { variant: "error" },
        );
        return;
      }
      setGeneratedCode(response.text);
    } catch (err) {
      toast.show(`Inline AI edit failed: ${String(err)}`, { variant: "error" });
    } finally {
      setIsGenerating(false);
    }
  }, [prompt, selectedText, filename, cwd, isGenerating, toast]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (generatedCode) {
        onAccept(generatedCode);
        onClose();
      } else {
        void handleGenerate();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.key === "Tab" && generatedCode) {
      e.preventDefault();
      onAccept(generatedCode);
      onClose();
    }
  };

  if (!visible) return null;

  const top = Math.max(8, (selectionCoords?.top ?? 60) - 8);
  const left = Math.max(16, selectionCoords?.left ?? 60);

  return (
    <div
      data-testid="inline-ctrl-k-prompt"
      style={{
        position: "absolute",
        top: `${top}px`,
        left: `${left}px`,
        zIndex: 1000,
        width: "480px",
        maxWidth: "calc(100% - 32px)",
        backgroundColor: "#1e1e1e",
        border: "1px solid #3b82f6",
        borderRadius: "8px",
        boxShadow: "0 8px 24px rgba(0, 0, 0, 0.45)",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        fontFamily: "system-ui, -apple-system, sans-serif",
      }}
      onKeyDown={handleKeyDown}
    >
      {/* Header / Input Row */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          padding: "8px 12px",
          gap: "8px",
          borderBottom: generatedCode ? "1px solid #333" : "none",
        }}
      >
        <Sparkles size={16} color="#60a5fa" />
        <input
          ref={inputRef}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder={`Edit ${filename.split("/").pop()} with AI (Ctrl+K)...`}
          disabled={isGenerating}
          style={{
            flex: 1,
            backgroundColor: "transparent",
            border: "none",
            outline: "none",
            color: "#f3f4f6",
            fontSize: "13px",
          }}
        />

        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          <button
            onClick={() => void handleGenerate()}
            disabled={!prompt.trim() || isGenerating}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "4px",
              padding: "4px 8px",
              backgroundColor: prompt.trim() ? "#2563eb" : "#374151",
              color: "#fff",
              border: "none",
              borderRadius: "4px",
              fontSize: "11px",
              cursor: prompt.trim() && !isGenerating ? "pointer" : "default",
            }}
          >
            {isGenerating ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <CornerDownLeft size={12} />
            )}
            Generate
          </button>

          <button
            onClick={onClose}
            style={{
              display: "flex",
              alignItems: "center",
              padding: "4px",
              backgroundColor: "transparent",
              color: "#9ca3af",
              border: "none",
              borderRadius: "4px",
              cursor: "pointer",
            }}
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Diff Preview if code generated */}
      {generatedCode && (
        <div style={{ display: "flex", flexDirection: "column" }}>
          <div
            style={{
              padding: "8px 12px",
              maxHeight: "160px",
              overflowY: "auto",
              backgroundColor: "#141414",
              fontFamily: "monospace",
              fontSize: "12px",
              lineHeight: "1.4",
            }}
          >
            <div
              style={{
                color: "#ef4444",
                backgroundColor: "rgba(239, 68, 68, 0.1)",
                padding: "4px 6px",
                borderRadius: "2px",
                marginBottom: "4px",
                whiteSpace: "pre-wrap",
              }}
            >
              {selectedText
                .split("\n")
                .map((line) => `- ${line}`)
                .join("\n")}
            </div>
            <div
              style={{
                color: "#22c55e",
                backgroundColor: "rgba(34, 197, 94, 0.1)",
                padding: "4px 6px",
                borderRadius: "2px",
                whiteSpace: "pre-wrap",
              }}
            >
              {generatedCode
                .split("\n")
                .map((line) => `+ ${line}`)
                .join("\n")}
            </div>
          </div>

          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "6px 12px",
              backgroundColor: "#18181b",
              borderTop: "1px solid #27272a",
              fontSize: "11px",
              color: "#a1a1aa",
            }}
          >
            <span>
              Press{" "}
              <kbd style={{ padding: "1px 4px", backgroundColor: "#27272a", borderRadius: "3px" }}>
                Tab
              </kbd>{" "}
              to accept
            </span>
            <div style={{ display: "flex", gap: "6px" }}>
              <button
                onClick={() => {
                  onAccept(generatedCode);
                  onClose();
                }}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "4px",
                  padding: "3px 8px",
                  backgroundColor: "#16a34a",
                  color: "#fff",
                  border: "none",
                  borderRadius: "4px",
                  cursor: "pointer",
                }}
              >
                <Check size={12} /> Accept
              </button>
              <button
                onClick={onClose}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "4px",
                  padding: "3px 8px",
                  backgroundColor: "#27272a",
                  color: "#d4d4d8",
                  border: "none",
                  borderRadius: "4px",
                  cursor: "pointer",
                }}
              >
                <X size={12} /> Reject
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
});
