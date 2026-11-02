import { BuiltinPluginLoader } from "../plugins/builtin/index.js";
import os from "node:os";
import path from "node:path";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import net from "node:net";
import { spawn } from "node:child_process";

import pino from "pino";
import {
  createPaseoDaemon,
  type PaseoDaemon,
  type PaseoDaemonConfig,
  type PaseoOpenAIConfig,
  type PaseoSpeechConfig,
} from "../bootstrap.js";
import type { AgentClient, AgentProvider } from "../agent/agent-sdk-types.js";
import { createTestAgentClients } from "./fake-agent-client.js";
import type { PushNotificationSender } from "../push/index.js";
import type { AgentProfile } from "@getpaseo/protocol/messages";

interface TestPaseoDaemonOptions {
  daemonVersion?: string;
  desktopManaged?: boolean;
  downloadTokenTtlMs?: number;
  corsAllowedOrigins?: string[];
  listen?: string;
  listenPort?: number;
  logger?: Parameters<typeof createPaseoDaemon>[1];
  mcpEnabled?: boolean;
  mcpDebug?: boolean;
  isDev?: boolean;
  relayEnabled?: boolean;
  relayEndpoint?: string;
  relayUseTls?: boolean;
  relayPublicUseTls?: boolean;
  relayRequireProtectedSecurity?: boolean;
  daemonStatusRpcCapability?: boolean;
  relayConfigCapability?: boolean;
  agentClients?: Partial<Record<AgentProvider, AgentClient>>;
  providerOverrides?: PaseoDaemonConfig["providerOverrides"];
  paseoHomeRoot?: string;
  staticDir?: string;
  cleanup?: boolean;
  openai?: PaseoOpenAIConfig;
  // localModelsDir defaults to the local provider's modelsDir, then to a dir under paseoHome.
  speech?: Omit<PaseoSpeechConfig, "localModelsDir"> & { localModelsDir?: string };
  voiceLlmProvider?: PaseoDaemonConfig["voiceLlmProvider"];
  voiceLlmProviderExplicit?: boolean;
  voiceLlmModel?: string | null;
  dictationFinalTimeoutMs?: number;
  auth?: PaseoDaemonConfig["auth"];
  pushNotificationSender?: PushNotificationSender;
  serviceProxy?: PaseoDaemonConfig["serviceProxy"];
  webUi?: PaseoDaemonConfig["webUi"];
  trustedProxies?: PaseoDaemonConfig["trustedProxies"];
  agentProfiles?: AgentProfile[];
  autoArchiveAfterMerge?: boolean;
  resumeOnStartup?: boolean;
  resumePrompt?: string;
  pluginsEnabled?: PaseoDaemonConfig["pluginsEnabled"];
  builtinPlugins?: BuiltinPluginLoader;
  plugins?: PaseoDaemonConfig["plugins"];
}

export interface TestPaseoDaemon {
  config: PaseoDaemonConfig;
  daemon: PaseoDaemon;
  port: number;
  paseoHome: string;
  staticDir: string;
  close: () => Promise<void>;
}

const TEST_DAEMON_START_TIMEOUT_MS = 20_000;

async function startDaemonWithTimeout(daemon: PaseoDaemon, timeoutMs: number): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const timeoutHandle = setTimeout(() => {
      const timeoutError = new Error(
        `Timed out starting test daemon after ${timeoutMs}ms`,
      ) as Error & { code?: string };
      timeoutError.code = "TEST_DAEMON_START_TIMEOUT";
      reject(timeoutError);
    }, timeoutMs);

    daemon.start().then(
      () => {
        clearTimeout(timeoutHandle);
        resolve();
        return;
      },
      (error) => {
        clearTimeout(timeoutHandle);
        reject(error);
      },
    );
  });
}

