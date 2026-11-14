export interface KronosComposerHandlerOptions {
  getValue: () => string;
  replaceText: (text: string) => void;
  handleSendMessage: () => void;
  debounceWindowMs?: number;
  setTimeoutFn?: (callback: () => void, ms: number) => any;
  clearTimeoutFn?: (id: any) => void;
  target?: EventTarget;
}

export function registerKronosComposerListeners(options: KronosComposerHandlerOptions): () => void {
  const {
    getValue,
    replaceText,
    handleSendMessage,
    debounceWindowMs = 1000,
    setTimeoutFn = setTimeout,
    clearTimeoutFn = clearTimeout,
    target = typeof window !== "undefined" ? window : undefined,
  } = options;

  if (!target) return () => {};

  let lastEvent: { text: string; timestamp: number; type: "insert" | "dispatch" } | null = null;
  let dispatchTimer: any = null;

  const handleInsert = (e: Event) => {
    const customEvent = e as CustomEvent<{ text?: string }>;
    const text = customEvent.detail?.text;
    if (!text) return;

    const now = Date.now();
    // Prevent double-text concatenation if kronos:dispatch-composer or insert was recently received for the exact same text
    if (
      lastEvent &&
      lastEvent.text === text &&
      now - lastEvent.timestamp < debounceWindowMs
    ) {
      return;
    }

    lastEvent = {
      text,
      timestamp: now,
      type: "insert",
    };

    const current = getValue();
    const next = current ? `${current}\n\n${text}` : text;
    replaceText(next);
  };

  const handleDispatch = (e: Event) => {
    const customEvent = e as CustomEvent<{ text?: string; agent?: string }>;
    const text = customEvent.detail?.text;
    if (!text) return;

    const now = Date.now();
    const wasRecentlyHandled =
      lastEvent &&
      lastEvent.text === text &&
      now - lastEvent.timestamp < debounceWindowMs;

    lastEvent = {
      text,
      timestamp: now,
      type: "dispatch",
    };

    // Update composer text with detail.text
    if (!wasRecentlyHandled || getValue() !== text) {
      replaceText(text);
    }

    // Trigger message dispatch after a short tick (50ms), ensuring the prompt is actually sent
    if (dispatchTimer) {
      clearTimeoutFn(dispatchTimer);
    }
    dispatchTimer = setTimeoutFn(() => {
      handleSendMessage();
    }, 50);
  };

  target.addEventListener("kronos:insert-composer", handleInsert);
  target.addEventListener("kronos:dispatch-composer", handleDispatch);

  return () => {
    target.removeEventListener("kronos:insert-composer", handleInsert);
    target.removeEventListener("kronos:dispatch-composer", handleDispatch);
    if (dispatchTimer) {
      clearTimeoutFn(dispatchTimer);
    }
  };
}
