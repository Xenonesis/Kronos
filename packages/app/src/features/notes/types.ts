export interface NoteRecord {
  id: string;
  title: string;
  content: string;
  tags: string[];
  pinned: boolean;
  createdAtMs: number;
  updatedAtMs: number;
}

export type CreateNoteInput = Partial<Omit<NoteRecord, "id" | "createdAtMs" | "updatedAtMs">> & {
  title?: string;
  content?: string;
};

export type UpdateNoteInput = Partial<Omit<NoteRecord, "id" | "createdAtMs">> & {
  updatedAtMs?: number;
};
