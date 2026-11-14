import React, { useState, useCallback } from "react";
import { Sparkles, Eye } from "lucide-react-native";
import { useArtifactsStore } from "./use-artifacts-store";
import type { Artifact, ArtifactSnippetChipProps } from "./types";

export const ArtifactSnippetChip: React.FC<ArtifactSnippetChipProps> = ({
  artifact,
  artifactId,
  title,
  type = "html",
  content = "",
  onClick,
  compact = false,
  style,
}) => {
  const [isHovered, setIsHovered] = useState(false);
  const addArtifact = useArtifactsStore((s) => s.addArtifact);
  const openInspector = useArtifactsStore((s) => s.openInspector);

  const effectiveTitle = artifact?.title || title || (type === "svg" ? "SVG Graphic" : "HTML Sandbox");
  const effectiveType = artifact?.type || type;
  const effectiveContent = artifact?.content || content;

  const handleClick = useCallback(
    (e: React.MouseEvent | React.KeyboardEvent) => {
      e.stopPropagation();

      let targetId = artifact?.id || artifactId;

      if (!targetId && effectiveContent) {
        const created = addArtifact({
          title: effectiveTitle,
          type: effectiveType,
          content: effectiveContent,
        });
        targetId = created.id;
      } else if (artifact) {
        addArtifact(artifact);
        targetId = artifact.id;
      }

      openInspector(targetId);
      onClick?.();
    },
    [
      artifact,
      artifactId,
      effectiveTitle,
      effectiveType,
      effectiveContent,
      addArtifact,
      openInspector,
      onClick,
    ],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handleClick(e);
      }
    },
    [handleClick],
  );

  return (
    <div
      role="button"
      tabIndex={0}
      data-testid="artifact-snippet-chip"
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: compact ? "6px" : "8px",
        padding: compact ? "4px 10px" : "6px 14px",
        borderRadius: "9999px",
        backgroundColor: isHovered
          ? "rgba(168, 85, 247, 0.22)"
          : "rgba(168, 85, 247, 0.12)",
        border: isHovered
          ? "1px solid #c084fc"
          : "1px solid rgba(168, 85, 247, 0.35)",
        boxShadow: isHovered
          ? "0 0 14px rgba(168, 85, 247, 0.3)"
          : "0 2px 6px rgba(0, 0, 0, 0.3)",
        cursor: "pointer",
        color: "#f4f4f5",
        fontFamily: "system-ui, -apple-system, sans-serif",
        fontSize: compact ? "11px" : "12px",
        fontWeight: 500,
        transition: "all 0.18s ease-in-out",
        userSelect: "none",
        outline: "none",
        margin: "4px 0",
        ...style,
      }}
    >
      <span
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: compact ? "12px" : "14px",
        }}
      >
        🎨
      </span>

      <span
        style={{
          color: "#e4e4e7",
          letterSpacing: "0.01em",
        }}
      >
        Preview Artifact: <strong style={{ color: "#ffffff", fontWeight: 600 }}>{effectiveTitle}</strong>
      </span>

      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: "3px",
          padding: "1px 6px",
          borderRadius: "4px",
          backgroundColor: "rgba(168, 85, 247, 0.25)",
          color: "#d8b4fe",
          fontSize: "10px",
          fontWeight: 700,
          textTransform: "uppercase",
          letterSpacing: "0.04em",
        }}
      >
        {effectiveType}
      </span>

      <Eye size={compact ? 11 : 13} color="#c084fc" style={{ marginLeft: 2 }} />
    </div>
  );
};
