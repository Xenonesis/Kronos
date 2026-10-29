import type {
  DiffLineType,
  DiffLine as NativeDiffLine,
  DiffHunk,
  FileDiffResult,
  NativeDiffBridge,
} from "@/desktop/host";
import { buildLineDiff, type DiffLine } from "./tool-call-parsers";

export type { DiffLineType, NativeDiffLine, DiffHunk, FileDiffResult, NativeDiffBridge, DiffLine };

export function isNativeDiffAvailable(): boolean {
  return typeof window !== "undefined" && Boolean(window.paseoDesktop?.nativeDiff?.compute);
}

function splitLines(text: string): string[] {
  if (text.length === 0) {
    return [];
  }
  return text.split(/\r?\n/);
}

interface DiffOp {
  type: DiffLineType;
  content: string;
  oldIndex: number | null;
  newIndex: number | null;
}

function buildLcsTable(oldLines: string[], newLines: string[]): number[][] {
  const m = oldLines.length;
  const n = newLines.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => Array<number>(n + 1).fill(0));
  for (let i = m - 1; i >= 0; i -= 1) {
    for (let j = n - 1; j >= 0; j -= 1) {
      if (oldLines[i] === newLines[j]) {
        dp[i][j] = dp[i + 1][j + 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1]);
      }
    }
  }
  return dp;
}

function buildDiffOps(
  oldLines: string[],
  newLines: string[],
): { ops: DiffOp[]; additions: number; deletions: number } {
  const dp = buildLcsTable(oldLines, newLines);
  const m = oldLines.length;
  const n = newLines.length;
  const ops: DiffOp[] = [];
  let i = 0;
  let j = 0;
  let additions = 0;
  let deletions = 0;

  while (i < m && j < n) {
    if (oldLines[i] === newLines[j]) {
      ops.push({ type: "context", content: oldLines[i], oldIndex: i + 1, newIndex: j + 1 });
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ type: "deletion", content: oldLines[i], oldIndex: i + 1, newIndex: null });
      deletions += 1;
      i += 1;
    } else {
      ops.push({ type: "addition", content: newLines[j], oldIndex: null, newIndex: j + 1 });
      additions += 1;
      j += 1;
    }
  }

  while (i < m) {
    ops.push({ type: "deletion", content: oldLines[i], oldIndex: i + 1, newIndex: null });
    deletions += 1;
    i += 1;
  }

  while (j < n) {
    ops.push({ type: "addition", content: newLines[j], oldIndex: null, newIndex: j + 1 });
    additions += 1;
    j += 1;
  }

  return { ops, additions, deletions };
}

function groupOpsIntoHunks(ops: DiffOp[], contextLines: number): DiffHunk[] {
  const hunks: DiffHunk[] = [];
  let hunkOps: DiffOp[] = [];
  let lastChangeIndex = -1;

  for (let idx = 0; idx < ops.length; idx += 1) {
    const op = ops[idx];
    const isChange = op.type !== "context";

    if (isChange) {
      if (hunkOps.length === 0) {
        const leadStart = Math.max(0, idx - contextLines);
        for (let k = leadStart; k < idx; k += 1) {
          hunkOps.push(ops[k]);
        }
      }
      hunkOps.push(op);
      lastChangeIndex = idx;
    } else if (hunkOps.length > 0) {
      if (idx - lastChangeIndex <= contextLines) {
        hunkOps.push(op);
      } else {
        hunks.push(buildHunkFromOps(hunkOps));
        hunkOps = [];
      }
    }
  }

  if (hunkOps.length > 0) {
    hunks.push(buildHunkFromOps(hunkOps));
  }

  return hunks;
}

