import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  formatAudioCompletionMessage,
  speakTaskCompletion,
  setSpeechAnnouncerEnabled,
} from "./task-speech-announcer";

describe("task-speech-announcer", () => {
  beforeEach(() => {
    setSpeechAnnouncerEnabled(true);
  });

  it("formats successful audio message with test status", () => {
    const msg = formatAudioCompletionMessage({
      filesChangedCount: 3,
      testsPassed: true,
    });
    expect(msg).toBe("Kronos finished: 3 files updated. All tests passed.");
  });

  it("formats error audio message accurately", () => {
    const msg = formatAudioCompletionMessage({
      hasErrors: true,
    });
    expect(msg).toContain("Kronos encountered errors");
  });

  it("handles speech synthesis call when window.speechSynthesis is mocked", () => {
    const speakMock = vi.fn();
    const cancelMock = vi.fn();
    (global as any).window = {
      speechSynthesis: {
        speak: speakMock,
        cancel: cancelMock,
      },
    };
    (global as any).SpeechSynthesisUtterance = class {
      text: string;
      constructor(text: string) {
        this.text = text;
      }
    };

    const spoken = speakTaskCompletion("Task complete");
    expect(spoken).toBe(true);
    expect(cancelMock).toHaveBeenCalled();
    expect(speakMock).toHaveBeenCalled();
  });
});
