/**
 * Kronos Event-Driven Audio Notification Suite
 *
 * Provides crisp, zero-dependency audio notifications via the Web Audio API:
 * - Task Completed (melodic ascending arpeggio)
 * - Attention Required / User Input Needed (pleasant two-tone bell)
 * - Error / Test Failure (gentle low alert)
 */

export type AudioNotificationKind = "task_success" | "attention_required" | "error_alert";

export interface AudioSuiteOptions {
  muted?: boolean;
  volume?: number; // 0.0 to 1.0
}

let audioContextInstance: AudioContext | null = null;

function getAudioContext(): AudioContext | null {
  if (typeof window === "undefined") {
    return null;
  }
  const AudioCtx =
    window.AudioContext ||
    (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!AudioCtx) {
    return null;
  }
  if (!audioContextInstance || audioContextInstance.state === "closed") {
    try {
      audioContextInstance = new AudioCtx();
    } catch {
      return null;
    }
  }
  if (audioContextInstance.state === "suspended") {
    void audioContextInstance.resume().catch(() => {});
  }
  return audioContextInstance;
}

/**
 * Plays a sequence of pure sine tones with exponential decay envelope.
 */
function playToneSequence(frequencies: number[], toneDuration: number, volume = 0.25): void {
  const ctx = getAudioContext();
  if (!ctx) return;

  const now = ctx.currentTime;
  const masterGain = ctx.createGain();
  masterGain.gain.setValueAtTime(Math.max(0, Math.min(1, volume)), now);
  masterGain.connect(ctx.destination);

  frequencies.forEach((freq, idx) => {
    const startTime = now + idx * toneDuration;
    const osc = ctx.createOscillator();
    const noteGain = ctx.createGain();

    osc.type = "sine";
    osc.frequency.setValueAtTime(freq, startTime);

    noteGain.gain.setValueAtTime(0.001, startTime);
    noteGain.gain.exponentialRampToValueAtTime(0.3, startTime + 0.02);
    noteGain.gain.exponentialRampToValueAtTime(0.0001, startTime + toneDuration);

    osc.connect(noteGain);
    noteGain.connect(masterGain);

    osc.start(startTime);
    osc.stop(startTime + toneDuration);
  });
}

/**
 * Play task success chime (C5 -> E5 -> G5)
 */
export function playTaskSuccessSound(options: AudioSuiteOptions = {}): void {
  if (options.muted) return;
  const vol = options.volume ?? 0.25;
  playToneSequence([523.25, 659.25, 783.99], 0.12, vol);
}

/**
 * Play attention required / waiting for user approval bell (A5 -> D6)
 */
export function playAttentionSound(options: AudioSuiteOptions = {}): void {
  if (options.muted) return;
  const vol = options.volume ?? 0.25;
  playToneSequence([880.0, 1174.66], 0.16, vol);
}

/**
 * Play error / test failure alert (Eb4 -> C4)
 */
export function playErrorSound(options: AudioSuiteOptions = {}): void {
  if (options.muted) return;
  const vol = options.volume ?? 0.25;
  playToneSequence([311.13, 261.63], 0.15, vol);
}

/**
 * Dispatches an audio notification by event kind.
 */
export function playNotificationSound(
  kind: AudioNotificationKind,
  options: AudioSuiteOptions = {},
): void {
  switch (kind) {
    case "task_success":
      playTaskSuccessSound(options);
      break;
    case "attention_required":
      playAttentionSound(options);
      break;
    case "error_alert":
      playErrorSound(options);
      break;
  }
}
