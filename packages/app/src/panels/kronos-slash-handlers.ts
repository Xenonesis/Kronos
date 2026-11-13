import type { ClientSlashCommand } from "@/client-slash-commands";
import {
  extractArtifactsFromMessages,
  type ArtifactItem,
} from "@/components/artifacts-canvas-modal";
import type { BugAuditItem } from "@/desktop/components";
import { compactSessionHistory, type RawChatMessage } from "@/agent-stream/session-compactor";
import { formatTestRepairPrompt, parseTestOutput } from "@/agent-stream/test-driven-loop";
import { synthesizeModelProposals, type ModelProposal } from "@/agent-stream/consensus-synthesizer";
import { triggerVoiceDictationToggle } from "@/composer/input/input";
import { useBrowserStore } from "@/desktop/browser/store";
import type { AppSettings, AppSettingsUpdate } from "@/hooks/use-settings";
import { buildDraftStoreKey } from "@/stores/draft-keys";
import { useDraftStore } from "@/stores/draft-store";
import { useSessionStore, type Agent } from "@/stores/session-store";
import { generateMessageId, type StreamItem } from "@/types/stream";
import { invokeTauri } from "@/utils/tauri-invoke";

export interface SlashToastApi {
  show: (message: string, opts?: { variant?: "success" | "error" | "info" }) => void;
}

export interface KronosSlashContext {
  agent: Agent;
  agentId: string;
  serverId: string;
  workspaceId?: string;
  tabId: string;
  cwd: string;
  toast: SlashToastApi;
  settings: AppSettings;
  updateSettings: (updates: AppSettingsUpdate) => Promise<void>;
  onOpenSemanticSearch: () => void;
  onOpenArtifacts: (items: ArtifactItem[]) => void;
  onOpenBugHunter: (items: BugAuditItem[]) => void;
  onOpenPreviewBrowser: () => void;
}

interface BugAuditReport {
  totalIssues: number;
  criticalCount: number;
  highCount: number;
  mediumCount: number;
  lowCount: number;
  issues: BugAuditItem[];
}

interface TestCommandRunResult {
  exitCode: number;
  passed: boolean;
  stdout: string;
  stderr: string;
  combined: string;
}

interface AvailableAiModel {
  id: string;
  provider: string;
  model: string;
  displayName: string;
}

interface ArenaComparisonResult {
  id: string;
  provider: string;
  model: string;
  displayName: string;
  response: string;
  error?: string | null;
}

const PERSONA_MODES = [
  "architect",
  "code",
  "ask",
  "test",
  "debugger",
  "security",
  "reviewer",
] as const;

function readAndConsumeDraftTail(serverId: string, agentId: string, commandName: string): string {
  const draftKey = buildDraftStoreKey({ serverId, agentId });
  const raw = useDraftStore.getState().drafts[draftKey]?.input.text ?? "";
  const trimmed = raw.trim();
  if (!trimmed) return "";
  const slashPrefix = `/${commandName}`;
  if (trimmed.toLowerCase().startsWith(slashPrefix.toLowerCase())) {
    const rest = trimmed.slice(slashPrefix.length).trim();
    useDraftStore.getState().replaceDraftText({ draftKey, text: "" });
    return rest;
  }
  return trimmed;
}

function collectTimelineMessages(serverId: string, agentId: string): StreamItem[] {
  const session = useSessionStore.getState().sessions[serverId];
  if (!session) return [];
  const head = session.agentStreamHead.get(agentId) ?? [];
  const tail = session.agentStreamTail.get(agentId) ?? [];
  return [...head, ...tail];
}

function toRawChatMessages(items: StreamItem[]): RawChatMessage[] {
  const messages: RawChatMessage[] = [];
  for (const item of items) {
    if (item.kind === "user_message" && item.text.trim()) {
      messages.push({ role: "user", content: item.text });
    } else if (item.kind === "assistant_message" && item.text.trim()) {
      messages.push({ role: "assistant", content: item.text });
    }
  }
  return messages;
}

