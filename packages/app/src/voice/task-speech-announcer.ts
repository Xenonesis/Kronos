/**
 * Kronos Spoken Audio Task Completion Announcer (Text-to-Speech)
 *
 * Provides Claude Code Voice Announcements & Devin Audio parity.
 * Speaks natural audio completions when long-running agent tasks, builds,
 * or test runs finish while the user is outside the application window.
 */

export interface SpeechAnnouncerOptions {
  enabled?: boolean;
  rate?: number; // 0.8 - 1.2
  pitch?: number; // 0.8 - 1.2
  volume?: number; // 0.0 - 1.0
}

let speechEnabled = true;

export function setSpeechAnnouncerEnabled(enabled: boolean): void {
  speechEnabled = enabled;
}

export function isSpeechAnnouncerEnabled(): boolean {
  return speechEnabled;
}

/**
 * Formats a clean, conversational 1-sentence completion summary suitable for audio playback.
 */
export function formatAudioCompletionMessage(input: {
  taskTitle?: string;
  filesChangedCount?: number;
  testsPassed?: boolean;
  hasErrors?: boolean;
}): string {
  if (input.hasErrors) {
    return "Kronos encountered errors during task execution. Please check the terminal.";
  }

  const filesPart = input.filesChangedCount
    ? `${input.filesChangedCount} file${input.filesChangedCount > 1 ? "s" : ""} updated.`
    : "Changes applied.";

  const testPart =
    input.testsPassed !== undefined
      ? input.testsPassed
        ? " All tests passed."
        : " Some tests failed."
      : "";

  return `Kronos finished: ${filesPart}${testPart}`;
}

/**
 * Speaks the text aloud via Web Speech Synthesis API.
 */
export function speakTaskCompletion(text: string, options: SpeechAnnouncerOptions = {}): boolean {
  if (!speechEnabled && options.enabled !== true) {
    return false;
  }

  if (typeof window === "undefined" || !("speechSynthesis" in window)) {
    return false;
  }

  try {
    window.speechSynthesis.cancel(); // Stop any pending utterances
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.rate = options.rate ?? 1.05;
    utterance.pitch = options.pitch ?? 1.0;
    utterance.volume = options.volume ?? 0.9;

    window.speechSynthesis.speak(utterance);
    return true;
  } catch (err) {
    console.warn("[Voice Announcer] Failed to speak completion utterance:", err);
    return false;
  }
}
