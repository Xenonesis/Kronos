import { i18n } from "@/i18n/i18next";
import { confirmDialog } from "@/utils/confirm-dialog";
import { getDesktopHost, isElectronRuntime } from "@/desktop/host";
import { invokeDesktopCommand } from "@/desktop/electron/invoke";
import type { AgentSkillSelection } from "@getpaseo/protocol/messages";
import { isWeb } from "@/constants/platform";
import type { HostConnection } from "@/types/host-connection";

// Refreshed only by native status/start responses, never persisted endpoint hints.
let managedDesktopStatus: DesktopDaemonStatus | null = null;
let daemonStatusRequest = 0;

export function getManagedDesktopStatus(): Readonly<DesktopDaemonStatus> | null {
  return managedDesktopStatus;
}

export function getManagedDesktopTransport(
  connection: HostConnection,
  serverId: string | undefined,
): EmbeddedTransportTarget | null {
  if (connection.type !== "directSocket" || connection.path !== "ipc://embedded") return null;
  const status = managedDesktopStatus;
  if (
    !serverId ||
    status?.serverId !== serverId ||
    status.engine !== "rust" ||
    !status.desktopManaged ||
    !status.ownedByDesktop ||
    status.localTransport !== "ipc"
  ) {
    throw new Error(
      "Managed embedded transport is unavailable for this host; refresh desktop status.",
    );
  }
  if (status.enginePhase?.phase !== "ready") {
    throw new Error(
      status.enginePhase?.phase === "failed"
        ? status.enginePhase.reason
        : "Embedded engine is not ready.",
    );
  }
  return { transportType: "embedded" };
}

export type DesktopDaemonState = "starting" | "running" | "stopped" | "errored";
export type DesktopDaemonStopReason =
  | "manual_ipc"
  | "settings"
  | "host_remove"
  | "quit"
  | "app_update"
  | "version_mismatch"
  | "restart";

export type DesktopEnginePhase =
  | { phase: "stopped" | "starting" | "ready" }
  | { phase: "failed"; reason: string };

export interface DesktopDaemonStatus {
  serverId: string;
  status: DesktopDaemonState;
  listen: string | null;
  hostname: string | null;
  pid: number | null;
  home: string;
  version: string | null;
  desktopManaged: boolean;
  ownedByDesktop: boolean;
  startedAt: string | null;
  error: string | null;
  engine?: "rust" | "node";
  enginePhase?: DesktopEnginePhase;
  networkAvailable?: boolean;
  networkError?: string | null;
  localTransport?: "ipc" | "websocket";
}

export interface DesktopDaemonLogs {
  logPath: string;
  contents: string;
}
export interface DesktopCrashLogs {
  logPath: string;
  contents: string;
}

export interface DesktopAppLogs {
  logPath: string;
  contents: string;
}

export interface DesktopUpdaterDiagnosticFile {
  path: string;
  exists: boolean;
  modifiedAt: string | null;
  contents: string;
  error: string | null;
}

export interface DesktopUpdaterDiagnostics {
  platform: string;
  currentVersion: string;
  targetVersion: string | null;
  targetVersionError: string | null;
  shipItDirectory: string | null;
  state: DesktopUpdaterDiagnosticFile | null;
  stdout: DesktopUpdaterDiagnosticFile | null;
  stderr: DesktopUpdaterDiagnosticFile | null;
}

export interface LocalTransportTarget {
  [key: string]: unknown;
  transportType: "socket" | "pipe";
  transportPath: string;
}

export interface RemoteSshTransportTarget {
  [key: string]: unknown;
  transportType: "ssh";
  host: string;
  sshPort?: number;
  daemonPort?: number;
}

export interface EmbeddedTransportTarget {
  [key: string]: unknown;
  transportType: "embedded";
}

export type DesktopDaemonTransportTarget =
  | LocalTransportTarget
  | RemoteSshTransportTarget
  | EmbeddedTransportTarget;

export interface OpenLocalTransportSessionInput {
  [key: string]: unknown;
  sessionId: string;
  target: DesktopDaemonTransportTarget;
  generation?: string;
}