function resolveDefaultTestCommand(cwd: string): string {
  return cwd.toLowerCase().includes("src-tauri") ? "cargo test" : "npm test";
}

async function handleCompactSession(ctx: KronosSlashContext): Promise<void> {
  const items = collectTimelineMessages(ctx.serverId, ctx.agentId);
  const rawMessages = toRawChatMessages(items);
  const compacted = compactSessionHistory(rawMessages);

  if (compacted.tokensFreedEstimate <= 0) {
    ctx.toast.show(compacted.summary, { variant: "info" });
    return;
  }

  try {
    await invokeTauri("save_kronos_workspace_doc", {
      cwd: ctx.cwd,
      relativePath: ".kronos/notepads/compacted-context.md",
      content: compacted.summary,
    });
  } catch {
    // Non-fatal outside desktop runtime
  }

  const draftKey = buildDraftStoreKey({ serverId: ctx.serverId, agentId: ctx.agentId });
  useDraftStore.getState().replaceDraftText({
    draftKey,
    text: `${compacted.summary}\n\nContinue from this compacted context:`,
  });

  ctx.toast.show(
    `Compacted ${rawMessages.length} turns into ${compacted.condensedMessages.length} entries (~${compacted.tokensFreedEstimate} tokens freed)`,
    { variant: "success" },
  );
}

async function handleTestLoop(ctx: KronosSlashContext): Promise<void> {
  const tailArg = readAndConsumeDraftTail(ctx.serverId, ctx.agentId, "test-loop");
  const command = tailArg || resolveDefaultTestCommand(ctx.cwd);
  ctx.toast.show(`Running "${command}"...`, { variant: "info" });

  try {
    const res = await invokeTauri<TestCommandRunResult>("run_workspace_test_command", {
      cwd: ctx.cwd,
      command,
    });
    if (!res) {
      ctx.toast.show("Test runner requires Kronos desktop runtime", { variant: "error" });
      return;
    }
    if (res.passed) {
      ctx.toast.show(`Tests passed (exit 0) for \`${command}\``, { variant: "success" });
      return;
    }

    const parsed = parseTestOutput(res.combined);
    const snippet = parsed.failureSnippet || res.combined.slice(-2500);
    const repairPrompt = formatTestRepairPrompt(command, 1, 3, snippet);

    const client = useSessionStore.getState().sessions[ctx.serverId]?.client;
    if (client) {
      await client.sendAgentMessage(ctx.agentId, repairPrompt, {
        messageId: generateMessageId(),
        images: [],
        attachments: [],
      });
      ctx.toast.show(
        `Detected ${parsed.failedTestsCount} failing test(s) — dispatched repair turn`,
        { variant: "error" },
      );
    } else {
      const draftKey = buildDraftStoreKey({ serverId: ctx.serverId, agentId: ctx.agentId });
      useDraftStore.getState().replaceDraftText({ draftKey, text: repairPrompt });
      ctx.toast.show(
        `Tests failed (${parsed.failedTestsCount}); repair prompt loaded in composer`,
        { variant: "error" },
      );
    }
  } catch (err) {
    ctx.toast.show(`Test loop failed: ${String(err)}`, { variant: "error" });
  }
}

async function handleAuditCodebase(ctx: KronosSlashContext): Promise<void> {
  ctx.toast.show("Scanning workspace for bugs and security issues...", { variant: "info" });
  try {
    const report = await invokeTauri<BugAuditReport>("run_codebase_bug_audit", { cwd: ctx.cwd });
    if (!report) {
      ctx.toast.show("Bug audit requires Kronos desktop runtime", { variant: "error" });
      return;
    }
    ctx.onOpenBugHunter(report.issues ?? []);
    ctx.toast.show(
      report.totalIssues === 0
        ? "Codebase audit complete: 0 issues detected"
        : `Codebase audit found ${report.totalIssues} issue(s) (${report.criticalCount} critical, ${report.highCount} high)`,
      { variant: report.totalIssues > 0 ? "error" : "success" },
    );
  } catch (err) {
    ctx.toast.show(`Bug audit failed: ${String(err)}`, { variant: "error" });
  }
}

