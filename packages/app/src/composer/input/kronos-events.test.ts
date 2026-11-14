import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { registerKronosComposerListeners } from "./kronos-events";

describe("registerKronosComposerListeners", () => {
  let target: EventTarget;
  let composerText: string;
  let replaceTextSpy: ReturnType<typeof vi.fn<(next: string) => void>>;
  let sendMessageSpy: ReturnType<typeof vi.fn<() => void>>;
  let unregister: (() => void) | null = null;

  beforeEach(() => {
    vi.useFakeTimers();
    target = new EventTarget();
    composerText = "";
    replaceTextSpy = vi.fn<(next: string) => void>((next: string) => {
      composerText = next;
    });
    sendMessageSpy = vi.fn<() => void>();

    unregister = registerKronosComposerListeners({
      target,
      getValue: () => composerText,
      replaceText: replaceTextSpy,
      handleSendMessage: sendMessageSpy,
      debounceWindowMs: 1000,
    });
  });

  afterEach(() => {
    if (unregister) {
      unregister();
      unregister = null;
    }
    vi.useRealTimers();
  });

  it("inserts text into composer on kronos:insert-composer", () => {
    target.dispatchEvent(
      new CustomEvent("kronos:insert-composer", {
        detail: { text: "First instruction" },
      }),
    );

    expect(replaceTextSpy).toHaveBeenCalledWith("First instruction");
    expect(composerText).toBe("First instruction");
    expect(sendMessageSpy).not.toHaveBeenCalled();

    // Secondary insert appends with newline
    target.dispatchEvent(
      new CustomEvent("kronos:insert-composer", {
        detail: { text: "Second instruction" },
      }),
    );

    expect(replaceTextSpy).toHaveBeenCalledWith("First instruction\n\nSecond instruction");
  });

  it("updates text and triggers handleSendMessage after 50ms tick on kronos:dispatch-composer", () => {
    target.dispatchEvent(
      new CustomEvent("kronos:dispatch-composer", {
        detail: { text: "Run lint and fix errors", agent: "codex" },
      }),
    );

    expect(replaceTextSpy).toHaveBeenCalledWith("Run lint and fix errors");
    expect(sendMessageSpy).not.toHaveBeenCalled();

    // Advance 50ms
    vi.advanceTimersByTime(50);
    expect(sendMessageSpy).toHaveBeenCalledTimes(1);
  });

  it("prevents double-text concatenation when kronos:insert-composer arrives after dispatch", () => {
    const prompt = "Repair broken test suite";

    // 1. Dispatch event arrives first
    target.dispatchEvent(
      new CustomEvent("kronos:dispatch-composer", {
        detail: { text: prompt, agent: "claude" },
      }),
    );

    expect(replaceTextSpy).toHaveBeenCalledWith(prompt);
    expect(composerText).toBe(prompt);

    // 2. Duplicate insert event arrives 10ms later with exact same text
    vi.advanceTimersByTime(10);
    target.dispatchEvent(
      new CustomEvent("kronos:insert-composer", {
        detail: { text: prompt },
      }),
    );

    // replaceText should NOT have been called with prompt\n\nprompt
    expect(replaceTextSpy).toHaveBeenCalledTimes(1);
    expect(composerText).toBe(prompt);

    // 3. Tick 40ms more (50ms total since dispatch)
    vi.advanceTimersByTime(40);
    expect(sendMessageSpy).toHaveBeenCalledTimes(1);
  });

  it("prevents double-text concatenation when kronos:insert-composer arrives before dispatch", () => {
    const prompt = "Repair broken test suite";

    // 1. Insert event arrives first
    target.dispatchEvent(
      new CustomEvent("kronos:insert-composer", {
        detail: { text: prompt },
      }),
    );

    expect(replaceTextSpy).toHaveBeenCalledWith(prompt);
    expect(composerText).toBe(prompt);

    // 2. Dispatch event arrives 10ms later with exact same text
    vi.advanceTimersByTime(10);
    target.dispatchEvent(
      new CustomEvent("kronos:dispatch-composer", {
        detail: { text: prompt, agent: "claude" },
      }),
    );

    // Composer text should still be the clean single prompt
    expect(composerText).toBe(prompt);

    // 3. Tick 50ms
    vi.advanceTimersByTime(50);
    expect(sendMessageSpy).toHaveBeenCalledTimes(1);
  });

  it("unregisters listeners cleanly and cancels pending dispatch timer", () => {
    unregister?.();
    unregister = null;

    target.dispatchEvent(
      new CustomEvent("kronos:dispatch-composer", {
        detail: { text: "Should be ignored" },
      }),
    );

    vi.advanceTimersByTime(100);
    expect(replaceTextSpy).not.toHaveBeenCalled();
    expect(sendMessageSpy).not.toHaveBeenCalled();
  });
});
