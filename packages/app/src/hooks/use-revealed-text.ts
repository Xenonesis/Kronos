import { useEffect, useRef, useState } from "react";

import {
  advanceTextReveal,
  beginTextReveal,
  completeTextReveal,
  isTextRevealPacingSupported,
  isTextRevealSettled,
  nextTextRevealFrame,
  retargetTextReveal,
  type TextRevealState,
  visibleRevealedText,
} from "@/agent-stream/text-reveal";
export type MarkdownPhase = "streaming" | "complete";

/**
 * Binds the paced reveal in @/agent-stream/text-reveal to a frame clock.
 *
 * All of the policy — what is revealed when, and where it is safe to cut — lives
 * in that module and is tested there. This hook only owns the requestAnimationFrame
 * wiring, so the rendered behavior is covered end to end by
 * `packages/app/e2e/browser/agent-stream-smoothness.spec.ts`.
 */
export function useRevealedText(text: string, phase: MarkdownPhase): string {
  const pacingSupported = isTextRevealPacingSupported();
  const stateRef = useRef<TextRevealState>(
    beginTextReveal(text, phase === "streaming" ? 0 : text.length),
  );
  const [, forceRender] = useState(0);
  const frameRef = useRef<number | null>(null);
  const lastFrameAtRef = useRef<number | null>(null);

  stateRef.current = retargetTextReveal(stateRef.current, text);

  useEffect(() => {
    const settle = () => {
      if (frameRef.current !== null) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
      lastFrameAtRef.current = null;
      const next = completeTextReveal(stateRef.current);
      if (next !== stateRef.current) {
        stateRef.current = next;
        forceRender((tick) => tick + 1);
      }
    };

    if (!pacingSupported || phase !== "streaming") {
      settle();
      return;
    }
    if (isTextRevealSettled(stateRef.current)) {
      lastFrameAtRef.current = null;
      return;
    }
    if (typeof requestAnimationFrame !== "function") {
      settle();
      return;
    }

    if (frameRef.current === null) {
      const tick = (timestamp: number) => {
        frameRef.current = null;
        const frame = nextTextRevealFrame(lastFrameAtRef.current, timestamp);
        if (!frame) {
          frameRef.current = requestAnimationFrame(tick);
          return;
        }
        lastFrameAtRef.current = frame.frameAtMs;

        const next = advanceTextReveal(stateRef.current, frame.elapsedMs);
        if (next !== stateRef.current) {
          stateRef.current = next;
          forceRender((count) => count + 1);
        }
        if (!isTextRevealSettled(stateRef.current)) {
          frameRef.current = requestAnimationFrame(tick);
        } else {
          lastFrameAtRef.current = null;
        }
      };

      frameRef.current = requestAnimationFrame(tick);
    }
  }, [text, pacingSupported, phase]);

  useEffect(() => {
    return () => {
      if (frameRef.current !== null) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
    };
  }, []);

  return pacingSupported
    ? visibleRevealedText(stateRef.current, { streaming: phase === "streaming" })
    : text;
}
