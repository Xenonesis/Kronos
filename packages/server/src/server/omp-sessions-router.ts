import { Router } from "express";
import type { Logger } from "pino";
import path from "node:path";
import {
  listOmpImportableSessions,
  resolveOmpSessionFile,
} from "./agent/providers/omp/session-descriptor.js";
import { readActiveOmpEntryChain } from "./agent/providers/omp/history.js";

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

export function createOmpSessionsRouter(logger: Logger): Router {
  const router = Router();

  // 1. List OMP sessions (supports ?cwd=... to filter by current project or all)
  router.get("/", async (req, res) => {
    try {
      const cwd = typeof req.query.cwd === "string" && req.query.cwd.trim() ? req.query.cwd.trim() : undefined;
      const limit = typeof req.query.limit === "string" ? parseInt(req.query.limit, 10) : 50;

      const sessions = await listOmpImportableSessions({
        cwd,
        limit: isNaN(limit) ? 50 : limit,
      });

      const summaries: OmpSessionSummary[] = await Promise.all(
        sessions.map(async (s) => {
          let turnCount = 0;
          let toolCallCount = 0;
          let model: string | null = null;
          let thinkingOptionId: string | null = null;

          try {
            const chain = await readActiveOmpEntryChain(s.providerHandleId);
            turnCount = chain.filter((e) => e.type === "message" || e.message).length;
            for (const entry of chain) {
              if (entry.type === "model_change" && typeof entry.model === "string") {
                model = entry.model;
              }
              if (entry.type === "thinking_level_change" && typeof entry.thinkingLevel === "string") {
                thinkingOptionId = entry.thinkingLevel;
              }
              const msg = entry.message;
              if (msg && typeof msg === "object" && "content" in msg && Array.isArray(msg.content)) {
                for (const part of msg.content) {
                  if (part && typeof part === "object" && "type" in part && part.type === "toolCall") {
                    toolCallCount += 1;
                  }
                }
              }
            }
          } catch {
            // ignore partial read errors on active/corrupted files
          }

          return {
            id: path.basename(s.providerHandleId, ".jsonl"),
            filePath: s.providerHandleId,
            cwd: s.cwd,
            title: s.title,
            firstPrompt: s.firstPromptPreview,
            lastPrompt: s.lastPromptPreview,
            lastActivityAt: s.lastActivityAt.toISOString(),
            model,
            thinkingOptionId,
            turnCount,
            toolCallCount,
          };
        }),
      );

      res.json({
        ok: true,
        sessions: summaries,
      });
    } catch (error) {
      logger.error({ err: error }, "Failed to list OMP sessions");
      res.status(500).json({ ok: false, error: (error as Error).message });
    }
  });

  // 2. Get full details, timeline turns, and file edits/diffs for a specific session
  router.get("/:idOrPath", async (req, res) => {
    try {
      const idOrPath = req.params.idOrPath;
      const cwd = typeof req.query.cwd === "string" ? req.query.cwd : undefined;

      // If idOrPath is full filename like 2026-09-26T..._01a0dd04-... extract suffix or UUID
      const candidateUuid = idOrPath.includes("_") ? idOrPath.split("_").pop() ?? idOrPath : idOrPath;
      const sessionFile =
        (await resolveOmpSessionFile(candidateUuid, { cwd })) ??
        (await resolveOmpSessionFile(idOrPath, { cwd }));
      if (!sessionFile) {
        res.status(404).json({ ok: false, error: "OMP session file not found" });
        return;
      }

      const chain = await readActiveOmpEntryChain(sessionFile);
      if (!chain || chain.length === 0) {
        res.status(404).json({ ok: false, error: "OMP session transcript empty" });
        return;
      }

      let sessionCwd = "";
      let title: string | null = null;
      let createdAt: string | null = null;
      let lastActivityAt: string | null = null;
      let model: string | null = null;
      let thinkingOptionId: string | null = null;

      const turns: OmpSessionDetails["turns"] = [];
      const fileDiffsMap = new Map<string, OmpFileDiff>();
      let totalToolCalls = 0;

      for (const entry of chain) {
        if (entry.type === "session" && typeof entry.cwd === "string") {
          sessionCwd = entry.cwd;
          if (entry.timestamp) createdAt = new Date(entry.timestamp).toISOString();
        }
        if (entry.type === "title" && typeof entry.title === "string") {
          title = entry.title;
        }
        if (entry.type === "model_change" && typeof entry.model === "string") {
          model = entry.model;
        }
        if (entry.type === "thinking_level_change" && typeof entry.thinkingLevel === "string") {
          thinkingOptionId = entry.thinkingLevel;
        }
        if (entry.timestamp) {
          lastActivityAt = new Date(entry.timestamp).toISOString();
        }

        const msg = entry.message;
        if (msg) {
          const rawRole = typeof msg.role === "string" ? msg.role : "custom";
          const role: OmpSessionDetails["turns"][number]["role"] =
            rawRole === "user" || rawRole === "assistant" || rawRole === "toolResult" || rawRole === "system"
              ? rawRole
              : "custom";
          let text: string | undefined;
          const toolCalls: Array<{ id: string; name: string; args: unknown }> = [];

          if (typeof msg.content === "string") {
            text = msg.content;
          } else if (Array.isArray(msg.content)) {
            const textParts: string[] = [];
            for (const part of msg.content) {
              if (!part || typeof part !== "object") continue;
              if ("type" in part && part.type === "text" && "text" in part && typeof part.text === "string") {
                textParts.push(part.text);
              } else if ("type" in part && part.type === "toolCall") {
                totalToolCalls += 1;
                const toolName = "name" in part && typeof part.name === "string" ? part.name : "";
                const args = "arguments" in part ? part.arguments : undefined;
                const callId = "id" in part && typeof part.id === "string" ? part.id : `call-${totalToolCalls}`;
                toolCalls.push({
                  id: callId,
                  name: toolName,
                  args,
                });

                // Detect file edit operations (edit, write, ast_edit, patch)
                if (
                  ["edit", "write", "ast_edit"].includes(toolName) &&
                  args &&
                  typeof args === "object" &&
                  "path" in args &&
                  typeof args.path === "string"
                ) {
                  const targetPath = args.path;
                  fileDiffsMap.set(targetPath, {
                    path: targetPath,
                    tool: toolName,
                    input: args,
                    status: "completed",
                  });
                }
              }
            }
            if (textParts.length > 0) {
              text = textParts.join("\n");
            }
          }

          let toolResult: OmpSessionDetails["turns"][number]["toolResult"];
          if (role === "toolResult") {
            toolResult = {
              toolCallId: String(msg.toolCallId || ""),
              toolName: typeof msg.toolName === "string" ? msg.toolName : undefined,
              content: msg.content,
              isError: Boolean(msg.isError),
            };
          }

          turns.push({
            id: entry.id || `entry-${turns.length}`,
            role,
            text,
            timestamp: entry.timestamp ? new Date(entry.timestamp).toISOString() : undefined,
            toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
            toolResult,
          });
        }
      }

      const fileDiffs = Array.from(fileDiffsMap.values());

      res.json({
        ok: true,
        session: {
          id: path.basename(sessionFile, ".jsonl"),
          filePath: sessionFile,
          cwd: sessionCwd || cwd || "",
          title: title || path.basename(sessionFile, ".jsonl"),
          createdAt,
          lastActivityAt,
          model,
          thinkingOptionId,
          turns,
          fileDiffs,
          stats: {
            totalTurns: turns.length,
            totalToolCalls,
            filesModified: fileDiffs.length,
          },
        },
      });
    } catch (error) {
      logger.error({ err: error }, "Failed to inspect OMP session");
      res.status(500).json({ ok: false, error: (error as Error).message });
    }
  });

  return router;
}
