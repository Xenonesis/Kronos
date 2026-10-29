/**
 * TypeScript types for Kronos Native Engine Preload Bridge.
 *
 * Strictly typed definitions for window.paseoDesktop.nativeTerminal and
 * window.paseoDesktop.nativeDiff.
 *
 * ts-no-any: Absolutely zero `: any` or `as any` allowed.
 */

export interface TerminalSpawnOptions {
  command?: string;
  args: string[];
  cwd?: string;
  cols: number;
  rows: number;
}

export type TerminalEvent =
  | { type: "data"; data: string }
  | { type: "exit"; data?: number | null }
  | { type: "error"; data: string };

export interface NativeTerminalBridge {
  spawn: (
    id: string,
    options: TerminalSpawnOptions,
    onEvent: (event: TerminalEvent) => void,
  ) => Promise<void>;
  write: (id: string, data: string) => Promise<void>;
  resize: (id: string, cols: number, rows: number) => Promise<void>;
  kill: (id: string) => Promise<void>;
}

export type DiffLineType = "context" | "addition" | "deletion";

export interface DiffLine {
  lineType: DiffLineType;
  oldLineNumber?: number | null;
  newLineNumber?: number | null;
  content: string;
}

export interface DiffHunk {
  oldStart: number;
  oldLines: number;
  newStart: number;
  newLines: number;
  header: string;
  lines: DiffLine[];
}

export interface FileDiffResult {
  hunks: DiffHunk[];
  additions: number;
  deletions: number;
  elapsedMicroseconds: number;
}

export interface NativeDiffBridge {
  compute: (
    oldText: string,
    newText: string,
    contextLines?: number,
  ) => Promise<FileDiffResult | null>;
}

export interface KronosDesktopBridge {
  platform: string;
  windowChromeMode: string;
  nativeTerminal: NativeTerminalBridge;
  nativeDiff: NativeDiffBridge;
}

declare global {
  interface Window {
    paseoDesktop?: KronosDesktopBridge;
  }
}
