import React, { memo, useState, useCallback, useEffect } from "react";
import { Check, X, ChevronDown, ChevronUp } from "lucide-react-native";
import { buildLineDiff, type DiffLine } from "@/utils/tool-call-parsers";

export interface DiffHunk {
  id: string;
  startLine: number;
  oldLines: string[];
  newLines: string[];
  diffs: DiffLine[];
}

export interface InlineDiffHunkViewerProps {
  visible: boolean;
  filename: string;
  originalContent: string;
  modifiedContent: string;
  onAcceptHunk: (hunk: DiffHunk) => void;
  onRejectHunk: (hunkId: string) => void;
  onAcceptAll: () => void;
  onRejectAll: () => void;
  onClose: () => void;
}

export const InlineDiffHunkViewer = memo(function InlineDiffHunkViewer({
  visible,
  filename,
  originalContent,
  modifiedContent,
  onAcceptHunk,
  onRejectHunk,
  onAcceptAll,
  onRejectAll,
  onClose,
}: InlineDiffHunkViewerProps) {
  const [activeHunkIndex, setActiveHunkIndex] = useState(0);

  const hunks = React.useMemo(() => {
    if (!originalContent || !modifiedContent || originalContent === modifiedContent) {
      return [];
    }
    const diffs = buildLineDiff(originalContent, modifiedContent);
    const result: DiffHunk[] = [];
    let currentOld: string[] = [];
    let currentNew: string[] = [];
    let currentDiffs: DiffLine[] = [];
    let hunkStartLine = 1;
    let lineCounter = 1;

    for (let i = 0; i < diffs.length; i++) {
      const d = diffs[i];
      if (d.type === "context") {
        if (currentOld.length > 0 || currentNew.length > 0) {
          result.push({
            id: `hunk-${result.length + 1}`,
            startLine: hunkStartLine,
            oldLines: [...currentOld],
            newLines: [...currentNew],
            diffs: [...currentDiffs],
          });
          currentOld = [];
          currentNew = [];
          currentDiffs = [];
        }
        lineCounter++;
      } else {
        if (currentOld.length === 0 && currentNew.length === 0) {
          hunkStartLine = lineCounter;
        }
        currentDiffs.push(d);
        if (d.type === "remove") {
          currentOld.push(d.content);
          lineCounter++;
        } else if (d.type === "add") {
          currentNew.push(d.content);
        }
      }
    }

    if (currentOld.length > 0 || currentNew.length > 0) {
      result.push({
        id: `hunk-${result.length + 1}`,
        startLine: hunkStartLine,
        oldLines: currentOld,
        newLines: currentNew,
        diffs: currentDiffs,
      });
    }

    return result;
  }, [originalContent, modifiedContent]);

  // Handle Tab (Accept) and Esc (Reject) keyboard shortcuts
  useEffect(() => {
    if (!visible || hunks.length === 0) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Tab" && !e.shiftKey && !e.ctrlKey) {
        e.preventDefault();
        const current = hunks[activeHunkIndex];
        if (current) {
          onAcceptHunk(current);
          if (activeHunkIndex >= hunks.length - 1) {
            onClose();
          }
        }
      } else if (e.key === "Escape") {
        e.preventDefault();
        const current = hunks[activeHunkIndex];
        if (current) {
          onRejectHunk(current.id);
          if (activeHunkIndex >= hunks.length - 1) {
            onClose();
          }
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [visible, hunks, activeHunkIndex, onAcceptHunk, onRejectHunk, onClose]);

  if (!visible || hunks.length === 0) {
    return null;
  }

  const currentHunk = hunks[activeHunkIndex] || hunks[0];

  const containerStyle: React.CSSProperties = {
    position: "absolute",
    bottom: 24,
    right: 24,
    zIndex: 900,
    background: "#18181b",
    border: "1px solid #3f3f46",
    borderRadius: "10px",
    padding: "12px",
    boxShadow: "0 8px 24px rgba(0, 0, 0, 0.5)",
    maxWidth: "520px",
    minWidth: "360px",
    fontFamily: "monospace",
    fontSize: "12px",
    userSelect: "none",
  };

  const headerStyle: React.CSSProperties = {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: "8px",
    color: "#e4e4e7",
  };

  const buttonStyle: React.CSSProperties = {
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
    padding: "4px 8px",
    borderRadius: "6px",
    fontSize: "11px",
    fontWeight: 600,
    cursor: "pointer",
    border: "none",
  };

  return (
    <div style={containerStyle} data-testid="inline-diff-hunk-viewer">
      <div style={headerStyle}>
        <span style={{ fontWeight: 600, fontSize: "11px", color: "#a1a1aa" }}>
          Inline Diff: {filename} (Hunk {activeHunkIndex + 1} of {hunks.length})
        </span>
        <div style={{ display: "flex", gap: "6px" }}>
          <button
            type="button"
            disabled={activeHunkIndex === 0}
            onClick={() => setActiveHunkIndex((prev) => Math.max(0, prev - 1))}
            style={{ ...buttonStyle, background: "#27272a", color: "#d4d4d8" }}
            title="Previous Hunk"
          >
            <ChevronUp size={12} />
          </button>
          <button
            type="button"
            disabled={activeHunkIndex >= hunks.length - 1}
            onClick={() => setActiveHunkIndex((prev) => Math.min(hunks.length - 1, prev + 1))}
            style={{ ...buttonStyle, background: "#27272a", color: "#d4d4d8" }}
            title="Next Hunk"
          >
            <ChevronDown size={12} />
          </button>
          <button
            type="button"
            onClick={onClose}
            style={{ ...buttonStyle, background: "transparent", color: "#71717a" }}
            title="Close Diff"
          >
            <X size={12} />
          </button>
        </div>
      </div>

      {/* Diff preview box */}
      <div
        style={{
          background: "#09090b",
          border: "1px solid #27272a",
          borderRadius: "6px",
          padding: "8px",
          maxHeight: "160px",
          overflowY: "auto",
          marginBottom: "10px",
        }}
      >
        {currentHunk.diffs.map((d, idx) => (
          <div
            key={idx}
            style={{
              color: d.type === "add" ? "#4ade80" : d.type === "remove" ? "#f87171" : "#71717a",
              background:
                d.type === "add"
                  ? "rgba(74, 222, 128, 0.1)"
                  : d.type === "remove"
                    ? "rgba(248, 113, 113, 0.1)"
                    : "transparent",
              padding: "1px 4px",
              borderRadius: "2px",
              whiteSpace: "pre-wrap",
              wordBreak: "break-all",
            }}
          >
            {d.type === "add" ? "+ " : d.type === "remove" ? "- " : "  "}
            {d.content}
          </div>
        ))}
      </div>

      {/* Actions */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div style={{ display: "flex", gap: "6px" }}>
          <button
            type="button"
            onClick={() => {
              onAcceptHunk(currentHunk);
              if (activeHunkIndex >= hunks.length - 1) onClose();
            }}
            style={{ ...buttonStyle, background: "#166534", color: "#bbf7d0" }}
          >
            <Check size={12} />
            <span>Accept (Tab)</span>
          </button>
          <button
            type="button"
            onClick={() => {
              onRejectHunk(currentHunk.id);
              if (activeHunkIndex >= hunks.length - 1) onClose();
            }}
            style={{ ...buttonStyle, background: "#7f1d1d", color: "#fecaca" }}
          >
            <X size={12} />
            <span>Reject (Esc)</span>
          </button>
        </div>
        <div style={{ display: "flex", gap: "6px" }}>
          <button
            type="button"
            onClick={onAcceptAll}
            style={{ ...buttonStyle, background: "#27272a", color: "#a1a1aa" }}
          >
            Accept All
          </button>
          <button
            type="button"
            onClick={onRejectAll}
            style={{ ...buttonStyle, background: "#27272a", color: "#a1a1aa" }}
          >
            Reject All
          </button>
        </div>
      </div>
    </div>
  );
});
