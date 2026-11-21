import type { Logger } from "pino";
import type {
  AgentCapabilityFlags,
  AgentClient,
  AgentLaunchContext,
  AgentModelDefinition,
  AgentPersistenceHandle,
  AgentProvider,
  AgentSession,
  AgentSessionConfig,
  FetchCatalogOptions,
  ProviderCatalog,
  ProviderRefreshContext,
} from "../agent-sdk-types.js";
import { getAgentProviderDefinition } from "@getpaseo/protocol/provider-manifest";
import type { ProviderRuntimeSettings } from "../provider-launch-config.js";
import { discoverOllamaModels } from "./ollama-discovery.js";

const DEFAULT_LOCAL_CAPABILITIES: AgentCapabilityFlags = {
  supportsStreaming: true,
  supportsSessionPersistence: false,
  supportsDynamicModes: false,
  supportsMcpServers: false,
  supportsReasoningStream: false,
  supportsToolInvocations: false,
  supportsRewindConversation: false,
  supportsRewindFiles: false,
  supportsRewindBoth: false,
};

export interface LocalAIAgentClientOptions {
  provider: AgentProvider;
  logger: Logger;
  runtimeSettings?: ProviderRuntimeSettings;
}

interface LMStudioModelsResponse {
  data?: Array<{ id: string }>;
}

export class LocalAIAgentClient implements AgentClient {
  readonly provider: AgentProvider;
  readonly capabilities: AgentCapabilityFlags = DEFAULT_LOCAL_CAPABILITIES;
  private readonly logger: Logger;
  private readonly runtimeSettings?: ProviderRuntimeSettings;

  constructor(options: LocalAIAgentClientOptions) {
    this.provider = options.provider;
    this.logger = options.logger.child({ provider: options.provider });
    this.runtimeSettings = options.runtimeSettings;
  }

  private getEndpoint(): string {
    if (this.provider === "ollama") {
      return (
        this.runtimeSettings?.env?.OLLAMA_HOST ||
        process.env.OLLAMA_HOST ||
        "http://127.0.0.1:11434"
      );
    }
    if (this.provider === "lmstudio") {
      return (
        this.runtimeSettings?.env?.LMSTUDIO_HOST ||
        process.env.LMSTUDIO_HOST ||
        "http://127.0.0.1:1234"
      );
    }
    return "http://127.0.0.1:8000";
  }

  async isAvailable(): Promise<boolean> {
    const endpoint = this.getEndpoint().replace(/\/$/, "");
    const probeUrl = this.provider === "ollama" ? `${endpoint}/api/tags` : `${endpoint}/v1/models`;

    try {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), 1200);
      const res = await fetch(probeUrl, {
        method: "GET",
        signal: controller.signal,
        headers: { Accept: "application/json" },
      });
      clearTimeout(timer);
      return res.ok;
    } catch {
      return false;
    }
  }

  async fetchCatalog(
    _options: FetchCatalogOptions,
    context?: ProviderRefreshContext,
  ): Promise<ProviderCatalog> {
    const definition = getAgentProviderDefinition(this.provider);
    const modes = definition?.modes ?? [];
    const defaultModeId = definition?.defaultModeId ?? modes[0]?.id ?? null;
    const endpoint = this.getEndpoint().replace(/\/$/, "");

    try {
      if (this.provider === "ollama") {
        const discovered = await discoverOllamaModels(endpoint, 1500);
        const models: AgentModelDefinition[] = discovered.map((m, index) => ({
          provider: this.provider,
          id: m.id,
          label: m.label,
          description: m.description,
          isDefault: index === 0,
          isSelectable: true,
        }));
        return { models, modes, defaultModeId };
      }

      if (this.provider === "lmstudio") {
        const controller = new AbortController();
        const signal = context?.signal ?? controller.signal;
        const timer = setTimeout(() => controller.abort(), 1500);

        try {
          const res = await fetch(`${endpoint}/v1/models`, {
            method: "GET",
            signal,
            headers: { Accept: "application/json" },
          });
          clearTimeout(timer);

          if (!res.ok) {
            return { models: [], modes, defaultModeId };
          }

          const body = (await res.json()) as LMStudioModelsResponse;
          const rawList = Array.isArray(body?.data) ? body.data : [];
          const models: AgentModelDefinition[] = rawList.map((item, index) => ({
            provider: this.provider,
            id: `lmstudio/${item.id}`,
            label: item.id,
            description: `Local LM Studio model: ${item.id}`,
            isDefault: index === 0,
            isSelectable: true,
          }));

          return { models, modes, defaultModeId };
        } catch {
          clearTimeout(timer);
          return { models: [], modes, defaultModeId };
        }
      }

      return { models: [], modes, defaultModeId };
    } catch (err) {
      this.logger.debug({ err }, "Local AI catalog fetch error");
      return { models: [], modes, defaultModeId };
    }
  }

  async getDiagnostic(): Promise<{ diagnostic: string }> {
    const endpoint = this.getEndpoint();
    const available = await this.isAvailable();
    return {
      diagnostic: available
        ? `${this.provider} server is connected and responding at ${endpoint}`
        : `${this.provider} server is not responding at ${endpoint}. Start the local daemon to use this provider.`,
    };
  }

  createSession(
    _config: AgentSessionConfig,
    _launchContext?: AgentLaunchContext,
  ): Promise<AgentSession> {
    const endpoint = this.getEndpoint();
    throw new Error(
      `Direct session execution for local provider '${this.provider}' requires local server running at ${endpoint}.`,
    );
  }

  resumeSession(
    _handle: AgentPersistenceHandle,
    _overrides?: Partial<AgentSessionConfig>,
    _launchContext?: AgentLaunchContext,
  ): Promise<AgentSession> {
    throw new Error(`Session resumption is not supported for '${this.provider}'.`);
  }
}
