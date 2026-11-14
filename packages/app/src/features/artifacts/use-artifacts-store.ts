import { create } from "zustand";
import type { Artifact, ArtifactType } from "./types";

export interface ArtifactsState {
  artifacts: Artifact[];
  selectedArtifactId: string | null;
  currentArtifact: Artifact | null;
  isInspectorOpen: boolean;

  addArtifact: (input: {
    id?: string;
    title: string;
    type: ArtifactType;
    content: string;
    createdAt?: number;
  }) => Artifact;
  removeArtifact: (id: string) => void;
  selectArtifact: (id: string | null) => void;
  openInspector: (id?: string) => void;
  closeInspector: () => void;
  clearArtifacts: () => void;
}

export const useArtifactsStore = create<ArtifactsState>((set, get) => ({
  artifacts: [],
  selectedArtifactId: null,
  currentArtifact: null,
  isInspectorOpen: false,

  addArtifact: (input) => {
    const id = input.id || `artifact-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const artifact: Artifact = {
      id,
      title: input.title,
      type: input.type,
      content: input.content,
      createdAt: input.createdAt ?? Date.now(),
    };

    set((state) => {
      const existingIdx = state.artifacts.findIndex((a) => a.id === id);
      const updatedArtifacts =
        existingIdx >= 0
          ? state.artifacts.map((item, idx) => (idx === existingIdx ? artifact : item))
          : [artifact, ...state.artifacts];

      return {
        artifacts: updatedArtifacts,
        selectedArtifactId: id,
        currentArtifact: artifact,
      };
    });

    return artifact;
  },

  removeArtifact: (id) => {
    set((state) => {
      const nextArtifacts = state.artifacts.filter((a) => a.id !== id);
      const isSelected = state.selectedArtifactId === id;
      const nextSelectedId = isSelected ? (nextArtifacts[0]?.id ?? null) : state.selectedArtifactId;
      const nextCurrent = nextArtifacts.find((a) => a.id === nextSelectedId) ?? null;

      return {
        artifacts: nextArtifacts,
        selectedArtifactId: nextSelectedId,
        currentArtifact: nextCurrent,
      };
    });
  },

  selectArtifact: (id) => {
    set((state) => {
      const found = id ? state.artifacts.find((a) => a.id === id) ?? null : null;
      return {
        selectedArtifactId: id,
        currentArtifact: found,
      };
    });
  },

  openInspector: (id) => {
    set((state) => {
      const targetId = id ?? state.selectedArtifactId ?? state.artifacts[0]?.id ?? null;
      const targetArtifact = targetId
        ? state.artifacts.find((a) => a.id === targetId) ?? state.currentArtifact
        : state.currentArtifact;

      return {
        isInspectorOpen: true,
        selectedArtifactId: targetId,
        currentArtifact: targetArtifact,
      };
    });
  },

  closeInspector: () => {
    set({ isInspectorOpen: false });
  },

  clearArtifacts: () => {
    set({
      artifacts: [],
      selectedArtifactId: null,
      currentArtifact: null,
      isInspectorOpen: false,
    });
  },
}));

/**
 * Extracts any HTML, SVG, React, or Markdown artifacts from message text or raw snippet.
 */
export function extractArtifactsFromSnippet(
  text: string,
  sourceId: string = "source",
): Artifact[] {
  const artifacts: Artifact[] = [];
  let count = 0;

  // Match ```html, ```svg, ```jsx, ```tsx, ```react
  const fenceRegex = /```(html|svg|jsx|tsx|react|markdown|md)\s*\r?\n([\s\S]*?)```/gi;
  let match: RegExpExecArray | null = fenceRegex.exec(text);

  while (match !== null) {
    count += 1;
    const rawLang = match[1].toLowerCase();
    const content = match[2].trim();

    if (content) {
      let type: ArtifactType = "html";
      let label = "HTML Sandbox";

      if (rawLang === "svg" || content.startsWith("<svg")) {
        type = "svg";
        label = "SVG Graphic";
      } else if (rawLang === "jsx" || rawLang === "tsx" || rawLang === "react") {
        type = "react";
        label = "React Component";
      } else if (rawLang === "markdown" || rawLang === "md") {
        type = "markdown";
        label = "Markdown Doc";
      }

      // Try to extract title from content
      let title = `${label} #${count}`;
      const titleMatch = content.match(/<title>([^<]+)<\/title>/i);
      if (titleMatch?.[1]) {
        title = titleMatch[1].trim();
      }

      artifacts.push({
        id: `artifact-${sourceId}-${count}`,
        title,
        type,
        content,
        createdAt: Date.now(),
      });
    }

    match = fenceRegex.exec(text);
  }

  // Also check if text is a standalone full HTML document without code fences
  if (artifacts.length === 0 && (/<html[\s>]/i.test(text) || /<!doctype html>/i.test(text))) {
    const titleMatch = text.match(/<title>([^<]+)<\/title>/i);
    artifacts.push({
      id: `artifact-${sourceId}-doc`,
      title: titleMatch?.[1]?.trim() ?? "HTML Document",
      type: "html",
      content: text.trim(),
      createdAt: Date.now(),
    });
  }

  // Also check if text is a standalone SVG
  if (artifacts.length === 0 && /<svg[\s>]/i.test(text) && /<\/svg>/i.test(text)) {
    artifacts.push({
      id: `artifact-${sourceId}-svg`,
      title: "SVG Graphic",
      type: "svg",
      content: text.trim(),
      createdAt: Date.now(),
    });
  }

  return artifacts;
}
