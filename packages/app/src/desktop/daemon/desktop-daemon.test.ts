import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/i18n/i18next", () => ({ i18n: { t: (key: string) => key } }));
vi.mock("@/utils/confirm-dialog", () => ({ confirmDialog: vi.fn() }));
vi.mock("@/desktop/electron/invoke", () => ({ invokeDesktopCommand: vi.fn() }));
vi.mock("@/constants/platform", () => ({ isWeb: true }));

import { invokeDesktopCommand } from "@/desktop/electron/invoke";
import {
  getDesktopDaemonStatus,
  getManagedDesktopTransport,
  listenToLocalTransportEvents,
  openLocalTransportSession,
  sendLocalTransportMessage,
  acknowledgeLocalTransportMessage,
  closeLocalTransportSession,
} from "./desktop-daemon";

const legacy = {
  serverId: "managed-local",
  status: "running",
  listen: "127.0.0.1:19104",
  hostname: "Local (This Device)",
  pid: 42,
  home: "/isolated-home",
  version: "0.16.0",
  desktopManaged: true,
  ownedByDesktop: true,
  startedAt: "2026-11-17T00:00:00Z",
  error: null,
};

describe("desktop daemon readiness status", () => {
  beforeEach(() => {
    vi.mocked(invokeDesktopCommand).mockReset();
  });

  it("retains native managed transport capability independently of TCP readiness", async () => {
    vi.mocked(invokeDesktopCommand).mockResolvedValue({
      ...legacy,
      engine: "rust",
      enginePhase: { phase: "ready" },
      listen: null,
      networkAvailable: false,
      localTransport: "ipc",
    });
    expect(await getDesktopDaemonStatus()).toMatchObject({
      engine: "rust",
      localTransport: "ipc",
      listen: null,
    });
  });

  it("rejects an unknown native transport setting instead of downgrading", async () => {
    vi.mocked(invokeDesktopCommand).mockResolvedValue({ ...legacy, localTransport: "mystery" });
    await expect(getDesktopDaemonStatus()).rejects.toThrow("Unknown desktop local transport");
  });

  it("revokes cached IPC authority when native status refresh fails", async () => {
    vi.mocked(invokeDesktopCommand).mockResolvedValue({
      ...legacy,
      engine: "rust",
      enginePhase: { phase: "ready" },
      localTransport: "ipc",
    });
    await getDesktopDaemonStatus();
    vi.mocked(invokeDesktopCommand).mockRejectedValue(new Error("native caller revoked"));
    await expect(getDesktopDaemonStatus()).rejects.toThrow("native caller revoked");
    expect(() =>
      getManagedDesktopTransport(
        {
          id: "socket:ipc://embedded",
          type: "directSocket",
          path: "ipc://embedded",
        },
        "managed-local",
      ),
    ).toThrow("unavailable");
  });

  it("keeps verified authority during a pending refresh without interrupting hello", async () => {
    const status = {
      ...legacy,
      engine: "rust",
      enginePhase: { phase: "ready" },
      localTransport: "ipc",
    };
    vi.mocked(invokeDesktopCommand).mockResolvedValue(status);
    await getDesktopDaemonStatus();
    let finish!: (value: unknown) => void;
    vi.mocked(invokeDesktopCommand).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const pending = getDesktopDaemonStatus();
    try {
      expect(
        getManagedDesktopTransport(
          { id: "socket:ipc://embedded", type: "directSocket", path: "ipc://embedded" },
          "managed-local",
        ),
      ).toEqual({ transportType: "embedded" });
    } finally {
      finish(status);
      await pending;
    }
  });

  it("preserves core readiness when listener is unavailable", async () => {
    vi.mocked(invokeDesktopCommand).mockResolvedValue({
      ...legacy,
      status: "errored",
      listen: null,
      error: "listener failed",
      engine: "rust",
      enginePhase: { phase: "ready" },
      networkAvailable: false,
      networkError: "listener failed",
    });
    expect(await getDesktopDaemonStatus()).toMatchObject({
      status: "errored",
      listen: null,
      engine: "rust",
      enginePhase: { phase: "ready" },
      networkAvailable: false,
      networkError: "listener failed",
    });
  });

  it("preserves legacy status without inventing engine ownership", async () => {
    vi.mocked(invokeDesktopCommand).mockResolvedValue(legacy);
    expect(await getDesktopDaemonStatus()).toStrictEqual(legacy);
  });

  it("retains failed core reason separately from network status", async () => {
    vi.mocked(invokeDesktopCommand).mockResolvedValue({
      ...legacy,
      status: "errored",
      listen: null,
      engine: "rust",
      enginePhase: { phase: "failed", reason: "storage unavailable" },
      networkAvailable: false,
      networkError: null,
    });
    expect(await getDesktopDaemonStatus()).toMatchObject({
      enginePhase: { phase: "failed", reason: "storage unavailable" },
      networkAvailable: false,
      networkError: null,
    });
  });

  it("does not promote malformed readiness metadata", async () => {
    vi.mocked(invokeDesktopCommand).mockResolvedValue({
      ...legacy,
      engine: "other",
      enginePhase: { phase: "failed" },
      networkAvailable: "true",
      networkError: 42,
    });
    expect(await getDesktopDaemonStatus()).toStrictEqual(legacy);
  });

  afterEach(() => vi.unstubAllGlobals());

  it("binds embedded commands to their registered channel and preserves empty payloads", async () => {
    type Handler = Parameters<typeof listenToLocalTransportEvents>[0];
    const channels: FakeChannel[] = [];
    class FakeChannel {
      onmessage: Handler = () => {};
      constructor() {
        channels.push(this);
      }
    }
    vi.stubGlobal("window", { __TAURI__: { core: { Channel: FakeChannel } } });
    const received: string[] = [];
    const scope = { sessionId: "owned", generation: "current" };
    const cleanup = await listenToLocalTransportEvents((event) => {
      received.push(event.sessionId);
    }, scope);
    const otherCleanup = await listenToLocalTransportEvents(
      () => {
        received.push("other");
      },
      { sessionId: "other", generation: "two" },
    );
    try {
      await expect(
        openLocalTransportSession({
          ...scope,
          generation: "old",
          target: { transportType: "embedded" },
        }),
      ).rejects.toThrow("not registered");
      await openLocalTransportSession({ ...scope, target: { transportType: "embedded" } });
      await sendLocalTransportMessage({ ...scope, text: "" });
      await acknowledgeLocalTransportMessage({ ...scope, sequence: 2 });
      await closeLocalTransportSession(scope.sessionId, scope.generation);
      expect(vi.mocked(invokeDesktopCommand).mock.calls).toEqual([
        ["open_embedded_transport", { input: scope, onEvent: channels[0] }],
        ["send_embedded_transport", { input: { ...scope, text: "" } }],
        ["ack_embedded_transport", { input: { ...scope, sequence: 2 } }],
        ["close_embedded_transport", { input: scope }],
      ]);
      channels[0]!.onmessage({ ...scope, sequence: 1, kind: "open" });
      expect(received).toEqual(["owned"]);
      cleanup();
      channels[0]!.onmessage({ ...scope, sequence: 2, kind: "message", text: "late" });
      channels[1]!.onmessage({ sessionId: "other", generation: "two", sequence: 1, kind: "open" });
      expect(received).toEqual(["owned", "other"]);
      await expect(
        openLocalTransportSession({ ...scope, target: { transportType: "embedded" } }),
      ).rejects.toThrow("not registered");
    } finally {
      cleanup();
      otherCleanup();
    }
  });

  it("keeps Node engine status distinct", async () => {
    vi.mocked(invokeDesktopCommand).mockResolvedValue({
      ...legacy,
      engine: "node",
      enginePhase: null,
      networkAvailable: true,
      networkError: null,
    });
    expect(await getDesktopDaemonStatus()).toStrictEqual({
      ...legacy,
      engine: "node",
      networkAvailable: true,
      networkError: null,
    });
  });
});