async function buildConsensusDraftText(
  taskPrompt: string,
  cwd: string,
  validProposals: ModelProposal[],
): Promise<{ finalText: string; statusMessage: string }> {
  try {
    const llmRes = await invokeTauri<{ text: string; provider: string; model: string }>(
      "synthesize_consensus_patch",
      {
        taskPrompt,
        cwd,
        proposals: validProposals.map((p) => ({
          modelName: p.modelName,
          solutionText: p.solutionText,
        })),
      },
    );
    if (llmRes?.text?.trim()) {
      return {
        finalText: `[Consensus Synthesis (${llmRes.provider}/${llmRes.model}) — ${validProposals.length} models]\n\n${llmRes.text.trim()}`,
        statusMessage: `Synthesized ${validProposals.length} model responses via LLM consensus judge`,
      };
    }
  } catch {
    // Fallback cleanly to deterministic synthesis if judge model call fails
  }

  const fallback = synthesizeModelProposals(taskPrompt, validProposals);
  return {
    finalText: fallback.synthesizedPrompt || taskPrompt,
    statusMessage: `Synthesized ${validProposals.length} model responses (confidence ${Math.round(fallback.confidenceScore * 100)}%)`,
  };
}

async function handleConsensusMode(ctx: KronosSlashContext): Promise<void> {
  const taskPrompt = readAndConsumeDraftTail(ctx.serverId, ctx.agentId, "consensus");
  if (!taskPrompt) {
    ctx.toast.show("Provide a prompt (e.g. /consensus <task>) to run multi-model debate", {
      variant: "info",
    });
    return;
  }

  ctx.toast.show("Querying available AI models in parallel...", { variant: "info" });
  try {
    const available = await invokeTauri<AvailableAiModel[]>("list_available_ai_models");
    const candidates = (available ?? []).slice(0, 2);
    if (candidates.length === 0) {
      ctx.toast.show(
        "No active local/CLI models detected for consensus (configure Ollama, LM Studio, Claude, or Codex).",
        { variant: "error" },
      );
      return;
    }

    const results = await invokeTauri<ArenaComparisonResult[]>("run_model_arena_comparison", {
      prompt: taskPrompt,
      cwd: ctx.cwd,
      candidates: candidates.map((c) => ({
        provider: c.provider,
        model: c.model,
        displayName: c.displayName,
      })),
    });

    const validProposals: ModelProposal[] = (results ?? [])
      .filter((r) => !r.error && r.response.trim())
      .map((r) => ({
        modelId: r.model,
        modelName: r.displayName || `${r.provider}/${r.model}`,
        solutionText: r.response,
      }));

    if (validProposals.length === 0) {
      const firstErr = results?.find((r) => r.error)?.error ?? "All model queries failed";
      ctx.toast.show(`Consensus failed: ${firstErr}`, { variant: "error" });
      return;
    }

    const { finalText, statusMessage } = await buildConsensusDraftText(
      taskPrompt,
      ctx.cwd,
      validProposals,
    );
    const draftKey = buildDraftStoreKey({ serverId: ctx.serverId, agentId: ctx.agentId });
    useDraftStore.getState().replaceDraftText({
      draftKey,
      text: finalText,
    });
    ctx.toast.show(statusMessage, { variant: "success" });
  } catch (err) {
    ctx.toast.show(`Consensus execution failed: ${String(err)}`, { variant: "error" });
  }
}

