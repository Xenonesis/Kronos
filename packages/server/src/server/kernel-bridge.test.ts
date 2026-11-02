import { describe, expect, it, afterEach } from "vitest";
import { WebSocketServer } from "ws";
import { KernelBridge } from "./kernel-bridge.js";

function setupEchoServer(wss: WebSocketServer): void {
  wss.on("connection", (socket) => {
    socket.on("message", (raw) => {
      const envelope = JSON.parse(raw.toString());
      socket.send(
        JSON.stringify({
          id: envelope.id,
          type: envelope.type,
          success: true,
          data: { echo: envelope.payload },
        }),
      );
    });
  });
}

describe("KernelBridge", () => {
  let wss: WebSocketServer | null = null;
  let bridge: KernelBridge | null = null;

  afterEach(() => {
    if (bridge) {
      bridge.close();
      bridge = null;
    }
    if (wss) {
      wss.close();
      wss = null;
    }
  });

  it("shouldDelegate respects prefix whitelist and enabled flag", () => {
    const disabledBridge = new KernelBridge({ enabled: false });
    expect(disabledBridge.shouldDelegate("ast/analyze")).toBe(false);

    const enabledBridge = new KernelBridge({ enabled: true });
    expect(enabledBridge.shouldDelegate("ast/analyze")).toBe(true);
    expect(enabledBridge.shouldDelegate("crdt/sync")).toBe(true);
    expect(enabledBridge.shouldDelegate("ioring/status")).toBe(true);
    expect(enabledBridge.shouldDelegate("shm/status")).toBe(true);
    expect(enabledBridge.shouldDelegate("speech/transcribe")).toBe(true);
    expect(enabledBridge.shouldDelegate("telemetry/psapi")).toBe(true);
    expect(enabledBridge.shouldDelegate("search/query")).toBe(true);
    expect(enabledBridge.shouldDelegate("vfs/read")).toBe(true);
    expect(enabledBridge.shouldDelegate("git/status")).toBe(true);
    expect(enabledBridge.shouldDelegate("unrelated/message")).toBe(false);
  });

  it("connects and roundtrips requests to mock kernel websocket", async () => {
    const port = 6793;
    wss = new WebSocketServer({ port });

    setupEchoServer(wss);

    bridge = new KernelBridge({
      url: `ws://127.0.0.1:${port}`,
      enabled: true,
    });

    await bridge.connect();
    interface EchoData {
      echo: { code: string };
    }
    const res = await bridge.dispatch<EchoData>("req-1", "ast/analyze", { code: "let x = 1;" });
    expect(res.id).toBe("req-1");
    expect(res.type).toBe("ast/analyze");
    expect(res.success).toBe(true);
    expect(res.data?.echo.code).toBe("let x = 1;");
  });
});
