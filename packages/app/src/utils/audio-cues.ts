/**
 * Kronos Ambient Audio Cues
 * Lightweight, zero-dependency Web Audio synthesizer for tactile acoustic feedback.
 */

export type AudioCueKind = "agent_start" | "agent_complete" | "agent_error" | "mic_toggle";

let audioCtx: AudioContext | null = null;

function getAudioContext(): AudioContext | null {
  if (typeof window === "undefined") return null;
  const AudioContextClass =
    window.AudioContext ||
    (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!AudioContextClass) return null;

  if (!audioCtx) {
    try {
      audioCtx = new AudioContextClass();
    } catch {
      return null;
    }
  }
  if (audioCtx.state === "suspended") {
    audioCtx.resume().catch(() => {});
  }
  return audioCtx;
}

export function playAudioCue(kind: AudioCueKind): void {
  try {
    const ctx = getAudioContext();
    if (!ctx) return;

    const now = ctx.currentTime;

    switch (kind) {
      case "agent_start": {
        // Ascending 2-tone chime: C5 (523Hz) -> E5 (659Hz)
        playTone(ctx, 523.25, now, 0.09, 0.05);
        playTone(ctx, 659.25, now + 0.08, 0.12, 0.06);
        break;
      }
      case "agent_complete": {
        // Triad chime: C5 (523Hz) -> E5 (659Hz) -> G5 (784Hz)
        playTone(ctx, 523.25, now, 0.08, 0.05);
        playTone(ctx, 659.25, now + 0.07, 0.08, 0.06);
        playTone(ctx, 783.99, now + 0.14, 0.18, 0.07);
        break;
      }
      case "agent_error": {
        // Descending warning: A4 (440Hz) -> E4 (330Hz)
        playTone(ctx, 440.0, now, 0.12, 0.07, "triangle");
        playTone(ctx, 329.63, now + 0.11, 0.2, 0.08, "triangle");
        break;
      }
      case "mic_toggle": {
        // Subtle transient tactile pop
        playTone(ctx, 880.0, now, 0.03, 0.04);
        break;
      }
    }
  } catch {
    // Non-fatal if audio cue fails
  }
}

function playTone(
  ctx: AudioContext,
  freq: number,
  startTime: number,
  duration: number,
  peakGain = 0.06,
  type: OscillatorType = "sine",
): void {
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.type = type;
  osc.frequency.setValueAtTime(freq, startTime);

  gain.gain.setValueAtTime(0.0001, startTime);
  gain.gain.exponentialRampToValueAtTime(peakGain, startTime + 0.015);
  gain.gain.exponentialRampToValueAtTime(0.0001, startTime + duration);

  osc.connect(gain);
  gain.connect(ctx.destination);

  osc.start(startTime);
  osc.stop(startTime + duration + 0.02);
}