async function handleAgentMode(ctx: KronosSlashContext): Promise<void> {
  const requested = readAndConsumeDraftTail(ctx.serverId, ctx.agentId, "mode").toLowerCase();
  const current = (ctx.agent.currentModeId ?? "code").toLowerCase();
  const currentIdx = PERSONA_MODES.indexOf(current as (typeof PERSONA_MODES)[number]);
  const nextMode =
    PERSONA_MODES.find((m) => m === requested) ??
    PERSONA_MODES[(currentIdx + 1) % PERSONA_MODES.length]!;

  try {
    await invokeTauri("set_agent_execution_mode", {
      agentId: ctx.agentId,
      modeId: nextMode,
    });
  } catch (err) {
    ctx.toast.show(`Failed to persist mode on backend: ${String(err)}`, { variant: "error" });
  }

  const client = useSessionStore.getState().sessions[ctx.serverId]?.client;
  if (client) {
    void client.setAgentMode(ctx.agentId, nextMode).catch(() => {
      // Host may manage mode locally when daemon doesn't expose dynamic mode switch
    });
  }

  ctx.toast.show(`Agent persona mode set to "${nextMode}"`, { variant: "success" });
}

export async function syncPermissionPolicyToHost(settings: AppSettings): Promise<void> {
  try {
    await invokeTauri("set_permission_policy", {
      policy: {
        executionMode: settings.executionMode ?? "guarded",
        autoApproveReadFiles: settings.autoApproveReadFiles ?? true,
        autoApproveTerminalSafe: settings.autoApproveTerminalSafe ?? true,
        autoApproveFileEdits: settings.autoApproveFileEdits ?? false,
        autoApproveWebSearch: settings.autoApproveWebSearch ?? true,
        autoApproveMcp: settings.autoApproveMcp ?? false,
      },
    });
  } catch {
    // Ignore outside desktop runtime
  }
}

async function handleYoloToggle(ctx: KronosSlashContext): Promise<void> {
  const nextMode: "guarded" | "yolo" = ctx.settings.executionMode === "yolo" ? "guarded" : "yolo";
  await ctx.updateSettings({ executionMode: nextMode });
  await syncPermissionPolicyToHost({ ...ctx.settings, executionMode: nextMode });
  ctx.toast.show(
    nextMode === "yolo"
      ? "YOLO Autonomous Mode ENABLED (destructive safety guardrails stay active)"
      : "Guarded Safe Execution Mode ENABLED",
    { variant: "success" },
  );
}

async function handleSwarmDispatch(ctx: KronosSlashContext): Promise<void> {
  const task = readAndConsumeDraftTail(ctx.serverId, ctx.agentId, "swarm");
  if (!task) {
    ctx.toast.show("Provide a task after /swarm <task> to spawn a worker subagent", {
      variant: "info",
    });
    return;
  }

  const client = useSessionStore.getState().sessions[ctx.serverId]?.client;
  if (!client) {
    ctx.toast.show("Daemon client unavailable — cannot spawn subagent", { variant: "error" });
    return;
  }

  try {
    const created = await client.createAgent({
      config: {
        provider: ctx.agent.provider,
        cwd: ctx.agent.cwd || ctx.cwd,
        ...(ctx.agent.currentModeId ? { modeId: ctx.agent.currentModeId } : {}),
        ...(ctx.agent.model ? { model: ctx.agent.model } : {}),
      },
      workspaceId: ctx.agent.workspaceId,
      callerAgentId: ctx.agentId,
      labels: {
        "paseo.parent-agent-id": ctx.agentId,
        role: "swarm-worker",
      },
      initialPrompt: task,
      clientMessageId: generateMessageId(),
    });
    ctx.toast.show(`Swarm worker "${created.id.slice(0, 8)}" spawned for task`, {
      variant: "success",
    });
  } catch (err) {
    ctx.toast.show(`Failed to spawn swarm worker: ${String(err)}`, { variant: "error" });
  }
}

