import { create } from "zustand";

export interface CascadeFlowState {
  isFlowActive: boolean;
  activeFile: string | null;
  cursorLine: number;
  cursorColumn: number;
  recentFiles: string[];
  setActiveFile: (file: string | null, line?: number, col?: number) => void;
  updateCursor: (line: number, col: number) => void;
  toggleFlow: (active?: boolean) => void;
  getFlowContextString: () => string | null;
}

export const useCascadeFlowStore = create<CascadeFlowState>((set, get) => ({
  isFlowActive: true,
  activeFile: null,
  cursorLine: 1,
  cursorColumn: 1,
  recentFiles: [],

  setActiveFile: (file, line = 1, col = 1) => {
    if (!file) {
      set({ activeFile: null });
      return;
    }
    const currentRecent = get().recentFiles;
    const filtered = currentRecent.filter((f) => f !== file);
    const updatedRecent = [file, ...filtered].slice(0, 10);

    set({
      activeFile: file,
      cursorLine: line,
      cursorColumn: col,
      recentFiles: updatedRecent,
    });
  },

  updateCursor: (line, col) => {
    set({ cursorLine: line, cursorColumn: col });
  },

  toggleFlow: (active) => {
    set((state) => ({
      isFlowActive: active !== undefined ? active : !state.isFlowActive,
    }));
  },

  getFlowContextString: () => {
    const { isFlowActive, activeFile, cursorLine, cursorColumn, recentFiles } = get();
    if (!isFlowActive || !activeFile) {
      return null;
    }

    const shortName = activeFile.split("/").pop()?.split("\\").pop() ?? activeFile;
    const otherRecent = recentFiles
      .filter((f) => f !== activeFile)
      .slice(0, 5)
      .map((f) => f.split("/").pop()?.split("\\").pop() ?? f);

    let context = `[Cascade Flow Active Context]\nActive File: ${shortName}:${cursorLine} (col ${cursorColumn})`;
    if (otherRecent.length > 0) {
      context += `\nRecent Workspace Tabs: [${otherRecent.join(", ")}]`;
    }
    return context;
  },
}));