async function createTestDaemonFromBinary(
  binPath: string,
  options: TestPaseoDaemonOptions,
): Promise<TestPaseoDaemon> {
  const { config, paseoHomeRoot, paseoHome, staticDir } = await prepareTestDaemonConfig(options);
  const server = net.createServer();
  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", () => resolve());
  });
  const port = (server.address() as net.AddressInfo).port;
  await new Promise<void>((resolve) => {
    server.close(() => resolve());
  });

  const child = spawn(binPath, ["--port", String(port)], {
    env: {
      ...process.env,
      KRONOS_PORT: String(port),
      PASEO_PORT: String(port),
      PASEO_HOME: paseoHome,
    },
    stdio: "pipe",
  });

  const start = Date.now();
  let healthy = false;
  while (Date.now() - start < 15_000) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/api/health`);
      if (res.ok) {
        healthy = true;
        break;
      }
    } catch {
      // retry
    }
    await new Promise((r) => setTimeout(r, 100));
  }

  if (!healthy) {
    child.kill();
    throw new Error(`Failed to start test daemon binary at ${binPath} on port ${port}`);
  }

  return {
    config,
    daemon: undefined as unknown as PaseoDaemon,
    port,
    paseoHome,
    staticDir,
    close: async () => {
      child.kill();
      if (options.cleanup ?? true) {
        await removeDirs([paseoHomeRoot, staticDir]);
      }
    },
  };
}

export async function createTestPaseoDaemon(
  options: TestPaseoDaemonOptions = {},
): Promise<TestPaseoDaemon> {
  if (process.env.PASEO_TEST_DAEMON_URL) {
    const rawUrl = process.env.PASEO_TEST_DAEMON_URL;
    const parsedUrl = new URL(rawUrl.startsWith("http") ? rawUrl : `http://${rawUrl}`);
    const port = parseInt(parsedUrl.port, 10) || (parsedUrl.protocol === "https:" ? 443 : 80);
    const { config, paseoHome, staticDir } = await prepareTestDaemonConfig(options);
    return {
      config,
      daemon: undefined as unknown as PaseoDaemon,
      port,
      paseoHome,
      staticDir,
      close: async () => {},
    };
  }

  if (process.env.KRONOS_TEST_DAEMON_BIN) {
    return createTestDaemonFromBinary(process.env.KRONOS_TEST_DAEMON_BIN, options);
  }
  const maxAttempts = 8;
  let lastError: unknown;

  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const { config, paseoHomeRoot, paseoHome, staticDir, createdDirs } =
      await prepareTestDaemonConfig(options);
    const logger = options.logger ?? pino({ level: "silent" });
    const daemon = await createPaseoDaemon(config, logger, {
      builtinPlugins: options.builtinPlugins ?? new BuiltinPluginLoader(undefined, []),
      serverFeatureOverrides: {
        daemonStatusRpc: options.daemonStatusRpcCapability,
        relayConfig: options.relayConfigCapability,
      },
    });
    try {
      await startDaemonWithTimeout(daemon, TEST_DAEMON_START_TIMEOUT_MS);
      const listenTarget = daemon.getListenTarget();
      if (!listenTarget || listenTarget.type !== "tcp") {
        throw new Error("Test daemon did not expose a bound TCP listen target");
      }

      const close = async (): Promise<void> => {
        await daemon.stop().catch(() => undefined);
        await daemon.agentManager.flush().catch(() => undefined);
        if (options.cleanup ?? true) {
          await new Promise((r) => setTimeout(r, 50));
          await removeDirs([paseoHomeRoot, staticDir]);
        }
      };

      return {
        config,
        daemon,
        port: listenTarget.port,
        paseoHome,
        staticDir,
        close,
      };
    } catch (error) {
      lastError = error;
      await daemon.stop().catch(() => undefined);
      // A failed attempt removes only what it created: a caller-supplied home keeps its serverId.
      await removeDirs(createdDirs);

      if (
        (!isAddressInUseError(error) && !isStartupTimeoutError(error)) ||
        attempt === maxAttempts - 1
      ) {
        throw error;
      }
    }
  }

  throw lastError ?? new Error("Failed to start test daemon");
}

