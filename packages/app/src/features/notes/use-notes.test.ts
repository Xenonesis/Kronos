import { describe, it, expect, beforeEach, vi } from "vitest";
import { useNotesStore } from "./use-notes";
import type { NoteRecord } from "./types";

describe("useNotesStore", () => {
  beforeEach(() => {
    useNotesStore.setState({
      notes: [],
      selectedNoteId: null,
      isDrawerOpen: false,
      isLoading: false,
      searchQuery: "",
      selectedTag: null,
      pinnedOnly: false,
    });
  });

  it("initializes with empty or loaded state", () => {
    const state = useNotesStore.getState();
    expect(state.notes).toEqual([]);
    expect(state.isDrawerOpen).toBe(false);
  });

  it("opens, closes, and toggles the drawer", () => {
    const store = useNotesStore.getState();
    store.openDrawer("test-id");
    expect(useNotesStore.getState().isDrawerOpen).toBe(true);
    expect(useNotesStore.getState().selectedNoteId).toBe("test-id");

    store.closeDrawer();
    expect(useNotesStore.getState().isDrawerOpen).toBe(false);

    store.toggleDrawer();
    expect(useNotesStore.getState().isDrawerOpen).toBe(true);
  });

  it("saves a new note and selects it", async () => {
    const store = useNotesStore.getState();
    const created = await store.saveNote({
      title: "Architecture Doc",
      content: "# System Design\nClean architecture.",
      tags: ["architecture", "docs"],
      pinned: true,
    });

    expect(created.id).toBeDefined();
    expect(created.title).toBe("Architecture Doc");
    expect(created.tags).toEqual(["architecture", "docs"]);
    expect(created.pinned).toBe(true);

    const state = useNotesStore.getState();
    expect(state.notes).toHaveLength(1);
    expect(state.selectedNoteId).toBe(created.id);
  });

  it("updates an existing note preserving created timestamp", async () => {
    const store = useNotesStore.getState();
    const created = await store.saveNote({
      title: "Note 1",
      content: "Initial content",
    });

    const updated = await store.saveNote({
      id: created.id,
      title: "Note 1 - Updated",
      content: "Revised content",
    });

    expect(updated.id).toBe(created.id);
    expect(updated.title).toBe("Note 1 - Updated");
    expect(updated.content).toBe("Revised content");
    expect(updated.createdAtMs).toBe(created.createdAtMs);
    expect(updated.updatedAtMs).toBeGreaterThanOrEqual(created.updatedAtMs);

    const state = useNotesStore.getState();
    expect(state.notes).toHaveLength(1);
  });

  it("deletes a note and re-assigns selection", async () => {
    const store = useNotesStore.getState();
    const note1 = await store.saveNote({ title: "First Note" });
    const note2 = await store.saveNote({ title: "Second Note" });

    store.selectNote(note2.id);
    expect(useNotesStore.getState().selectedNoteId).toBe(note2.id);

    await store.deleteNote(note2.id);

    const state = useNotesStore.getState();
    expect(state.notes).toHaveLength(1);
    expect(state.notes[0]?.id).toBe(note1.id);
    expect(state.selectedNoteId).toBe(note1.id);
  });

  it("toggles note pinned status", async () => {
    const store = useNotesStore.getState();
    const note = await store.saveNote({ title: "Pin Me", pinned: false });
    expect(note.pinned).toBe(false);

    await store.togglePin(note.id);
    expect(useNotesStore.getState().notes.find((n) => n.id === note.id)?.pinned).toBe(true);

    await store.togglePin(note.id);
    expect(useNotesStore.getState().notes.find((n) => n.id === note.id)?.pinned).toBe(false);
  });

  it("formats note as context block for LLM prompts", () => {
    const store = useNotesStore.getState();
    const note: NoteRecord = {
      id: "n-1",
      title: "Backend Auth API",
      content: "POST /auth/login returns JWT bearer token",
      tags: ["api"],
      pinned: false,
      createdAtMs: 1000,
      updatedAtMs: 2000,
    };

    const formatted = store.formatNoteAsContext(note);
    expect(formatted).toContain("[Attached Context: Note - Backend Auth API]");
    expect(formatted).toContain("POST /auth/login returns JWT bearer token");
    expect(formatted).toContain("```markdown");
  });
});
