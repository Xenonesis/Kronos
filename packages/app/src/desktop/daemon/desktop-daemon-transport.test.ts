import { describe, expect, it, vi } from "vitest";
import {
  buildDesktopDaemonTransportUrl,
  createDesktopDaemonTransportFactory,
} from "./desktop-daemon-transport";
import { createFakeLocalDaemonTransportRpc } from "./test-local-daemon-transport-rpc";

const LOCAL_URL = "paseo+desktop://socket?path=%2Ftmp%2Fpaseo.sock";

async function flushDelivery() {
  for (let i = 0; i < 16; i++) await Promise.resolve();
}

async function embeddedFixture() {
  const rpc = createFakeLocalDaemonTransportRpc();
  const cleanup = vi.fn();
  const transport = createDesktopDaemonTransportFactory(rpc)!({ url: "paseo+desktop://embedded" });
  rpc.resolveListen(cleanup);
  await Promise.resolve();
  const { sessionId, generation } = rpc.openCalls[0]!;
  expect(generation).toEqual(expect.any(String));
  expect(rpc.scopes).toEqual([{ sessionId, generation }]);
  rpc.emitEvent({ sessionId, generation, sequence: 1, kind: "open" });
  rpc.resolveRegistration();
  await Promise.resolve();
  return { rpc, transport, cleanup, sessionId, generation: generation! };
}

