import { describe, expect, it, vi } from "vitest";
import {
  playAttentionSound,
  playErrorSound,
  playNotificationSound,
  playTaskSuccessSound,
} from "./audio-suite";

describe("audio-suite", () => {
  it("runs without throwing when window or AudioContext is undefined", () => {
    expect(() => playTaskSuccessSound()).not.toThrow();
    expect(() => playAttentionSound()).not.toThrow();
    expect(() => playErrorSound()).not.toThrow();
    expect(() => playNotificationSound("task_success")).not.toThrow();
    expect(() => playNotificationSound("attention_required")).not.toThrow();
    expect(() => playNotificationSound("error_alert")).not.toThrow();
  });

  it("respects muted option", () => {
    const playSpy = vi.fn();
    expect(() => playTaskSuccessSound({ muted: true })).not.toThrow();
    expect(() => playAttentionSound({ muted: true })).not.toThrow();
    expect(() => playErrorSound({ muted: true })).not.toThrow();
    expect(playSpy).not.toHaveBeenCalled();
  });
});
