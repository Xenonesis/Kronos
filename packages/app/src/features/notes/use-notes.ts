import { create } from "zustand";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { invokeTauri } from "@/utils/tauri-invoke";
import type { NoteRecord, CreateNoteInput, UpdateNoteInput } from "./types";

const NOTES_STORAGE_KEY = "@kronos/notes_v1";

const INITIAL_SEED_NOTES: NoteRecord[] = [
  {
    id: "note-welcome",
    title: "Welcome to Kronos Notes & Scratchpad",
    content: `# Kronos Scratchpad & Context Notes

Use this drawer to store code snippets, architecture thoughts, API contracts, or prompt templates.

## Quick Features
- **Insert into Chat**: Click the button below to paste note context directly into the composer.
- **Composer @ Mention**: Type \`@note\` in your prompt to search and inject note contents on the fly.
- **Pinning**: Pin critical project guides to the top for easy 1-click access.
- **Auto-Sync**: Automatically synchronized with native Kronos desktop storage and persistent cache.
`,
    tags: ["guide", "scratchpad"],
    pinned: true,
    createdAtMs: Date.now() - 3600000,
    updatedAtMs: Date.now() - 3600000,
  },
];

export interface NotesState {
  notes: NoteRecord[];
  selectedNoteId: string | null;
  isDrawerOpen: boolean;
  isLoading: boolean;
  searchQuery: string;
  selectedTag: string | null;
  pinnedOnly: boolean;

  openDrawer: (noteId?: string) => void;
  closeDrawer: () => void;
  toggleDrawer: () => void;
  selectNote: (id: string | null) => void;
  setSearchQuery: (query: string) => void;
  setSelectedTag: (tag: string | null) => void;
  setPinnedOnly: (pinned: boolean) => void;

  loadNotes: () => Promise<void>;
  saveNote: (input: {
    id?: string;
    title?: string;
    content?: string;
    tags?: string[];
    pinned?: boolean;
  }) => Promise<NoteRecord>;
  deleteNote: (id: string) => Promise<boolean>;
  togglePin: (id: string) => Promise<void>;
  formatNoteAsContext: (note: NoteRecord) => string;
}

export const useNotesStore = create<NotesState>((set, get) => ({
  notes: INITIAL_SEED_NOTES,
  selectedNoteId: INITIAL_SEED_NOTES[0]?.id ?? null,
  isDrawerOpen: false,
  isLoading: false,
  searchQuery: "",
  selectedTag: null,
  pinnedOnly: false,

  openDrawer: (noteId) => {
    set((state) => ({
      isDrawerOpen: true,
      selectedNoteId: noteId ?? state.selectedNoteId ?? (state.notes[0]?.id ?? null),
    }));
  },

  closeDrawer: () => {
    set({ isDrawerOpen: false });
  },

  toggleDrawer: () => {
    set((state) => ({
      isDrawerOpen: !state.isDrawerOpen,
      selectedNoteId: !state.isDrawerOpen
        ? (state.selectedNoteId ?? state.notes[0]?.id ?? null)
        : state.selectedNoteId,
    }));
  },

  selectNote: (id) => {
    set({ selectedNoteId: id });
  },

  setSearchQuery: (searchQuery) => {
    set({ searchQuery });
  },

  setSelectedTag: (selectedTag) => {
    set({ selectedTag });
  },

  setPinnedOnly: (pinnedOnly) => {
    set({ pinnedOnly });
  },

  loadNotes: async () => {
    set({ isLoading: true });
    try {
      // 1. Try native Tauri bridge command
      const tauriNotes = await invokeTauri<NoteRecord[]>("list_notes");
      if (Array.isArray(tauriNotes) && tauriNotes.length > 0) {
        set({
          notes: tauriNotes,
          selectedNoteId: get().selectedNoteId ?? tauriNotes[0]?.id ?? null,
          isLoading: false,
        });
        await AsyncStorage.setItem(NOTES_STORAGE_KEY, JSON.stringify(tauriNotes)).catch(() => {});
        return;
      }

      // 2. Fall back to local AsyncStorage
      const rawStored = await AsyncStorage.getItem(NOTES_STORAGE_KEY);
      if (rawStored) {
        const parsed = JSON.parse(rawStored) as NoteRecord[];
        if (Array.isArray(parsed) && parsed.length > 0) {
          set({
            notes: parsed,
            selectedNoteId: get().selectedNoteId ?? parsed[0]?.id ?? null,
            isLoading: false,
          });
          return;
        }
      }

      // 3. Fall back to seed notes
      set({ notes: INITIAL_SEED_NOTES, isLoading: false });
      await AsyncStorage.setItem(NOTES_STORAGE_KEY, JSON.stringify(INITIAL_SEED_NOTES)).catch(() => {});
    } catch {
      set({ isLoading: false });
    }
  },

  saveNote: async (input) => {
    const now = Date.now();
    const existing = input.id ? get().notes.find((n) => n.id === input.id) : undefined;
    const noteRecord: NoteRecord = {
      id: input.id || `note-${now}-${Math.random().toString(36).substring(2, 7)}`,
      title: input.title ?? existing?.title ?? "Untitled Note",
      content: input.content ?? existing?.content ?? "",
      tags: input.tags ?? existing?.tags ?? [],
      pinned: input.pinned ?? existing?.pinned ?? false,
      createdAtMs: existing?.createdAtMs ?? now,
      updatedAtMs: now,
    };

    // Update in-memory state
    set((state) => {
      const exists = state.notes.some((n) => n.id === noteRecord.id);
      const nextNotes = exists
        ? state.notes.map((n) => (n.id === noteRecord.id ? noteRecord : n))
        : [noteRecord, ...state.notes];
      return {
        notes: nextNotes,
        selectedNoteId: noteRecord.id,
      };
    });

    // Sync with AsyncStorage
    const allNotes = get().notes;
    await AsyncStorage.setItem(NOTES_STORAGE_KEY, JSON.stringify(allNotes)).catch(() => {});

    // Sync with Tauri native bridge if present
    try {
      await invokeTauri<NoteRecord>("save_note", { note: noteRecord });
    } catch {
      // Graceful fallback
    }

    return noteRecord;
  },

  deleteNote: async (id: string) => {
    set((state) => {
      const nextNotes = state.notes.filter((n) => n.id !== id);
      const nextSelected =
        state.selectedNoteId === id ? (nextNotes[0]?.id ?? null) : state.selectedNoteId;
      return {
        notes: nextNotes,
        selectedNoteId: nextSelected,
      };
    });

    const allNotes = get().notes;
    await AsyncStorage.setItem(NOTES_STORAGE_KEY, JSON.stringify(allNotes)).catch(() => {});

    try {
      await invokeTauri<boolean>("delete_note", { id });
    } catch {
      // Graceful fallback
    }

    return true;
  },

  togglePin: async (id: string) => {
    const note = get().notes.find((n) => n.id === id);
    if (!note) return;
    await get().saveNote({ id, pinned: !note.pinned });
  },

  formatNoteAsContext: (note: NoteRecord) => {
    return `[Attached Context: Note - ${note.title}]\n\`\`\`markdown\n${note.content.trim()}\n\`\`\`\n`;
  },
}));

export function useNotes() {
  return useNotesStore();
}
