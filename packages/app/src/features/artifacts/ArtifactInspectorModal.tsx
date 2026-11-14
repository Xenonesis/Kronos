import React, { memo, useState, useEffect, useCallback } from "react";
import {
  Layers,
  X,
  Maximize2,
  Minimize2,
  Sparkles,
  Code,
  FileText,
  Clock,
} from "lucide-react-native";
import { useArtifactsStore } from "./use-artifacts-store";
import { HtmlArtifactFrame } from "./HtmlArtifactFrame";
import type { Artifact, ArtifactInspectorModalProps } from "./types";

export const ArtifactInspectorModal = memo(function ArtifactInspectorModal({
  visible,
  artifact: propArtifact,
  artifacts: propArtifacts,
  onClose: propOnClose,
}: ArtifactInspectorModalProps) {
  // Store integration
  const storeArtifacts = useArtifactsStore((s) => s.artifacts);
  const storeCurrent = useArtifactsStore((s) => s.currentArtifact);
  const isStoreOpen = useArtifactsStore((s) => s.isInspectorOpen);
  const closeStoreInspector = useArtifactsStore((s) => s.closeInspector);
  const selectStoreArtifact = useArtifactsStore((s) => s.selectArtifact);

  // Controlled vs store-driven visibility
  const isVisible = visible !== undefined ? visible : isStoreOpen;
  const artifactsList = propArtifacts ?? storeArtifacts;

  const [isFullscreen, setIsFullscreen] = useState(false);
  const [internalSelectedId, setInternalSelectedId] = useState<string | null>(
    propArtifact?.id ?? storeCurrent?.id ?? artifactsList[0]?.id ?? null,
  );

  // Sync selection when props or store change
  useEffect(() => {
    if (propArtifact?.id) {
      setInternalSelectedId(propArtifact.id);
    } else if (storeCurrent?.id) {
      setInternalSelectedId(storeCurrent.id);
    } else if (artifactsList.length > 0 && !internalSelectedId) {
      setInternalSelectedId(artifactsList[0].id);
    }
  }, [propArtifact, storeCurrent, artifactsList, internalSelectedId]);

  const activeArtifact: Artifact | null =
    (propArtifact?.id === internalSelectedId ? propArtifact : null) ??
    artifactsList.find((a) => a.id === internalSelectedId) ??
    propArtifact ??
    storeCurrent ??
    artifactsList[0] ??
    null;

  const handleClose = useCallback(() => {
    if (propOnClose) {
      propOnClose();
    } else {
      closeStoreInspector();
    }
  }, [propOnClose, closeStoreInspector]);

  // Handle ESC key to dismiss
  useEffect(() => {
    if (!isVisible) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        handleClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isVisible, handleClose]);

  if (!isVisible) return null;

  return (
    <div
      data-testid="artifact-inspector-modal"
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0, 0, 0, 0.78)",
        backdropFilter: "blur(8px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 99999,
        padding: isFullscreen ? "0px" : "24px",
        transition: "all 0.2s ease-in-out",
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          handleClose();
        }
      }}
    >
      <div
        style={{
          width: isFullscreen ? "100vw" : "1100px",
          maxWidth: "100%",
          height: isFullscreen ? "100vh" : "88vh",
          backgroundColor: "#121214",
          border: isFullscreen ? "none" : "1px solid #27272a",
          borderRadius: isFullscreen ? "0px" : "16px",
          display: "flex",
          flexDirection: "column",
          boxShadow: "0 30px 70px rgba(0, 0, 0, 0.8)",
          color: "#f4f4f5",
          fontFamily: "system-ui, -apple-system, sans-serif",
          overflow: "hidden",
        }}
      >
        {/* Modal Top Header */}
        <div
          data-testid="artifact-inspector-header"
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "12px 18px",
            borderBottom: "1px solid #27272a",
            backgroundColor: "#18181b",
            gap: "12px",
          }}
        >
          {/* Left Title / Type info */}
          <div style={{ display: "flex", alignItems: "center", gap: "10px", minWidth: 0 }}>
            <div
              style={{
                width: "28px",
                height: "28px",
                borderRadius: "8px",
                backgroundColor: "rgba(168, 85, 247, 0.15)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Sparkles size={16} color="#a855f7" />
            </div>

            <div style={{ display: "flex", flexDirection: "column", minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <span
                  style={{
                    fontWeight: 600,
                    fontSize: "14px",
                    color: "#f4f4f5",
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    maxWidth: "340px",
                  }}
                >
                  {activeArtifact?.title ?? "Web Artifact Inspector"}
                </span>

                {activeArtifact && (
                  <span
                    style={{
                      padding: "2px 7px",
                      borderRadius: "4px",
                      fontSize: "10px",
                      fontWeight: 700,
                      textTransform: "uppercase",
                      backgroundColor: "rgba(168, 85, 247, 0.2)",
                      color: "#c084fc",
                      letterSpacing: "0.03em",
                    }}
                  >
                    {activeArtifact.type}
                  </span>
                )}
              </div>

              {activeArtifact?.createdAt && (
                <span
                  style={{
                    fontSize: "11px",
                    color: "#71717a",
                    display: "flex",
                    alignItems: "center",
                    gap: "4px",
                  }}
                >
                  <Clock size={10} />
                  {new Date(activeArtifact.createdAt).toLocaleTimeString()}
                </span>
              )}
            </div>
          </div>

          {/* Right Header Controls */}
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            {/* Fullscreen Toggle */}
            <button
              data-testid="artifact-fullscreen-btn"
              onClick={() => setIsFullscreen(!isFullscreen)}
              title={isFullscreen ? "Exit Fullscreen" : "Fullscreen"}
              style={{
                background: "transparent",
                border: "1px solid #3f3f46",
                borderRadius: "6px",
                color: "#a1a1aa",
                cursor: "pointer",
                padding: "6px",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              {isFullscreen ? <Minimize2 size={15} /> : <Maximize2 size={15} />}
            </button>

            {/* Close Modal Button */}
            <button
              data-testid="artifact-close-modal-btn"
              onClick={handleClose}
              title="Close (Esc)"
              style={{
                background: "transparent",
                border: "1px solid #3f3f46",
                borderRadius: "6px",
                color: "#a1a1aa",
                cursor: "pointer",
                padding: "6px",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <X size={15} />
            </button>
          </div>
        </div>

        {/* Tab Strip if multiple artifacts */}
        {artifactsList.length > 1 && (
          <div
            data-testid="artifact-tabs-strip"
            style={{
              display: "flex",
              alignItems: "center",
              gap: "6px",
              padding: "8px 16px",
              backgroundColor: "#151518",
              borderBottom: "1px solid #27272a",
              overflowX: "auto",
            }}
          >
            <span style={{ fontSize: "11px", color: "#71717a", marginRight: "4px" }}>
              ARTIFACTS ({artifactsList.length}):
            </span>
            {artifactsList.map((art) => {
              const isSelected = art.id === activeArtifact?.id;
              return (
                <button
                  key={art.id}
                  onClick={() => {
                    setInternalSelectedId(art.id);
                    selectStoreArtifact(art.id);
                  }}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "6px",
                    padding: "5px 12px",
                    borderRadius: "6px",
                    fontSize: "11px",
                    fontWeight: isSelected ? 600 : 400,
                    backgroundColor: isSelected ? "rgba(168, 85, 247, 0.18)" : "#27272a",
                    border: isSelected ? "1px solid #a855f7" : "1px solid transparent",
                    color: isSelected ? "#c084fc" : "#a1a1aa",
                    cursor: "pointer",
                    whiteSpace: "nowrap",
                  }}
                >
                  {art.type === "svg" && <Layers size={12} />}
                  {art.type === "html" && <Code size={12} />}
                  {art.type === "markdown" && <FileText size={12} />}
                  {art.title}
                </button>
              );
            })}
          </div>
        )}

        {/* Modal Main Body */}
        <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
          {!activeArtifact ? (
            <div
              style={{
                flex: 1,
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                color: "#71717a",
                gap: "12px",
                padding: "32px",
                textAlign: "center",
              }}
            >
              <Sparkles size={32} color="#a855f7" />
              <div style={{ fontSize: "15px", fontWeight: 600, color: "#e4e4e7" }}>
                No Artifact Selected
              </div>
              <div style={{ fontSize: "12px", maxWidth: "400px" }}>
                Generate or detect an HTML, SVG, or React snippet to preview it in this sandboxed frame.
              </div>
            </div>
          ) : (
            <HtmlArtifactFrame artifact={activeArtifact} />
          )}
        </div>
      </div>
    </div>
  );
});
