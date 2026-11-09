import { create } from "zustand";

interface AssistantMessageViewState {
  rawMessageIds: Record<string, boolean>;
  toggleRaw: (messageId: string) => void;
  isRaw: (messageId: string) => boolean;
}

export const useAssistantMessageViewStore = create<AssistantMessageViewState>((set, get) => ({
  rawMessageIds: {},
  toggleRaw: (messageId) => {
    set((state) => ({
      rawMessageIds: {
        ...state.rawMessageIds,
        [messageId]: !state.rawMessageIds[messageId],
      },
    }));
  },
  isRaw: (messageId) => Boolean(get().rawMessageIds[messageId]),
}));
