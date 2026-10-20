import { Router, type Request, type Response } from "express";
import type { Logger } from "pino";
import path from "node:path";
import {
  listOmpImportableSessions,
  resolveOmpSessionFile,
} from "./agent/providers/omp/session-descriptor.js";
import { readActiveOmpEntryChain } from "./agent/providers/omp/history.js";
import type { ImportableProviderSession } from "./agent/agent-sdk-types.js";

export interface OmpSessionSummary {
  id: string;
  filePath: string;
  cwd: string;
  title: string | null;
  firstPrompt: string | null;
  lastPrompt: string | null;
  lastActivityAt: string;
  model: string | null;
  thinkingOptionId: string | null;
  turnCount: number;
  toolCallCount: number;
}

export interface OmpFileDiff {
  path: string;
  tool: string;
  input?: unknown;
  status: "completed" | "error" | "pending";
}

export interface OmpSessionDetails {
  id: string;
  filePath: string;
  cwd: string;
  title: string | null;
  createdAt: string | null;
  lastActivityAt: string | null;
  model: string | null;
  thinkingOptionId: string | null;
  turns: Array<{
    id: string;
    role: "user" | "assistant" | "toolResult" | "custom" | "system";
    text?: string;
    timestamp?: string;
    toolCalls?: Array<{
      id: string;
      name: string;
      args: unknown;
    }>;
    toolResult?: {
      toolCallId: string;
      toolName?: string;
      content: unknown;
      isError?: boolean;
    };
  }>;
  fileDiffs: OmpFileDiff[];
  stats: {
    totalTurns: number;
    totalToolCalls: number;
    filesModified: number;
  };
}

/**
 * One entry of an OMP session transcript, as the chain reader returns it. The transcript is
 * provider-owned JSON, so the extra fields stay `unknown` and are narrowed at the point of use.
 */
interface OmpChainEntry {
  type?: string;
  id?: string;
  timestamp?: string | number;
  message?: Record<string, unknown>;
  cwd?: unknown;
  title?: unknown;
  model?: unknown;
  thinkingLevel?: unknown;
}

type OmpTurn = OmpSessionDetails["turns"][number];
type OmpTurnRole = OmpTurn["role"];
type OmpTurnToolCall = NonNullable<OmpTurn["toolCalls"]>[number];
type OmpTurnToolResult = NonNullable<OmpTurn["toolResult"]>;

/** Tools whose calls are recorded as file edits in the session timeline. */
const FILE_EDIT_TOOLS: Record<string, true> = {
  edit: true,
  write: true,
  ast_edit: true,
};

function normalizeTurnRole(rawRole: unknown): OmpTurnRole {
  if (
    rawRole === "user" ||
    rawRole === "assistant" ||
    rawRole === "toolResult" ||
    rawRole === "system"
  ) {
    return rawRole;
  }
  return "custom";
}

function toIsoTimestamp(timestamp: string | number | undefined): string | undefined {
  if (!timestamp) return undefined;
  return new Date(timestamp).toISOString();
}

function countToolCallParts(message: Record<string, unknown> | undefined): number {
  if (!message || !("content" in message)) return 0;
  const content: unknown = message.content;
  if (!Array.isArray(content)) return 0;
  let count = 0;
  for (const part of content) {
    if (part && typeof part === "object" && "type" in part && part.type === "toolCall") {
      count += 1;
    }
  }
  return count;
}

interface OmpChainSummary {
  turnCount: number;
  toolCallCount: number;
  model: string | null;
  thinkingOptionId: string | null;
}

function readChainSummary(chain: OmpChainEntry[]): OmpChainSummary {
  let model: string | null = null;
  let thinkingOptionId: string | null = null;
  let toolCallCount = 0;

  for (const entry of chain) {
    if (entry.type === "model_change" && typeof entry.model === "string") {
      model = entry.model;
    }
    if (entry.type === "thinking_level_change" && typeof entry.thinkingLevel === "string") {
      thinkingOptionId = entry.thinkingLevel;
    }
    toolCallCount += countToolCallParts(entry.message);
  }

  return {
    turnCount: chain.filter((entry) => entry.type === "message" || entry.message).length,
    toolCallCount,
    model,
    thinkingOptionId,
  };
}

