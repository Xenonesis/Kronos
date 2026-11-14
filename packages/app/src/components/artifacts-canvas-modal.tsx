import React, { memo, useState, useCallback } from "react";
import {
  Layers,
  Code2,
  FileText,
  Copy,
  Download,
  ExternalLink,
  X,
  Maximize2,
  Check,
  Sparkles,
} from "lucide-react-native";
import { useToast } from "@/contexts/toast-context";
import { MermaidFence } from "@/components/markdown/fence/mermaid";
import { MarkdownRenderer } from "@/components/markdown/renderer";
import { HtmlArtifactFrame } from "@/features/artifacts";

export interface ArtifactItem {
  id: string;
  title: string;
  type: "mermaid" | "markdown" | "html";
  content: string;
}

export interface ArtifactsCanvasModalProps {
  visible: boolean;
  artifacts?: ArtifactItem[];
  onClose: () => void;
}

export function extractArtifactsFromMessages(
  messages: Array<{ id?: string; kind?: string; text?: string }>,
): ArtifactItem[] {
  const collected: ArtifactItem[] = [];
  let count = 0;

  for (const msg of messages) {
    if (msg.kind !== "assistant_message" || !msg.text) continue;
    const text = msg.text;
    const fenceRegex = /```(mermaid|html|svg)\s*\r?\n([\s\S]*?)```/gi;
    let match: RegExpExecArray | null = fenceRegex.exec(text);
    while (match !== null) {
      count += 1;
      const lang = match[1].toLowerCase();
      const body = match[2].trim();
      if (body) {
        collected.push({
          id: `artifact-${msg.id ?? "msg"}-${count}`,
          title:
            lang === "mermaid"
              ? `Diagram #${count}`
              : lang === "svg"
                ? `SVG Graphic #${count}`
                : `HTML Sandbox #${count}`,
          type: lang === "mermaid" ? "mermaid" : "html",
          content: body,
        });
      }
      match = fenceRegex.exec(text);
    }

    if (text.length >= 260 && /^#\s+/m.test(text)) {
      count += 1;
      const firstHeading = text.match(/^#\s+(.+)$/m)?.[1]?.trim() ?? `Report #${count}`;
      collected.push({
        id: `artifact-md-${msg.id ?? "msg"}-${count}`,
        title: firstHeading.slice(0, 42),
        type: "markdown",
        content: text,
      });
    }
  }

  return collected;
}

export const ArtifactsCanvasModal = memo(function ArtifactsCanvasModal({
  visible,
  artifacts = [],
  onClose,
}: ArtifactsCanvasModalProps) {
  const [activeArtifactId, setActiveArtifactId] = useState(artifacts[0]?.id ?? "");
  const [copied, setCopied] = useState(false);
  const toast = useToast();

  React.useEffect(() => {
    if (visible && artifacts.length > 0 && !artifacts.some((a) => a.id === activeArtifactId)) {
      setActiveArtifactId(artifacts[0].id);
    }
  }, [visible, artifacts, activeArtifactId]);

  const activeArtifact = artifacts.find((a) => a.id === activeArtifactId) ?? artifacts[0] ?? null;

  const handleCopy = useCallback(() => {
    if (!activeArtifact) return;
    if (navigator?.clipboard?.writeText) {
      navigator.clipboard.writeText(activeArtifact.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
      toast.show("Artifact source copied to clipboard", { variant: "success" });
    }
  }, [activeArtifact, toast]);

  if (!visible) return null;

  return (
    <div
      data-testid="artifacts-canvas-modal"
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0, 0, 0, 0.75)",
        backdropFilter: "blur(6px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 9999,
        padding: "24px",
      }}
    >
      <div
        style={{
          width: "900px",
          maxWidth: "100%",
          height: "80vh",
          backgroundColor: "#121214",
          border: "1px solid #27272a",
          borderRadius: "14px",
          display: "flex",
          flexDirection: "column",
          boxShadow: "0 24px 48px rgba(0, 0, 0, 0.7)",
          color: "#f4f4f5",
          fontFamily: "system-ui, -apple-system, sans-serif",
          overflow: "hidden",
        }}
      >
        {/* Top Bar */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "12px 18px",
            borderBottom: "1px solid #27272a",
            backgroundColor: "#18181b",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <Layers size={18} color="#a855f7" />
            <span style={{ fontWeight: 600, fontSize: "14px" }}>Visual Artifacts Canvas</span>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <button
              onClick={handleCopy}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "5px",
                padding: "5px 10px",
                backgroundColor: "#27272a",
                border: "1px solid #3f3f46",
                borderRadius: "6px",
                color: "#e4e4e7",
                fontSize: "12px",
                cursor: "pointer",
              }}
            >
              {copied ? <Check size={13} color="#22c55e" /> : <Copy size={13} />}
              {copied ? "Copied" : "Copy Source"}
            </button>

            <button
              onClick={onClose}
              style={{
                background: "transparent",
                border: "none",
                color: "#a1a1aa",
                cursor: "pointer",
                padding: "4px",
              }}
            >
              <X size={18} />
            </button>
          </div>
        </div>

        {/* Tab Selector Pills */}
        <div
          style={{
            display: "flex",
            gap: "8px",
            padding: "10px 18px",
            borderBottom: "1px solid #27272a",
            backgroundColor: "#151518",
          }}
        >
          {artifacts.map((a) => {
            const isActive = a.id === activeArtifact?.id;
            return (
              <button
                key={a.id}
                onClick={() => setActiveArtifactId(a.id)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                  padding: "6px 12px",
                  backgroundColor: isActive ? "rgba(168, 85, 247, 0.15)" : "#27272a",
                  border: isActive ? "1px solid #a855f7" : "1px solid transparent",
                  borderRadius: "6px",
                  color: isActive ? "#c084fc" : "#a1a1aa",
                  fontSize: "12px",
                  fontWeight: isActive ? 600 : 400,
                  cursor: "pointer",
                }}
              >
                {a.type === "mermaid" && <Layers size={13} />}
                {a.type === "markdown" && <FileText size={13} />}
                {a.type === "html" && <Code2 size={13} />}
                {a.title}
              </button>
            );
          })}
        </div>

        {/* Canvas Body */}
        <div
          style={{
            flex: 1,
            overflowY: "auto",
            backgroundColor: "#09090b",
            display: "flex",
            flexDirection: "column",
          }}
        >
          {!activeArtifact ? (
            <div
              style={{
                flex: 1,
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                color: "#71717a",
                padding: "32px",
                textAlign: "center",
                gap: "8px",
              }}
            >
              <Layers size={28} />
              <div style={{ fontSize: "14px", fontWeight: 600, color: "#d4d4d8" }}>
                No Session Artifacts Yet
              </div>
              <div style={{ fontSize: "12px", maxWidth: "420px" }}>
                Ask the agent to output a Mermaid diagram, HTML snippet, or Markdown report — or run
                /diagram to generate a live workspace dependency graph.
              </div>
            </div>
          ) : activeArtifact.type === "html" ? (
            <HtmlArtifactFrame
              artifact={{
                id: activeArtifact.id,
                title: activeArtifact.title,
                type: "html",
                content: activeArtifact.content,
                createdAt: Date.now(),
              }}
            />
          ) : activeArtifact.type === "mermaid" ? (
            <div style={{ padding: "24px", maxWidth: "960px", margin: "0 auto", width: "100%" }}>
              <MermaidFence code={activeArtifact.content} />
            </div>
          ) : (
            <div style={{ padding: "24px", maxWidth: "840px", margin: "0 auto", width: "100%" }}>
              <MarkdownRenderer text={activeArtifact.content} selectable />
            </div>
          )}
        </div>
      </div>
    </div>
  );
});
