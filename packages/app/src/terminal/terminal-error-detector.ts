/**
 * FEAT-21: Terminal Error Auto-Fix Listener
 * Strips ANSI escape codes from PTY output chunks and detects compiler errors,
 * panics, tracebacks, and non-zero exit signatures so users can click
 * "Debug with Kronos" immediately.
 */

const ANSI_PREFIX_CLASS = `[${String.fromCharCode(0x1b)}${String.fromCharCode(0x9b)}]`;
const ANSI_ESCAPE_RE = new RegExp(
  `${ANSI_PREFIX_CLASS}[[()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-ORZcf-nqry=><~]`,
  "g",
);

const ERROR_SIGNATURES: readonly RegExp[] = [
  /error\s+TS\d+:/i,
  /error\[E\d+\]:/i,
  /^\s*Traceback \(most recent call last\):/m,
  /\b(?:TypeError|ReferenceError|SyntaxError|RangeError|AssertionError):/,
  /\bpanicked at\b/,
  /\bnpm ERR!\b/,
  /\bexited with code [1-9]\d*/i,
  /\bexit status [1-9]\d*/i,
  /\bFAIL\s+\S+/,
  /\bcargo test\b.*FAILED/,
  /\berror: could not compile\b/i,
];

export function stripAnsi(input: string): string {
  return input.replace(ANSI_ESCAPE_RE, "").replace(/\r\n/g, "\n");
}

export function detectTerminalFailure(rawChunk: string): string | null {
  const clean = stripAnsi(rawChunk).trim();
  if (!clean) {
    return null;
  }

  for (const pattern of ERROR_SIGNATURES) {
    if (pattern.test(clean)) {
      const lines = clean.split("\n").filter((l) => l.trim().length > 0);
      // Keep the most diagnostic trailing 20 lines capped at 1500 chars
      return lines.slice(-20).join("\n").slice(-1500);
    }
  }

  return null;
}

export class TerminalOutputErrorBuffer {
  private buffer = "";
  private readonly maxChars: number;

  constructor(maxChars = 4000) {
    this.maxChars = maxChars;
  }

  pushBytes(bytes: Uint8Array): string | null {
    const decoded = new TextDecoder("utf-8", { fatal: false }).decode(bytes);
    return this.pushText(decoded);
  }

  pushText(chunk: string): string | null {
    const clean = stripAnsi(chunk);
    if (!clean) {
      return null;
    }
    this.buffer = (this.buffer + clean).slice(-this.maxChars);
    return detectTerminalFailure(this.buffer);
  }

  clear(): void {
    this.buffer = "";
  }
}
