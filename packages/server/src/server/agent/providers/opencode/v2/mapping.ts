import type { AgentInfo, ModelInfo, ModelRef, SessionInfo } from "@opencode/client";
import type { AgentMode, AgentModelDefinition, AgentUsage } from "../../../agent-sdk-types.js";

export function modelRef(id: string, variant?: string | null): ModelRef {
  const slash = id.indexOf("/");
  if (slash < 1 || slash === id.length - 1)
    throw new Error("OpenCode model must be provider/model");
  const normVariant = variant && variant !== "default" ? variant : undefined;
  return {
    providerID: id.slice(0, slash),
    id: id.slice(slash + 1),
    ...(normVariant ? { variant: normVariant } : {}),
  };
}

const DEFAULT_OPENCODE_THINKING_OPTIONS = [
  { id: "default", label: "Default", isDefault: true },
  { id: "low", label: "Low" },
  { id: "medium", label: "Medium" },
  { id: "high", label: "High" },
  { id: "max", label: "Max" },
];

export function modelsFromV2(models: ModelInfo[]): AgentModelDefinition[] {
  return models
    .filter((model) => model.enabled)
    .map((model) => {
      const thinkingOptions =
        model.variants && model.variants.length > 0
          ? [
              { id: "default", label: "Default", isDefault: true },
              ...model.variants
                .filter((v) => v.id !== "default")
                .map((v) => ({
                  id: v.id,
                  label:
                    v.id === "low"
                      ? "Low"
                      : v.id === "medium"
                        ? "Medium"
                        : v.id === "high"
                          ? "High"
                          : v.id === "max"
                            ? "Max"
                            : v.id,
                })),
            ]
          : DEFAULT_OPENCODE_THINKING_OPTIONS;

      return {
        provider: "opencode",
        id: `${model.providerID}/${model.id}`,
        label: model.name,
        contextWindowMaxTokens: model.limit.context,
        defaultThinkingOptionId: "default",
        metadata: {
          providerId: model.providerID,
          modelId: model.id,
          supportsAttachments: model.capabilities.input.includes("image"),
          supportsToolCall: model.capabilities.tools,
          contextWindowMaxTokens: model.limit.context,
        },
        thinkingOptions,
      };
    });
}

export function modesFromV2(agents: AgentInfo[]): AgentMode[] {
  return agents
    .filter((agent) => !agent.hidden && agent.mode !== "subagent")
    .map((agent) => ({ id: agent.id, label: agent.name, description: agent.description }));
}

export function usageFromV2(session: SessionInfo): AgentUsage {
  return {
    inputTokens: session.tokens.input,
    outputTokens: session.tokens.output,
    cachedInputTokens: session.tokens.cache.read,
    totalCostUsd: session.cost,
  };
}
