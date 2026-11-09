import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import AsyncStorage from "@react-native-async-storage/async-storage";

interface PinnedMessagesState {
  pinnedByScope: Record<string, string[]>;
  togglePin: (scopeKey: string, messageId: string) => boolean;
  isPinned: (scopeKey: string, messageId: string) => boolean;
  getPinnedIds: (scopeKey: string) => string[];
}

export const usePinnedMessagesStore = create<PinnedMessagesState>()(
  persist(
    (set, get) => ({
      pinnedByScope: {},
      togglePin: (scopeKey, messageId) => {
        const current = get().pinnedByScope[scopeKey] ?? [];
        const exists = current.includes(messageId);
        const next = exists ? current.filter((id) => id !== messageId) : [...current, messageId];
        set((state) => ({
          pinnedByScope: {
            ...state.pinnedByScope,
            [scopeKey]: next,
          },
        }));
        return !exists;
      },
      isPinned: (scopeKey, messageId) => {
        const current = get().pinnedByScope[scopeKey] ?? [];
        return current.includes(messageId);
      },
      getPinnedIds: (scopeKey) => {
        return get().pinnedByScope[scopeKey] ?? [];
      },
    }),
    {
      name: "@paseo:pinned-messages-store",
      storage: createJSONStorage(() => AsyncStorage),
    },
  ),
);