export function computeJsDiff(
  oldText: string,
  newText: string,
  contextLines: number = 3,
): FileDiffResult {
  const startTime = globalThis.performance?.now?.() ?? Date.now();
  const oldLines = splitLines(oldText);
  const newLines = splitLines(newText);

  const { ops, additions, deletions } = buildDiffOps(oldLines, newLines);
  const elapsed = Math.round(((globalThis.performance?.now?.() ?? Date.now()) - startTime) * 1000);

  if (additions === 0 && deletions === 0) {
    return {
      hunks: [],
      additions: 0,
      deletions: 0,
      elapsedMicroseconds: Math.max(1, elapsed),
    };
  }

  const hunks = groupOpsIntoHunks(ops, contextLines);
  return {
    hunks,
    additions,
    deletions,
    elapsedMicroseconds: Math.max(1, elapsed),
  };
}

function getDiffLinePrefix(lineType: DiffLineType): string {
  if (lineType === "addition") {
    return "+";
  }
  if (lineType === "deletion") {
    return "-";
  }
  return " ";
}

function buildHunkFromOps(ops: DiffOp[]): DiffHunk {
  let oldStart = 0;
  let oldLines = 0;
  let newStart = 0;
  let newLines = 0;

  for (const op of ops) {
    if (op.type === "context") {
      if (oldStart === 0 && op.oldIndex !== null) oldStart = op.oldIndex;
      if (newStart === 0 && op.newIndex !== null) newStart = op.newIndex;
      oldLines += 1;
      newLines += 1;
    } else if (op.type === "deletion") {
      if (oldStart === 0 && op.oldIndex !== null) oldStart = op.oldIndex;
      oldLines += 1;
    } else if (op.type === "addition") {
      if (newStart === 0 && op.newIndex !== null) newStart = op.newIndex;
      newLines += 1;
    }
  }

  if (oldStart === 0) oldStart = 1;
  if (newStart === 0) newStart = 1;

  const header = `@@ -${oldStart},${oldLines} +${newStart},${newLines} @@`;
  const lines: NativeDiffLine[] = ops.map((op) => ({
    lineType: op.type,
    oldLineNumber: op.oldIndex,
    newLineNumber: op.newIndex,
    content: op.content,
  }));

  return {
    oldStart,
    oldLines,
    newStart,
    newLines,
    header,
    lines,
  };
}

export async function computeFileDiff(
  oldText: string,
  newText: string,
  contextLines: number = 3,
): Promise<FileDiffResult | null> {
  if (typeof window !== "undefined" && window.paseoDesktop?.nativeDiff?.compute) {
    try {
      const nativeResult = await window.paseoDesktop.nativeDiff.compute(
        oldText,
        newText,
        contextLines,
      );
      if (nativeResult) {
        return nativeResult;
      }
    } catch {
      // Fall back to JS implementation if native diff throws
    }
  }

  return computeJsDiff(oldText, newText, contextLines);
}

export function fileDiffResultToDiffLines(diff: FileDiffResult): DiffLine[] {
  const result: DiffLine[] = [];
  for (const hunk of diff.hunks) {
    if (hunk.header) {
      result.push({ type: "header", content: hunk.header });
    }
    for (const line of hunk.lines) {
      if (line.lineType === "addition") {
        result.push({ type: "add", content: `+${line.content}` });
      } else if (line.lineType === "deletion") {
        result.push({ type: "remove", content: `-${line.content}` });
      } else {
        result.push({ type: "context", content: ` ${line.content}` });
      }
    }
  }
  return result;
}

export function fileDiffResultToUnifiedDiff(diff: FileDiffResult): string {
  const parts: string[] = [];
  for (const hunk of diff.hunks) {
    parts.push(hunk.header);
    for (const line of hunk.lines) {
      const prefix = getDiffLinePrefix(line.lineType);
      parts.push(`${prefix}${line.content}`);
    }
  }
  return parts.join("\n");
}

export async function computeAcceleratedLineDiff(
  oldText: string,
  newText: string,
): Promise<DiffLine[]> {
  if (isNativeDiffAvailable()) {
    try {
      const fileDiff = await computeFileDiff(oldText, newText, 3);
      if (fileDiff && fileDiff.hunks.length > 0) {
        return fileDiffResultToDiffLines(fileDiff);
      }
    } catch {
      // Fall through to buildLineDiff
    }
  }

  return buildLineDiff(oldText, newText);
}