interface LocalTransportEventPayload {
  sessionId: string;
  generation?: string;
  sequence?: number;
  kind: "open" | "message" | "close" | "error";
  text?: string | null;
  messages?: string[] | null;
  binaryBase64?: string | null;
  code?: number | null;
  reason?: string | null;
  error?: string | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function toStringOrNull(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

function toNumberOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function parseDesktopDaemonState(value: unknown): DesktopDaemonState {
  const normalized = toStringOrNull(value)?.toLowerCase();
  switch (normalized) {
    case "starting":
      return "starting";
    case "running":
      return "running";
    case "errored":
    case "error":
      return "errored";
    case "stopped":
    case "stopping":
    case "unknown":
    default:
      return "stopped";
  }
}

function parseDesktopEnginePhase(raw: unknown): DesktopEnginePhase | undefined {
  if (!isRecord(raw)) return undefined;
  switch (raw.phase) {
    case "stopped":
    case "starting":
    case "ready":
      return { phase: raw.phase };
    case "failed":
      return typeof raw.reason === "string" ? { phase: "failed", reason: raw.reason } : undefined;
    default:
      return undefined;
  }
}

function parseDesktopDaemonStatus(raw: unknown): DesktopDaemonStatus {
  if (!isRecord(raw)) {
    throw new Error("Unexpected desktop daemon status response.");
  }
  if (
    raw.localTransport !== undefined &&
    raw.localTransport !== "ipc" &&
    raw.localTransport !== "websocket"
  ) {
    throw new Error("Unknown desktop local transport setting.");
  }
  const enginePhase = parseDesktopEnginePhase(raw.enginePhase);
  const status: DesktopDaemonStatus = {
    serverId: toStringOrNull(raw.serverId) ?? "",
    status: parseDesktopDaemonState(raw.status),
    listen: toStringOrNull(raw.listen),
    hostname: toStringOrNull(raw.hostname),
    pid: toNumberOrNull(raw.pid),
    home: toStringOrNull(raw.home) ?? "",
    version: toStringOrNull(raw.version),
    desktopManaged: raw.desktopManaged === true,
    ownedByDesktop: raw.ownedByDesktop === true,
    startedAt: typeof raw.startedAt === "string" ? raw.startedAt : null,
    error: toStringOrNull(raw.error),
    ...(raw.engine === "rust" || raw.engine === "node" ? { engine: raw.engine } : {}),
    ...(enginePhase ? { enginePhase } : {}),
    ...(raw.localTransport === "ipc" || raw.localTransport === "websocket"
      ? { localTransport: raw.localTransport }
      : {}),
    ...(typeof raw.networkAvailable === "boolean"
      ? { networkAvailable: raw.networkAvailable }
      : {}),
    ...(raw.networkError === null || typeof raw.networkError === "string"
      ? { networkError: raw.networkError }
      : {}),
  };
  return status;
}

function parseDesktopDaemonLogs(raw: unknown): DesktopDaemonLogs {
  if (!isRecord(raw)) {
    throw new Error("Unexpected desktop daemon logs response.");
  }
  return {
    logPath: toStringOrNull(raw.logPath) ?? "",
    contents: typeof raw.contents === "string" ? raw.contents : "",
  };
}

function parseDesktopUpdaterDiagnosticFile(raw: unknown): DesktopUpdaterDiagnosticFile | null {
  if (raw === null) return null;
  if (!isRecord(raw)) {
    throw new Error("Unexpected desktop updater diagnostic file response.");
  }
  return {
    path: toStringOrNull(raw.path) ?? "",
    exists: raw.exists === true,
    modifiedAt: toStringOrNull(raw.modifiedAt),
    contents: typeof raw.contents === "string" ? raw.contents : "",
    error: toStringOrNull(raw.error),
  };
}

function parseDesktopUpdaterDiagnostics(raw: unknown): DesktopUpdaterDiagnostics {
  if (!isRecord(raw)) {
    throw new Error("Unexpected desktop updater diagnostics response.");
  }
  return {
    platform: toStringOrNull(raw.platform) ?? "unknown",
    currentVersion: toStringOrNull(raw.currentVersion) ?? "unknown",
    targetVersion: toStringOrNull(raw.targetVersion),
    targetVersionError: toStringOrNull(raw.targetVersionError),
    shipItDirectory: toStringOrNull(raw.shipItDirectory),
    state: parseDesktopUpdaterDiagnosticFile(raw.state),
    stdout: parseDesktopUpdaterDiagnosticFile(raw.stdout),
    stderr: parseDesktopUpdaterDiagnosticFile(raw.stderr),
  };
}

export function shouldUseDesktopDaemon(): boolean {
  return isElectronRuntime();
}

async function invokeDaemonStatus(
  command: string,
  args?: Record<string, unknown>,
): Promise<DesktopDaemonStatus> {
  const request = ++daemonStatusRequest;
  try {
    const status = parseDesktopDaemonStatus(await invokeDesktopCommand(command, args));
    if (request === daemonStatusRequest) {
      managedDesktopStatus =
        status.localTransport && status.desktopManaged && status.ownedByDesktop
          ? { ...status }
          : null;
    }
    return status;
  } catch (error) {
    if (request === daemonStatusRequest) managedDesktopStatus = null;
    throw error;
  }
}

export async function getDesktopDaemonStatus(): Promise<DesktopDaemonStatus> {
  return invokeDaemonStatus("desktop_daemon_status");
}

export async function startDesktopDaemon(): Promise<DesktopDaemonStatus> {
  return invokeDaemonStatus("start_desktop_daemon");
}

export async function stopDesktopDaemon(
  reason: DesktopDaemonStopReason = "manual_ipc",
): Promise<DesktopDaemonStatus> {
  return invokeDaemonStatus("stop_desktop_daemon", { reason });
}

export async function confirmAndStopDesktopDaemon(): Promise<DesktopDaemonStatus | null> {
  const captured = await getDesktopDaemonStatus();
  if (!captured.pid || !captured.startedAt) return captured;
  const ownership = captured.ownedByDesktop
    ? i18n.t("desktop.daemon.lifecycle.ownedMessage")
    : i18n.t("desktop.daemon.lifecycle.attachedMessage");
  const confirmed = await confirmDialog({
    title: i18n.t("desktop.daemon.lifecycle.stopTitle"),
    message: i18n.t("desktop.daemon.lifecycle.stopMessage", {
      ownership,
      home: captured.home,
      pid: captured.pid,
    }),
    confirmLabel: i18n.t("desktop.daemon.lifecycle.stop"),
    cancelLabel: i18n.t("common.actions.cancel"),
    destructive: true,
  });
  if (!confirmed) return null;
  return invokeDaemonStatus("stop_desktop_daemon", {
    reason: "manual_ipc",
    pid: captured.pid,
    startedAt: captured.startedAt,
  });
}

export async function restartDesktopDaemon(): Promise<DesktopDaemonStatus> {
  return invokeDaemonStatus("restart_desktop_daemon");
}

export async function getDesktopDaemonLogs(): Promise<DesktopDaemonLogs> {
  return parseDesktopDaemonLogs(await invokeDesktopCommand("desktop_daemon_logs"));
}

export async function getDesktopAppLogs(): Promise<DesktopAppLogs> {
  const raw = await invokeDesktopCommand("desktop_app_logs");
  if (!isRecord(raw)) {
    throw new Error("Unexpected desktop app logs response.");
  }
  return {
    logPath: toStringOrNull(raw.logPath) ?? "",
    contents: typeof raw.contents === "string" ? raw.contents : "",
  };
}
export async function getDesktopCrashLogs(): Promise<DesktopCrashLogs> {
  const raw = await invokeDesktopCommand("desktop_crash_logs");
  if (!isRecord(raw)) {
    throw new Error("Unexpected desktop crash logs response.");
  }
  return {
    logPath: toStringOrNull(raw.logPath) ?? "",
    contents: typeof raw.contents === "string" ? raw.contents : "",
  };
}

export async function getDesktopUpdaterDiagnostics(): Promise<DesktopUpdaterDiagnostics> {
  return parseDesktopUpdaterDiagnostics(await invokeDesktopCommand("desktop_update_diagnostics"));
}

export async function getCliDaemonStatus(): Promise<string> {
  const raw = await invokeDesktopCommand<unknown>("cli_daemon_status");
  if (typeof raw !== "string") {
    throw new Error("Unexpected CLI daemon status response.");
  }
  return raw;
}

export type LocalTransportEventUnlisten = () => void;
export type LocalTransportEventHandler = (payload: LocalTransportEventPayload) => void;

interface EmbeddedChannel {
  onmessage: (payload: LocalTransportEventPayload) => void;
}
const embeddedChannels = new Map<string, { generation: string; channel: EmbeddedChannel }>();

export async function listenToLocalTransportEvents(
  handler: LocalTransportEventHandler,
  scope?: { sessionId: string; generation: string },
): Promise<LocalTransportEventUnlisten> {
  if (scope) {
    if (!isWeb || typeof window === "undefined")
      throw new Error("Embedded transport requires a desktop webview");
    const Channel = (
      window as unknown as { __TAURI__?: { core?: { Channel?: new () => EmbeddedChannel } } }
    ).__TAURI__?.core?.Channel;
    if (!Channel) throw new Error("Tauri session channel API is unavailable");
    if (embeddedChannels.has(scope.sessionId))
      throw new Error("Duplicate embedded channel registration");
    const channel = new Channel();
    // oxlint-disable-next-line unicorn/prefer-add-event-listener -- Tauri Channel is not an EventTarget.
    channel.onmessage = handler;
    const entry = { generation: scope.generation, channel };
    embeddedChannels.set(scope.sessionId, entry);
    return () => {
      // oxlint-disable-next-line unicorn/prefer-add-event-listener -- Tauri Channel is not an EventTarget.
      channel.onmessage = () => {};
      if (embeddedChannels.get(scope.sessionId) === entry) embeddedChannels.delete(scope.sessionId);
    };
  }
  const listen = getDesktopHost()?.events?.on;
  if (typeof listen !== "function") {
    throw new Error("Desktop events API is unavailable.");
  }
  const unlisten = await listen("local-daemon-transport-event", (payload: unknown) => {
    if (!isRecord(payload)) {
      return;
    }
    const messages = Array.isArray(payload.messages)
      ? payload.messages.filter((m): m is string => typeof m === "string" && m.length > 0)
      : null;
    handler({
      sessionId: toStringOrNull(payload.sessionId) ?? "",
      kind: (toStringOrNull(payload.kind) ?? "error") as LocalTransportEventPayload["kind"],
      text: toStringOrNull(payload.text),
      messages: messages && messages.length > 0 ? messages : null,
      binaryBase64: toStringOrNull(payload.binaryBase64),
      code: toNumberOrNull(payload.code),
      reason: toStringOrNull(payload.reason),
      error: toStringOrNull(payload.error),
    });
  });
  return typeof unlisten === "function" ? unlisten : () => {};
}

export async function openLocalTransportSession(
  input: OpenLocalTransportSessionInput,
): Promise<void> {
  if (input.target.transportType === "embedded") {
    const entry = embeddedChannels.get(input.sessionId);
    if (!entry || entry.generation !== input.generation)
      throw new Error("Embedded session channel is not registered");
    await invokeDesktopCommand("open_embedded_transport", {
      input: { sessionId: input.sessionId, generation: input.generation },
      onEvent: entry.channel,
    });
    return;
  }
  await invokeDesktopCommand("open_local_daemon_transport", input);
}

export async function sendLocalTransportMessage(input: {
  sessionId: string;
  generation?: string;
  text?: string;
  binaryBase64?: string;
}): Promise<void> {
  const payload = {
    sessionId: input.sessionId,
    ...(input.generation ? { generation: input.generation } : {}),
    ...(input.text !== undefined ? { text: input.text } : {}),
    ...(input.binaryBase64 !== undefined ? { binaryBase64: input.binaryBase64 } : {}),
  };
  await invokeDesktopCommand(
    input.generation ? "send_embedded_transport" : "send_local_daemon_transport_message",
    input.generation ? { input: payload } : payload,
  );
}

export async function acknowledgeLocalTransportMessage(input: {
  sessionId: string;
  generation: string;
  sequence: number;
}): Promise<void> {
  await invokeDesktopCommand("ack_embedded_transport", { input });
}

export async function closeLocalTransportSession(
  sessionId: string,
  generation?: string,
): Promise<void> {
  await invokeDesktopCommand(
    generation ? "close_embedded_transport" : "close_local_daemon_transport",
    generation ? { input: { sessionId, generation } } : { sessionId },
  );
}

// ---------------------------------------------------------------------------
// Integrations
// ---------------------------------------------------------------------------

export interface InstallStatus {
  installed: boolean;
}

function parseInstallStatus(raw: unknown): InstallStatus {
  if (!isRecord(raw)) {
    throw new Error("Unexpected install status response.");
  }
  return { installed: raw.installed === true };
}

export async function getCliInstallStatus(): Promise<InstallStatus> {
  return parseInstallStatus(await invokeDesktopCommand("get_cli_install_status"));
}

export async function installCli(): Promise<InstallStatus> {
  return parseInstallStatus(await invokeDesktopCommand("install_cli"));
}

export async function uninstallCli(): Promise<InstallStatus> {
  return parseInstallStatus(await invokeDesktopCommand("uninstall_cli"));
}

// COMPAT(desktopSkillSelectionMigration): added in v0.4.0; remove after 2027-02-16.
export function readLegacySkillSelection(): Promise<AgentSkillSelection | null> {
  return invokeDesktopCommand("read_legacy_skill_selection") as Promise<AgentSkillSelection | null>;
}

// COMPAT(desktopSkillSelectionMigration): added in v0.4.0; remove after 2027-02-16.
export async function deleteLegacySkillSelection(): Promise<void> {
  await invokeDesktopCommand("delete_legacy_skill_selection");
}

export interface DesktopSandboxDiagnostics {
  enabled: boolean;
  reason: string;
}

export async function getDesktopSandboxDiagnostics(): Promise<DesktopSandboxDiagnostics> {
  const result: unknown = await invokeDesktopCommand("desktop_sandbox_diagnostics");
  if (
    !isRecord(result) ||
    typeof result.enabled !== "boolean" ||
    typeof result.reason !== "string"
  ) {
    throw new Error("Unexpected desktop sandbox diagnostics response.");
  }
  return { enabled: result.enabled, reason: result.reason };
}