describe("desktop-daemon-transport", () => {
  it("uses the main-process event as readiness when it races registration", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const cleanup = vi.fn();
    const transportFactory = createDesktopDaemonTransportFactory(rpc);
    expect(transportFactory).not.toBeNull();

    const transport = transportFactory!({ url: LOCAL_URL });

    const onOpen = vi.fn();
    transport.onOpen(onOpen);

    rpc.resolveListen(cleanup);
    await Promise.resolve();

    const sessionId = rpc.openCalls[0]?.sessionId ?? "";
    expect(sessionId).not.toBe("");
    rpc.emitEvent({ sessionId, kind: "open" });
    expect(onOpen).toHaveBeenCalledTimes(1);

    rpc.resolveRegistration();
    await Promise.resolve();

    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it("does not start a session when listener setup finishes after close", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const cleanup = vi.fn();

    const transportFactory = createDesktopDaemonTransportFactory(rpc);
    expect(transportFactory).not.toBeNull();

    const transport = transportFactory!({ url: LOCAL_URL });

    transport.close();

    rpc.resolveListen(cleanup);
    await Promise.resolve();
    await Promise.resolve();

    expect(rpc.openCalls).toHaveLength(0);
    expect(rpc.closedSessions).toHaveLength(1);
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it("cancels a registered session while readiness is pending", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const transportFactory = createDesktopDaemonTransportFactory(rpc);
    expect(transportFactory).not.toBeNull();

    const transport = transportFactory!({ url: LOCAL_URL });
    rpc.resolveListen(vi.fn());
    await Promise.resolve();

    const sessionId = rpc.openCalls[0]?.sessionId ?? "";
    expect(sessionId).not.toBe("");

    transport.close();

    expect(rpc.closedSessions).toEqual([sessionId]);
  });

  it("keeps a session closed when registration resolves after close", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const cleanup = vi.fn();
    const transport = createDesktopDaemonTransportFactory(rpc)!({ url: LOCAL_URL });
    const onOpen = vi.fn();
    transport.onOpen(onOpen);
    rpc.resolveListen(cleanup);
    await Promise.resolve();

    expect(rpc.openCalls).toHaveLength(1);
    const sessionId = rpc.openCalls[0]!.sessionId;
    transport.close();
    rpc.resolveRegistration();
    await Promise.resolve();
    transport.close();

    expect(rpc.closedSessions).toEqual([sessionId]);
    expect(cleanup).toHaveBeenCalledTimes(1);
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("ignores a late open event after close", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const cleanup = vi.fn();
    const transport = createDesktopDaemonTransportFactory(rpc)!({ url: LOCAL_URL });
    const onOpen = vi.fn();
    const onMessage = vi.fn();
    const onError = vi.fn();
    transport.onOpen(onOpen);
    transport.onMessage(onMessage);
    transport.onError(onError);
    rpc.resolveListen(cleanup);
    await Promise.resolve();

    expect(rpc.openCalls).toHaveLength(1);
    const sessionId = rpc.openCalls[0]!.sessionId;
    transport.close();
    rpc.emitEvent({ sessionId, kind: "open" });
    rpc.emitEvent({ sessionId, kind: "message", text: "late message" });
    rpc.emitEvent({ sessionId, kind: "error", error: "late error" });
    rpc.resolveRegistration();
    await Promise.resolve();
    transport.send("must not send");

    expect(onOpen).not.toHaveBeenCalled();
    expect(onMessage).not.toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
    expect(rpc.recordedSends).toEqual([]);
    expect(rpc.closedSessions).toEqual([sessionId]);
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it("passes Remote SSH parameters to the desktop transport bridge", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const transportFactory = createDesktopDaemonTransportFactory(rpc);
    expect(transportFactory).not.toBeNull();

    const url = buildDesktopDaemonTransportUrl({
      transportType: "ssh",
      host: "deploy@example.com",
      sshPort: 2222,
      daemonPort: 7777,
    });
    transportFactory!({ url });
    rpc.resolveListen(vi.fn());
    await Promise.resolve();

    expect(rpc.openCalls).toHaveLength(1);
    expect(rpc.openCalls[0]?.target).toEqual({
      transportType: "ssh",
      host: "deploy@example.com",
      sshPort: 2222,
      daemonPort: 7777,
    });
  });

  it.each([0, 65536])("rejects an out-of-range Remote SSH port (%s)", (sshPort) => {
    const transportFactory = createDesktopDaemonTransportFactory(
      createFakeLocalDaemonTransportRpc(),
    );
    expect(transportFactory).not.toBeNull();

    const url = buildDesktopDaemonTransportUrl({
      transportType: "ssh",
      host: "deploy@example.com",
      sshPort,
    });

    expect(() => transportFactory!({ url })).toThrow("Invalid SSH transport target");
  });

  it("preserves order and bytes across text batches and binary delivery", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const transport = createDesktopDaemonTransportFactory(rpc)!({ url: LOCAL_URL });
    rpc.resolveListen(vi.fn());
    await Promise.resolve();

    expect(rpc.openCalls).toHaveLength(1);
    const sessionId = rpc.openCalls[0]!.sessionId;
    const received: Array<{ data: unknown; isBinary: boolean }> = [];
    transport.onMessage((data, isBinary) => {
      received.push({ data, isBinary });
    });
    rpc.emitEvent({ sessionId, kind: "open" });
    rpc.resolveRegistration();
    await Promise.resolve();

    rpc.emitEvent({ sessionId, kind: "message", messages: ["text-1", "text-2"] });
    rpc.emitEvent({ sessionId, kind: "message", binaryBase64: "AAH/gA==" });
    rpc.emitEvent({ sessionId, kind: "message", messages: ["text-3", "text-4"] });
    rpc.emitEvent({ sessionId, kind: "message", text: "text-5" });

    expect(received).toEqual([
      { data: "text-1", isBinary: false },
      { data: "text-2", isBinary: false },
      { data: new Uint8Array([0, 1, 255, 128]), isBinary: true },
      { data: "text-3", isBinary: false },
      { data: "text-4", isBinary: false },
      { data: "text-5", isBinary: false },
    ]);
    transport.close();
  });

  it("surfaces a rejected send through the transport error handler", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const transport = createDesktopDaemonTransportFactory(rpc)!({ url: LOCAL_URL });
    const onError = vi.fn();
    transport.onError(onError);
    rpc.resolveListen(vi.fn());
    await Promise.resolve();

    expect(rpc.openCalls).toHaveLength(1);
    const sessionId = rpc.openCalls[0]!.sessionId;
    rpc.emitEvent({ sessionId, kind: "open" });
    rpc.resolveRegistration();
    await Promise.resolve();
    const failure = new Error("owned test writer failed");
    rpc.failNextSend(failure);

    const sending = transport.send("request-1");
    expect(sending).toBeInstanceOf(Promise);
    await expect(sending).rejects.toBe(failure);

    expect(rpc.recordedSends).toEqual([{ sessionId, text: "request-1" }]);
    expect(onError).toHaveBeenCalledExactlyOnceWith(failure);
    transport.close();
  });

  it("supports an explicit embedded target without a path or SSH target", () => {
    expect(() =>
      createDesktopDaemonTransportFactory(createFakeLocalDaemonTransportRpc())!({
        url: "paseo+desktop://embedded",
      }),
    ).not.toThrow();
  });

  it("ignores stale embedded generations and duplicate callbacks", async () => {
    const { rpc, transport, sessionId, generation } = await embeddedFixture();
    const received: unknown[] = [];
    transport.onMessage((message) => {
      received.push(message);
    });
    rpc.emitEvent({ sessionId, generation: "old", sequence: 2, kind: "message", text: "stale" });
    rpc.emitEvent({ sessionId, generation, sequence: 2, kind: "message", text: "current" });
    rpc.emitEvent({ sessionId, generation, sequence: 2, kind: "message", text: "duplicate" });
    await flushDelivery();
    expect(received).toEqual(["current"]);
    expect(rpc.recordedAcks).toEqual([{ sessionId, generation, sequence: 2 }]);
    transport.close();
  });

  it("awaits embedded handlers before ordered binary delivery and cumulative acknowledgement", async () => {
    const { rpc, transport, sessionId, generation } = await embeddedFixture();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const received: unknown[] = [];
    transport.onMessage(async (message) => {
      received.push(message);
      if (message === "first") await gate;
    });
    rpc.emitEvent({ sessionId, generation, sequence: 2, kind: "message", text: "first" });
    rpc.emitEvent({
      sessionId,
      generation,
      sequence: 3,
      kind: "message",
      binaryBase64: "AAH/gA==",
    });
    await flushDelivery();
    expect(received).toEqual(["first"]);
    expect(rpc.recordedAcks).toEqual([]);
    release();
    await flushDelivery();
    expect(received).toEqual(["first", new Uint8Array([0, 1, 255, 128])]);
    expect(rpc.recordedAcks).toEqual([{ sessionId, generation, sequence: 3 }]);
    transport.close();
  });

  it("does not acknowledge embedded handler failure", async () => {
    const { rpc, transport, sessionId, generation } = await embeddedFixture();
    const errors: unknown[] = [];
    transport.onError((error) => {
      errors.push(error);
    });
    const failure = new Error("consumer failed");
    transport.onMessage(() => {
      throw failure;
    });
    rpc.emitEvent({ sessionId, generation, sequence: 2, kind: "message", text: "not consumed" });
    await flushDelivery();
    expect(errors).toEqual([failure]);
    expect(rpc.recordedAcks).toEqual([]);
    expect(rpc.closedSessions).toEqual([sessionId]);
  });

  it("delivers embedded terminal close once without waiting for a blocked consumer", async () => {
    const { rpc, transport, cleanup, sessionId, generation } = await embeddedFixture();
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const received: unknown[] = [];
    transport.onMessage(async (message) => {
      received.push(message);
      await gate;
    });
    const closed: unknown[] = [];
    transport.onClose((event) => {
      closed.push(event);
    });
    rpc.emitEvent({ sessionId, generation, sequence: 2, kind: "message", text: "in flight" });
    rpc.emitEvent({
      sessionId,
      generation,
      sequence: 3,
      kind: "message",
      text: "discarded on recovery",
    });
    const terminal = {
      sessionId,
      generation,
      sequence: 4,
      kind: "close" as const,
      code: 1013,
      reason: "resynchronize",
    };
    rpc.emitEvent(terminal);
    rpc.emitEvent(terminal);
    expect(closed).toEqual([terminal]);
    expect(cleanup).toHaveBeenCalledTimes(1);
    release();
    await flushDelivery();
    expect(received).toEqual(["in flight"]);
    expect(rpc.recordedAcks).toEqual([]);
  });

  it("rejects embedded sends after close without accepting a native enqueue", async () => {
    const { rpc, transport } = await embeddedFixture();
    transport.close();
    const sending = transport.send("must not enqueue");
    expect(sending).toBeInstanceOf(Promise);
    await expect(sending).rejects.toThrow("not open");
    expect(rpc.recordedSends).toEqual([]);
    expect(rpc.recordedAcks).toEqual([]);
  });

  it("requires embedded resynchronization on a sequence gap", async () => {
    const { rpc, transport, sessionId, generation } = await embeddedFixture();
    const closed: Array<{ code?: number }> = [];
    transport.onClose((event) => {
      closed.push(event as { code?: number });
    });
    rpc.emitEvent({ sessionId, generation, sequence: 3, kind: "message", text: "gap" });
    await flushDelivery();
    expect(closed).toEqual([expect.objectContaining({ code: 1013 })]);
    expect(rpc.recordedAcks).toEqual([]);
    expect(rpc.closedSessions).toEqual([sessionId]);
  });

  it("delivers batched messages to message handlers in order", async () => {
    const rpc = createFakeLocalDaemonTransportRpc();
    const transportFactory = createDesktopDaemonTransportFactory(rpc);
    const transport = transportFactory!({ url: LOCAL_URL });
    rpc.resolveListen(vi.fn());
    await Promise.resolve();

    const sessionId = rpc.openCalls[0]?.sessionId ?? "";
    const received: string[] = [];
    transport.onMessage((data) => {
      received.push(String(data));
    });

    rpc.emitEvent({
      sessionId,
      kind: "message",
      messages: ["chunk1", "chunk2", "chunk3"],
    });

    expect(received).toEqual(["chunk1", "chunk2", "chunk3"]);
  });
});
