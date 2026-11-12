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

const DEFAULT_ARTIFACTS: ArtifactItem[] = [
  {
    id: "arch-1",
    title: "System Architecture Flow",
    type: "mermaid",
    content: `graph TD
  User((User)) -->|Prompt / CodeLens| KronosUI[Kronos React Desktop UI]
  KronosUI -->|Zero-Copy SHM / IPC| RustKernel[Kronos Native Rust Kernel]
  RustKernel -->|Diagnostics Probe| ShadowLinter[Background Shadow Linter]
  RustKernel -->|Headless Process| OpenCode[OpenCode / Beam AI Engine]
  OpenCode -->|Multi-Model Debate| Consensus[Consensus Synthesizer]
  RustKernel -->|Branch & PR Ops| GitHubOps[1-Click GitHub PR Engine]
  RustKernel -->|Sub-millisecond| Ripgrep[Ripgrep Batch Search Engine]`,
  },
  {
    id: "spec-1",
    title: "Technical Specification",
    type: "markdown",
    content: `# Kronos Enterprise Parity Architecture Specification

## Overview
Kronos unites extreme client responsiveness (zero-node native Rust runtime) with deep frontier agent capabilities:

- **Background Shadow Linter**: Continuous compiler/typecheck verification after every file edit.
- **Inline Ctrl+K Micro-Composer**: Context-aware in-place refactor and code transformation inside CodeMirror.
- **1-Click GitHub PR Engine**: Automated branch staging, git push, and AI-synthesized PR generation.
- **Global Ripgrep Search & Batch Replace**: Instant regex project-wide text manipulation.
- **Visual Artifacts Canvas**: Real-time architecture diagrams, markdown deliverables, and live HTML sandboxes.`,
  },
  {
    id: "html-1",
    title: "Interactive Component Sandbox",
    type: "html",
    content: `<!DOCTYPE html>
<html>
<head>
  <style>
    body { font-family: system-ui, sans-serif; background: #09090b; color: #f4f4f5; padding: 24px; }
    .card { background: #18181b; border: 1px solid #27272a; border-radius: 12px; padding: 20px; max-width: 480px; }
    .badge { display: inline-block; background: #2563eb; color: #fff; border-radius: 20px; padding: 4px 10px; font-size: 11px; font-weight: 600; }
    h2 { margin: 12px 0 8px 0; font-size: 18px; }
    p { color: #a1a1aa; font-size: 13px; line-height: 1.5; }
    button { background: #3b82f6; border: none; border-radius: 6px; color: #fff; padding: 8px 16px; cursor: pointer; font-weight: 500; }
    button:hover { background: #2563eb; }
  </style>
</head>
<body>
  <div class="card">
    <div class="badge">LIVE PREVIEW</div>
    <h2>Kronos Native Component Sandbox</h2>
    <p>This interactive component renders in an isolated sandbox. Edit the prompt or source to see real-time updates.</p>
    <button onclick="alert('Action dispatched!')">Interact with Component</button>
  </div>
</body>
</html>`,
  },
];

export const ArtifactsCanvasModal = memo(function ArtifactsCanvasModal({
  visible,
  artifacts = DEFAULT_ARTIFACTS,
  onClose,
}: ArtifactsCanvasModalProps) {
  const [activeArtifactId, setActiveArtifactId] = useState(artifacts[0]?.id ?? "");
  const [copied, setCopied] = useState(false);
  const toast = useToast();

  const activeArtifact = artifacts.find((a) => a.id === activeArtifactId) ?? artifacts[0];

  const handleCopy = useCallback(() => {
    if (!activeArtifact) return;
    if (navigator?.clipboard?.writeText) {
      navigator.clipboard.writeText(activeArtifact.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
      toast.show("Artifact source copied to clipboard", { variant: "success" });
    }
  }, [activeArtifact, toast]);

  if (!visible || !activeArtifact) return null;

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
            const isActive = a.id === activeArtifact.id;
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
          {activeArtifact.type === "html" ? (
            <iframe
              srcDoc={activeArtifact.content}
              title={activeArtifact.title}
              style={{
                flex: 1,
                border: "none",
                width: "100%",
                height: "100%",
                backgroundColor: "#09090b",
              }}
            />
          ) : (
            <div style={{ padding: "24px", maxWidth: "800px", margin: "0 auto", width: "100%" }}>
              <pre
                style={{
                  fontFamily: "monospace",
                  fontSize: "13px",
                  lineHeight: "1.6",
                  color: "#e4e4e7",
                  backgroundColor: "#18181b",
                  padding: "16px",
                  borderRadius: "8px",
                  border: "1px solid #27272a",
                  overflowX: "auto",
                  whiteSpace: "pre-wrap",
                }}
              >
                {activeArtifact.content}
              </pre>
            </div>
          )}
        </div>
      </div>
    </div>
  );
});
