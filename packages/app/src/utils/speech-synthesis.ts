import { create } from "zustand";

interface SpeechState {
  speakingMessageId: string | null;
  setSpeakingMessageId: (id: string | null) => void;
}

export const useSpeechStore = create<SpeechState>((set) => ({
  speakingMessageId: null,
  setSpeakingMessageId: (id) => set({ speakingMessageId: id }),
}));

/**
 * Strips markdown markup, code fences, and links to make speech sound natural.
 */
export function stripMarkdownForSpeech(markdown: string): string {
  return (
    markdown
      // Code fences -> brief spoken indicator
      .replace(/```[\s\S]*?```/g, "Code block omitted.")
      // Inline code
      .replace(/`([^`]+)`/g, "$1")
      // Images
      .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
      // Links: keep anchor text
      .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
      // Headings
      .replace(/^#{1,6}\s+/gm, "")
      // Blockquotes
      .replace(/^\s*>\s+/gm, "")
      // Bold / italic
      .replace(/(\*\*|__)(.*?)\1/g, "$2")
      .replace(/(\*|_)(.*?)\1/g, "$2")
      // Strikethrough
      .replace(/~~(.*?)~~/g, "$1")
      // Bullet / numbered lists
      .replace(/^\s*[-*+]\s+/gm, "")
      .replace(/^\s*\d+\.\s+/gm, "")
      // Collapse excess whitespace
      .replace(/\n{2,}/g, ". ")
      .replace(/\s+/g, " ")
      .trim()
  );
}

export function isSpeechSupported(): boolean {
  return typeof window !== "undefined" && "speechSynthesis" in window;
}

export function stopSpeaking(): void {
  if (isSpeechSupported()) {
    try {
      window.speechSynthesis.cancel();
    } catch {
      // Ignore synthesis cancel errors
    }
  }
  useSpeechStore.getState().setSpeakingMessageId(null);
}

export function speakMessage(messageId: string, text: string): void {
  if (!isSpeechSupported()) {
    return;
  }

  const store = useSpeechStore.getState();
  if (store.speakingMessageId === messageId) {
    stopSpeaking();
    return;
  }

  stopSpeaking();

  const cleanText = stripMarkdownForSpeech(text);
  if (!cleanText) {
    return;
  }

  const utterance = new SpeechSynthesisUtterance(cleanText);
  utterance.onend = () => {
    store.setSpeakingMessageId(null);
  };
  utterance.onerror = () => {
    store.setSpeakingMessageId(null);
  };

  store.setSpeakingMessageId(messageId);
  window.speechSynthesis.speak(utterance);
}
