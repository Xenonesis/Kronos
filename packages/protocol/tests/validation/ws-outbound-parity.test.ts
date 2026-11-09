import { describe, expect, it } from "vitest";
import {
  isWSOutboundAotLoaded,
  loadWSOutboundAotValidators,
  validateWSOutboundMessage,
} from "../../src/validation/ws-outbound.js";

describe("WS Outbound validation parity (Zod fallback vs AOT)", () => {
  const sampleMessages = [
    {
      name: "pong message",
      envelope: { type: "pong" },
      isValid: true,
    },
    {
      name: "corrupted envelope",
      envelope: { type: "unknown_envelope_type" },
      isValid: false,
    },
    {
      name: "read_project_config_response with setup changes",
      envelope: {
        type: "session",
        message: {
          type: "read_project_config_response",
          payload: {
            requestId: "r1",
            repoRoot: "/repo",
            ok: true,
            config: null,
            revision: null,
            hasUncommittedWorktreeSetupChanges: true,
          },
        },
      },
      isValid: true,
    },
    {
      name: "get_providers_snapshot_response compact",
      envelope: {
        type: "session",
        message: {
          type: "get_providers_snapshot_response",
          payload: {
            entries: [],
            compactSnapshot: {
              entries: [
                {
                  provider: "pi",
                  status: "ready",
                  enabled: true,
                  models: [{ id: "model-a", label: "Model A", thinkingSet: 0 }],
                },
              ],
              thinkingSets: [
                {
                  options: [{ id: "high", label: "High", isDefault: true }],
                  defaultOptionId: "high",
                },
              ],
            },
            snapshotHash: "snapshot-hash",
            generatedAt: "2026-08-04T00:00:00.000Z",
            requestId: "provider-snapshot",
          },
        },
      },
      isValid: true,
    },
    {
      name: "agent_update with defaults applied",
      envelope: {
        type: "session",
        message: {
          type: "agent_update",
          payload: {
            kind: "upsert",
            agent: {
              id: "agent-1",
              provider: "claude",
              cwd: "/repo",
              model: "model",
              createdAt: "2026-01-01T00:00:00.000Z",
              updatedAt: "2026-01-01T00:00:00.000Z",
              lastUserMessageAt: "2026-01-01T00:00:00.000Z",
              status: "initializing",
              capabilities: {
                supportsStreaming: false,
                supportsSessionPersistence: false,
                supportsDynamicModes: false,
                supportsMcpServers: false,
                supportsReasoningStream: false,
                supportsToolInvocations: false,
              },
              currentModeId: "default",
              availableModes: [],
              pendingPermissions: [],
              persistence: { provider: "claude", sessionId: "session-1" },
              title: "Agent",
            },
          },
        },
      },
      isValid: true,
      checkDefaults: (res: any) => {
        expect(res.data.message.payload.agent.capabilities.supportsRewindConversation).toBe(false);
      },
    },
  ];

  it("validates accurately in both Zod and AOT modes", async () => {
    // 1. Test before AOT is loaded (or test if currently loaded)
    const initialModeIsAot = isWSOutboundAotLoaded();

    for (const sample of sampleMessages) {
      const res = validateWSOutboundMessage(sample.envelope);
      expect(res.success).toBe(sample.isValid);
      if (res.success && sample.checkDefaults) {
        sample.checkDefaults(res);
      }
    }

    // 2. Load AOT explicitly
    await loadWSOutboundAotValidators();
    expect(isWSOutboundAotLoaded()).toBe(true);

    // 3. Test after AOT is loaded
    for (const sample of sampleMessages) {
      const res = validateWSOutboundMessage(sample.envelope);
      expect(res.success).toBe(sample.isValid);
      if (res.success && sample.checkDefaults) {
        sample.checkDefaults(res);
      }
    }
  }, 20000);
});