async function buildSessionSummary(session: ImportableProviderSession): Promise<OmpSessionSummary> {
  let summary: OmpChainSummary = {
    turnCount: 0,
    toolCallCount: 0,
    model: null,
    thinkingOptionId: null,
  };

  try {
    const chain = await readActiveOmpEntryChain(session.providerHandleId);
    summary = readChainSummary(chain);
  } catch {
    // ignore partial read errors on active/corrupted files
  }

  return {
    id: path.basename(session.providerHandleId, ".jsonl"),
    filePath: session.providerHandleId,
    cwd: session.cwd,
    title: session.title,
    firstPrompt: session.firstPromptPreview,
    lastPrompt: session.lastPromptPreview,
    lastActivityAt: session.lastActivityAt.toISOString(),
    model: summary.model,
    thinkingOptionId: summary.thinkingOptionId,
    turnCount: summary.turnCount,
    toolCallCount: summary.toolCallCount,
  };
}

function readListQuery(req: Request): { cwd: string | undefined; limit: number } {
  const rawCwd = req.query.cwd;
  const cwd = typeof rawCwd === "string" && rawCwd.trim() ? rawCwd.trim() : undefined;
  const rawLimit = req.query.limit;
  const limit = typeof rawLimit === "string" ? parseInt(rawLimit, 10) : 50;
  return { cwd, limit: isNaN(limit) ? 50 : limit };
}

async function listOmpSessions(req: Request, res: Response, logger: Logger): Promise<void> {
  try {
    const { cwd, limit } = readListQuery(req);
    const sessions = await listOmpImportableSessions({ cwd, limit });
    const summaries = await Promise.all(sessions.map((session) => buildSessionSummary(session)));

    res.json({
      ok: true,
      sessions: summaries,
    });
  } catch (error) {
    logger.error({ err: error }, "Failed to list OMP sessions");
    res.status(500).json({ ok: false, error: (error as Error).message });
  }
}

interface OmpDetailsAccumulator {
  sessionCwd: string;
  title: string | null;
  createdAt: string | null;
  lastActivityAt: string | null;
  model: string | null;
  thinkingOptionId: string | null;
  turns: OmpTurn[];
  fileDiffs: Map<string, OmpFileDiff>;
  totalToolCalls: number;
}

function recordFileDiff(accumulator: OmpDetailsAccumulator, toolName: string, args: unknown): void {
  if (!FILE_EDIT_TOOLS[toolName]) return;
  if (!args || typeof args !== "object" || !("path" in args)) return;
  const targetPath = args.path;
  if (typeof targetPath !== "string") return;
  accumulator.fileDiffs.set(targetPath, {
    path: targetPath,
    tool: toolName,
    input: args,
    status: "completed",
  });
}

function extractTurnContent(
  message: Record<string, unknown>,
  accumulator: OmpDetailsAccumulator,
): { text: string | undefined; toolCalls: OmpTurnToolCall[] } {
  const toolCalls: OmpTurnToolCall[] = [];
  const content: unknown = message.content;

  if (typeof content === "string") {
    return { text: content, toolCalls };
  }
  if (!Array.isArray(content)) {
    return { text: undefined, toolCalls };
  }

  const textParts: string[] = [];
  for (const part of content) {
    if (!part || typeof part !== "object" || !("type" in part)) continue;
    if (part.type === "text" && "text" in part && typeof part.text === "string") {
      textParts.push(part.text);
      continue;
    }
    if (part.type !== "toolCall") continue;
    accumulator.totalToolCalls += 1;
    const toolName = "name" in part && typeof part.name === "string" ? part.name : "";
    const args = "arguments" in part ? part.arguments : undefined;
    const callId =
      "id" in part && typeof part.id === "string" ? part.id : `call-${accumulator.totalToolCalls}`;
    toolCalls.push({ id: callId, name: toolName, args });
    recordFileDiff(accumulator, toolName, args);
  }

  return {
    text: textParts.length > 0 ? textParts.join("\n") : undefined,
    toolCalls,
  };
}

function buildToolResult(message: Record<string, unknown>): OmpTurnToolResult {
  return {
    toolCallId: String(message.toolCallId || ""),
    toolName: typeof message.toolName === "string" ? message.toolName : undefined,
    content: message.content,
    isError: Boolean(message.isError),
  };
}