interface PreparedTestDaemonConfig {
  config: PaseoDaemonConfig;
  paseoHomeRoot: string;
  paseoHome: string;
  staticDir: string;
  createdDirs: string[];
}

async function removeDirs(dirs: string[]): Promise<void> {
  await Promise.all(
    dirs.map((dir) => rm(dir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })),
  );
}

async function createTempDir(prefix: string, createdDirs: string[]): Promise<string> {
  const dir = await mkdtemp(path.join(os.tmpdir(), prefix));
  createdDirs.push(dir);
  return dir;
}

async function prepareTestDaemonConfig(
  options: TestPaseoDaemonOptions,
): Promise<PreparedTestDaemonConfig> {
  const createdDirs: string[] = [];
  const paseoHomeRoot = options.paseoHomeRoot ?? (await createTempDir("paseo-home-", createdDirs));
  const paseoHome = path.join(paseoHomeRoot, ".paseo");
  await mkdir(paseoHome, { recursive: true });
  const staticDir = options.staticDir ?? (await createTempDir("paseo-static-", createdDirs));
  const listenHost = options.listen ?? "127.0.0.1";
  const listenPort = options.listenPort ?? 0;
  const config: PaseoDaemonConfig = {
    listen: `${listenHost}:${listenPort}`,
    paseoHome,
    daemonVersion: options.daemonVersion,
    desktopManaged: options.desktopManaged,
    corsAllowedOrigins: options.corsAllowedOrigins ?? [],
    hostnames: true,
    mcpEnabled: options.mcpEnabled ?? true,
    staticDir,
    mcpDebug: options.mcpDebug ?? false,
    isDev: options.isDev,
    agentClients: options.agentClients ?? createTestAgentClients(),
    providerOverrides: options.providerOverrides,
    agentStoragePath: path.join(paseoHome, "agents"),
    relayEnabled: options.relayEnabled ?? false,
    relayEndpoint: options.relayEndpoint ?? "relay.paseo.sh:443",
    relayUseTls: options.relayUseTls,
    relayPublicUseTls: options.relayPublicUseTls,
    relayRequireProtectedSecurity: options.relayRequireProtectedSecurity,
    appBaseUrl: "https://app.paseo.sh",
    auth: options.auth,
    pushNotificationSender: options.pushNotificationSender,
    serviceProxy: options.serviceProxy,
    webUi: options.webUi,
    trustedProxies: options.trustedProxies,
    openai: options.openai,
    speech: options.speech
      ? {
          ...options.speech,
          localModelsDir:
            options.speech.localModelsDir ??
            options.speech.local?.modelsDir ??
            path.join(paseoHome, "models", "local-speech"),
        }
      : undefined,
    voiceLlmProvider: options.voiceLlmProvider ?? null,
    voiceLlmProviderExplicit: options.voiceLlmProviderExplicit ?? false,
    voiceLlmModel: options.voiceLlmModel ?? null,
    dictationFinalTimeoutMs: options.dictationFinalTimeoutMs,
    downloadTokenTtlMs: options.downloadTokenTtlMs,
    agentProfiles: options.agentProfiles,
    autoArchiveAfterMerge: options.autoArchiveAfterMerge,
    resumeOnStartup: options.resumeOnStartup,
    resumePrompt: options.resumePrompt,
    pluginsEnabled: options.pluginsEnabled,
    plugins: options.plugins,
  };
  return { config, paseoHomeRoot, paseoHome, staticDir, createdDirs };
}

function isAddressInUseError(error: unknown): boolean {
  if (!error || typeof error !== "object") {
    return false;
  }
  const record = error as { code?: string };
  return record.code === "EADDRINUSE";
}

function isStartupTimeoutError(error: unknown): boolean {
  if (!error || typeof error !== "object") {
    return false;
  }
  const record = error as { code?: string };
  return record.code === "TEST_DAEMON_START_TIMEOUT";
}