async function handleArtifactsViewCommand(ctx: KronosSlashContext): Promise<void> {
  const messages = collectTimelineMessages(ctx.serverId, ctx.agentId);
  const items = extractArtifactsFromMessages(
    messages.map((m) => ({
      id: m.id,
      kind: m.kind,
      text: "text" in m && typeof m.text === "string" ? m.text : undefined,
    })),
  );
  ctx.onOpenArtifacts(items);
}

async function handleWebPreviewCommand(ctx: KronosSlashContext): Promise<void> {
  try {
    const detectedUrl = await invokeTauri<string>("detect_workspace_preview_url", {
      cwd: ctx.cwd,
    });
    if (detectedUrl) {
      useBrowserStore.getState().updateBrowser("preview", { url: detectedUrl });
    }
    ctx.onOpenPreviewBrowser();
    ctx.toast.show(`Preview browser opened at ${detectedUrl || "http://localhost:3000"}`, {
      variant: "success",
    });
  } catch {
    ctx.onOpenPreviewBrowser();
  }
}

export async function executeKronosClientSlashCommand(
  kind: ClientSlashCommand["kind"],
  ctx: KronosSlashContext,
): Promise<boolean> {
  const handlers: Record<string, () => Promise<void> | void> = {
    "artifacts-view": () => handleArtifactsViewCommand(ctx),
    "semantic-search": () => ctx.onOpenSemanticSearch(),
    "audit-codebase": () => handleAuditCodebase(ctx),
    "compact-session": () => handleCompactSession(ctx),
    "test-loop": () => handleTestLoop(ctx),
    "consensus-mode": () => handleConsensusMode(ctx),
    "agent-mode": () => handleAgentMode(ctx),
    "yolo-mode": () => handleYoloToggle(ctx),
    "voice-dictation": () => triggerVoiceDictationToggle(),
    "agent-swarm": () => handleSwarmDispatch(ctx),
    "web-preview": () => handleWebPreviewCommand(ctx),
    "preview-tunnel": () => handlePreviewTunnelCommand(ctx),
  };

  const handler = handlers[kind];
  if (!handler) {
    return false;
  }
  await handler();
  return true;
}

async function handlePreviewTunnelCommand(ctx: KronosSlashContext): Promise<void> {
  try {
    const current = await invokeTauri<{ status: string }>("get_preview_tunnel_state");
    if (current?.status === "active") {
      await invokeTauri("stop_preview_tunnel");
      ctx.toast.show("Stopped active public preview tunnel", { variant: "success" });
      return;
    }

    const detectedUrl =
      (await invokeTauri<string>("detect_workspace_preview_url", { cwd: ctx.cwd }).catch(
        () => null,
      )) ?? "http://localhost:3000";

    let port = 3000;
    try {
      const parsed = new URL(detectedUrl);
      if (parsed.port) port = Number.parseInt(parsed.port, 10) || 3000;
    } catch {
      port = 3000;
    }

    useBrowserStore.getState().updateBrowser("preview", { url: detectedUrl });
    ctx.onOpenPreviewBrowser();
    ctx.toast.show(`Opening public preview tunnel for localhost:${port}...`);

    try {
      const started = await invokeTauri<{ status: string; publicUrl?: string }>(
        "start_preview_tunnel",
        { localPort: port, provider: "cloudflare" },
      );
      if (started?.publicUrl) {
        ctx.toast.show(`Public URL ready: ${started.publicUrl}`, { variant: "success" });
      }
    } catch (cfErr) {
      const boreStarted = await invokeTauri<{ status: string; publicUrl?: string }>(
        "start_preview_tunnel",
        { localPort: port, provider: "bore" },
      ).catch(() => null);
      if (boreStarted?.publicUrl) {
        ctx.toast.show(`Public URL ready: ${boreStarted.publicUrl}`, { variant: "success" });
      } else {
        ctx.toast.show(`Tunnel failed: ${String(cfErr)}`, { variant: "error" });
      }
    }
  } catch (err) {
    ctx.toast.show(`Preview tunnel failed: ${String(err)}`, { variant: "error" });
  }
}