function applyEntryMetadata(entry: OmpChainEntry, accumulator: OmpDetailsAccumulator): void {
  if (entry.type === "session" && typeof entry.cwd === "string") {
    accumulator.sessionCwd = entry.cwd;
    const createdAt = toIsoTimestamp(entry.timestamp);
    if (createdAt) accumulator.createdAt = createdAt;
  }
  if (entry.type === "title" && typeof entry.title === "string") {
    accumulator.title = entry.title;
  }
  if (entry.type === "model_change" && typeof entry.model === "string") {
    accumulator.model = entry.model;
  }
  if (entry.type === "thinking_level_change" && typeof entry.thinkingLevel === "string") {
    accumulator.thinkingOptionId = entry.thinkingLevel;
  }
  const lastActivityAt = toIsoTimestamp(entry.timestamp);
  if (lastActivityAt) {
    accumulator.lastActivityAt = lastActivityAt;
  }
}

function buildTurn(
  entry: OmpChainEntry,
  message: Record<string, unknown>,
  accumulator: OmpDetailsAccumulator,
): OmpTurn {
  const role = normalizeTurnRole(message.role);
  const { text, toolCalls } = extractTurnContent(message, accumulator);
  const toolResult = role === "toolResult" ? buildToolResult(message) : undefined;

  return {
    id: entry.id || `entry-${accumulator.turns.length}`,
    role,
    text,
    timestamp: toIsoTimestamp(entry.timestamp),
    toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
    toolResult,
  };
}

function buildSessionDetails(chain: OmpChainEntry[]): OmpDetailsAccumulator {
  const accumulator: OmpDetailsAccumulator = {
    sessionCwd: "",
    title: null,
    createdAt: null,
    lastActivityAt: null,
    model: null,
    thinkingOptionId: null,
    turns: [],
    fileDiffs: new Map<string, OmpFileDiff>(),
    totalToolCalls: 0,
  };

  for (const entry of chain) {
    applyEntryMetadata(entry, accumulator);
    const message = entry.message;
    if (!message) continue;
    accumulator.turns.push(buildTurn(entry, message, accumulator));
  }

  return accumulator;
}

async function resolveSessionFile(
  idOrPath: string,
  cwd: string | undefined,
): Promise<string | null> {
  // If idOrPath is full filename like 2026-09-26T..._01a0dd04-... extract suffix or UUID
  const candidateUuid = idOrPath.includes("_") ? (idOrPath.split("_").pop() ?? idOrPath) : idOrPath;
  const resolved = await resolveOmpSessionFile(candidateUuid, { cwd });
  if (resolved) return resolved;
  return resolveOmpSessionFile(idOrPath, { cwd });
}

async function inspectOmpSession(req: Request, res: Response, logger: Logger): Promise<void> {
  try {
    const idOrPath = req.params.idOrPath;
    const cwd = typeof req.query.cwd === "string" ? req.query.cwd : undefined;

    const sessionFile = await resolveSessionFile(idOrPath, cwd);
    if (!sessionFile) {
      res.status(404).json({ ok: false, error: "OMP session file not found" });
      return;
    }

    const chain = await readActiveOmpEntryChain(sessionFile);
    if (!chain || chain.length === 0) {
      res.status(404).json({ ok: false, error: "OMP session transcript empty" });
      return;
    }

    const details = buildSessionDetails(chain);
    const fileDiffs = Array.from(details.fileDiffs.values());
    const sessionId = path.basename(sessionFile, ".jsonl");

    res.json({
      ok: true,
      session: {
        id: sessionId,
        filePath: sessionFile,
        cwd: details.sessionCwd || cwd || "",
        title: details.title || sessionId,
        createdAt: details.createdAt,
        lastActivityAt: details.lastActivityAt,
        model: details.model,
        thinkingOptionId: details.thinkingOptionId,
        turns: details.turns,
        fileDiffs,
        stats: {
          totalTurns: details.turns.length,
          totalToolCalls: details.totalToolCalls,
          filesModified: fileDiffs.length,
        },
      },
    });
  } catch (error) {
    logger.error({ err: error }, "Failed to inspect OMP session");
    res.status(500).json({ ok: false, error: (error as Error).message });
  }
}

export function createOmpSessionsRouter(logger: Logger): Router {
  const router = Router();

  // 1. List OMP sessions (supports ?cwd=... to filter by current project or all)
  router.get("/", (req, res) => {
    void listOmpSessions(req, res, logger);
  });

  // 2. Get full details, timeline turns, and file edits/diffs for a specific session
  router.get("/:idOrPath", (req, res) => {
    void inspectOmpSession(req, res, logger);
  });

  return router;
}
