import { create } from "zustand";
import type {
  QuickComposerAgent,
  QuickComposerOptions,
  QuickComposerTarget,
} from "./types";

export interface QuickComposerStoreState {
  isOpen: boolean;
  target: QuickComposerTarget;
  selectedAgent: QuickComposerAgent;
  prompt: string;
  isDispatching: boolean;

  open: (options?: QuickComposerOptions) => void;
  close: () => void;
  toggle: (options?: QuickComposerOptions) => void;
  setTarget: (target: QuickComposerTarget) => void;
  setSelectedAgent: (agent: QuickComposerAgent) => void;
  setPrompt: (prompt: string) => void;
  setIsDispatching: (isDispatching: boolean) => void;
  reset: () => void;
}

export const useQuickComposerStore = create<QuickComposerStoreState>((set) => ({
  isOpen: false,
  target: "workspace",
  selectedAgent: "claude",
  prompt: "",
  isDispatching: false,

  open: (options) => {
    set((state) => ({
      isOpen: true,
      prompt: options?.initialPrompt ?? state.prompt,
      target: options?.target ?? state.target,
      selectedAgent: options?.agent ?? state.selectedAgent,
    }));
  },

  close: () => {
    set({ isOpen: false, isDispatching: false });
  },

  toggle: (options) => {
    set((state) => ({
      isOpen: !state.isOpen,
      prompt: options?.initialPrompt ?? state.prompt,
      target: options?.target ?? state.target,
      selectedAgent: options?.agent ?? state.selectedAgent,
      isDispatching: false,
    }));
  },

  setTarget: (target) => {
    set({ target });
  },

  setSelectedAgent: (selectedAgent) => {
    set({ selectedAgent });
  },

  setPrompt: (prompt) => {
    set({ prompt });
  },

  setIsDispatching: (isDispatching) => {
    set({ isDispatching });
  },

  reset: () => {
    set({
      prompt: "",
      isDispatching: false,
    });
  },
}));

export function useQuickComposer() {
  return useQuickComposerStore();
}
