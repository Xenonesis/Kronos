import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  computeFileDiff,
  computeJsDiff,
  computeAcceleratedLineDiff,
  fileDiffResultToDiffLines,
  fileDiffResultToUnifiedDiff,
  isNativeDiffAvailable,
  type DiffLine,
  type FileDiffResult,
  type NativeDiffBridge,
  type NativeDiffLine,
} from "./diff";

interface GlobalWithWindow {
  window?: {
    paseoDesktop?: {
      nativeDiff?: NativeDiffBridge;
    };
  };
}

function setMockNativeDiff(nativeDiff?: NativeDiffBridge): void {
  const env = globalThis as unknown as GlobalWithWindow;
  env.window = {
    paseoDesktop: {
      nativeDiff,
    },
  };
}

function hasDiffLineType(lines: NativeDiffLine[], type: string): boolean {
  for (const line of lines) {
    if (line.lineType === type) {
      return true;
    }
  }
  return false;
}

function hasUiLine(lines: DiffLine[], type: string, content?: string): boolean {
  for (const line of lines) {
    if (line.type === type && (content === undefined || line.content === content)) {
      return true;
    }
  }
  return false;
}

describe("diff utility", () => {
  const originalWindow = globalThis.window;

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    globalThis.window = originalWindow;
  });

  it("returns zero hunks and zero additions/deletions for identical text", () => {
    const result = computeJsDiff("hello\nworld", "hello\nworld");
    expect(result.hunks).toHaveLength(0);
    expect(result.additions).toBe(0);
    expect(result.deletions).toBe(0);
    expect(result.elapsedMicroseconds).toBeGreaterThanOrEqual(1);
  });

  it("computes additions, deletions, and hunks for modified text", () => {
    const oldText = "line 1\nline 2\nline 3";
    const newText = "line 1\nline 2 modified\nline 3\nline 4";

    const result = computeJsDiff(oldText, newText);
    expect(result.additions).toBe(2);
    expect(result.deletions).toBe(1);
    expect(result.hunks.length).toBeGreaterThanOrEqual(1);

    const firstHunk = result.hunks[0];
    expect(firstHunk.header).toMatch(/^@@ -\d+,\d+ \+\d+,\d+ @@$/);
    expect(hasDiffLineType(firstHunk.lines, "deletion")).toBe(true);
    expect(hasDiffLineType(firstHunk.lines, "addition")).toBe(true);
    expect(hasDiffLineType(firstHunk.lines, "context")).toBe(true);
  });

  it("formats hunk lines to UI diff lines and unified diff text", () => {
    const oldText = "alpha\nbeta\ngamma";
    const newText = "alpha\nbeta-changed\ngamma";
    const diff = computeJsDiff(oldText, newText);

    const uiLines = fileDiffResultToDiffLines(diff);
    expect(hasUiLine(uiLines, "header")).toBe(true);
    expect(hasUiLine(uiLines, "remove", "-beta")).toBe(true);
    expect(hasUiLine(uiLines, "add", "+beta-changed")).toBe(true);

    const unified = fileDiffResultToUnifiedDiff(diff);
    expect(unified).toContain("-beta");
    expect(unified).toContain("+beta-changed");
  });

  it("calls window.paseoDesktop.nativeDiff.compute when available", async () => {
    const mockResult: FileDiffResult = {
      hunks: [
        {
          oldStart: 1,
          oldLines: 3,
          newStart: 1,
          newLines: 3,
          header: "@@ -1,3 +1,3 @@",
          lines: [
            { lineType: "context", content: "foo", oldLineNumber: 1, newLineNumber: 1 },
            { lineType: "deletion", content: "bar", oldLineNumber: 2, newLineNumber: null },
            { lineType: "addition", content: "baz", oldLineNumber: null, newLineNumber: 2 },
          ],
        },
      ],
      additions: 1,
      deletions: 1,
      elapsedMicroseconds: 42,
    };

    const mockNativeDiff: NativeDiffBridge = {
      compute: vi.fn().mockResolvedValue(mockResult),
    };

    setMockNativeDiff(mockNativeDiff);

    expect(isNativeDiffAvailable()).toBe(true);
    const result = await computeFileDiff("foo\nbar", "foo\nbaz", 3);

    expect(mockNativeDiff.compute).toHaveBeenCalledWith("foo\nbar", "foo\nbaz", 3);
    expect(result).toEqual(mockResult);
  });

  it("falls back to computeJsDiff when nativeDiff returns null or throws", async () => {
    const mockNativeDiff: NativeDiffBridge = {
      compute: vi.fn().mockRejectedValue(new Error("native crash")),
    };

    setMockNativeDiff(mockNativeDiff);

    const result = await computeFileDiff("hello", "world");
    expect(result).not.toBeNull();
    expect(result?.additions).toBe(1);
    expect(result?.deletions).toBe(1);
  });

  it("accelerates computeAcceleratedLineDiff using nativeDiff when available", async () => {
    const mockResult: FileDiffResult = {
      hunks: [
        {
          oldStart: 1,
          oldLines: 1,
          newStart: 1,
          newLines: 1,
          header: "@@ -1,1 +1,1 @@",
          lines: [
            { lineType: "deletion", content: "old", oldLineNumber: 1, newLineNumber: null },
            { lineType: "addition", content: "new", oldLineNumber: null, newLineNumber: 1 },
          ],
        },
      ],
      additions: 1,
      deletions: 1,
      elapsedMicroseconds: 50,
    };

    const mockNativeDiff: NativeDiffBridge = {
      compute: vi.fn().mockResolvedValue(mockResult),
    };

    setMockNativeDiff(mockNativeDiff);

    const lines = await computeAcceleratedLineDiff("old", "new");
    expect(mockNativeDiff.compute).toHaveBeenCalled();
    expect(hasUiLine(lines, "header")).toBe(true);
    expect(hasUiLine(lines, "remove", "-old")).toBe(true);
    expect(hasUiLine(lines, "add", "+new")).toBe(true);
  });
});
