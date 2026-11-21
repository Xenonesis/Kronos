import type {
  DaemonTransport,
  DaemonTransportFactory,
} from "@getpaseo/client/internal/daemon-client";
import { validatePort, validateSshHost } from "@getpaseo/protocol/ssh-transport";
import type { DesktopDaemonTransportTarget } from "./desktop-daemon";
import {
  defaultLocalDaemonTransportRpc,
  type LocalDaemonTransportEvent,
  type LocalDaemonTransportRpc,
} from "./local-daemon-transport-rpc";

const DESKTOP_TRANSPORT_SCHEME = "paseo+desktop:";

function encodeBinaryToBase64(data: Uint8Array | ArrayBuffer): string {
  const bytes = data instanceof ArrayBuffer ? new Uint8Array(data) : data;
  let binary = "";
  for (let index = 0; index < bytes.length; index += 1) {
    binary += String.fromCharCode(bytes[index]);
  }
  return globalThis.btoa(binary);
}

function decodeBase64ToBytes(base64: string): Uint8Array {
  const binary = globalThis.atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

export function buildDesktopDaemonTransportUrl(target: DesktopDaemonTransportTarget): string {
  const url = new URL(`${DESKTOP_TRANSPORT_SCHEME}//${target.transportType}`);
  if (target.transportType === "ssh") {
    url.searchParams.set("host", target.host);
    if (target.sshPort !== undefined) {
      url.searchParams.set("port", String(target.sshPort));
    }
    if (target.daemonPort !== undefined) {
      url.searchParams.set("daemonPort", String(target.daemonPort));
    }
  } else if (target.transportType !== "embedded") {
    url.searchParams.set("path", target.transportPath);
  }
  return url.toString();
}

function parseDesktopDaemonTransportUrl(url: string): DesktopDaemonTransportTarget {
  const parsed = new URL(url);
  if (parsed.protocol !== DESKTOP_TRANSPORT_SCHEME) {
    throw new Error(`Unsupported desktop transport URL: ${url}`);
  }
  const transportType = parsed.hostname;
  if (transportType === "embedded") {
    if (
      parsed.search ||
      parsed.username ||
      parsed.password ||
      parsed.port ||
      (parsed.pathname && parsed.pathname !== "/")
    ) {
      throw new Error("Embedded transport accepts no external target");
    }
    return { transportType };
  }
  if (transportType === "ssh") return parseSshDesktopTransportUrl(parsed, url);
  const transportPath = parsed.searchParams.get("path")?.trim() ?? "";
  if ((transportType !== "socket" && transportType !== "pipe") || !transportPath) {
    throw new Error(`Invalid desktop transport target: ${url}`);
  }
  return {
    transportType,
    transportPath,
  };
}

function parseSshDesktopTransportUrl(parsed: URL, rawUrl: string): DesktopDaemonTransportTarget {
  try {
    const host = validateSshHost(parsed.searchParams.get("host") ?? "");
    const sshPort = parseOptionalUrlPort(parsed, "port", "SSH port");
    const daemonPort = parseOptionalUrlPort(parsed, "daemonPort", "Daemon port");
    return {
      transportType: "ssh",
      host,
      ...(sshPort !== undefined ? { sshPort } : {}),
      ...(daemonPort !== undefined ? { daemonPort } : {}),
    };
  } catch (error) {
    throw new Error(`Invalid SSH transport target: ${rawUrl}`, { cause: error });
  }
}

function parseOptionalUrlPort(parsed: URL, key: string, label: string): number | undefined {
  const value = parsed.searchParams.get(key);
  return value === null ? undefined : validatePort(value, label);
}

export function createDesktopDaemonTransportFactory(
  rpc: LocalDaemonTransportRpc = defaultLocalDaemonTransportRpc,
): DaemonTransportFactory | null {
  return ({ url }) => {
    const target = parseDesktopDaemonTransportUrl(url);
    const sessionId = `local-session-${globalThis.crypto.randomUUID()}`;
    const embedded = target.transportType === "embedded";
    const generation = embedded ? globalThis.crypto.randomUUID() : undefined;
    const identity = { sessionId, ...(generation ? { generation } : {}) };
    let unlisten: (() => void) | null = null;
    let disposed = false;
    let didEmitOpen = false;

    const openHandlers = new Set<() => void>();
    const closeHandlers = new Set<(event?: unknown) => void>();
    const errorHandlers = new Set<(event?: unknown) => void>();
    const messageHandlers = new Set<(data: unknown, isBinary: boolean) => void>();

    const emitOpen = () => {
      if (didEmitOpen || disposed) {
        return;
      }
      didEmitOpen = true;
      for (const handler of openHandlers) {
        handler();
      }
    };
    const emitClose = (event?: unknown) => {
      for (const handler of closeHandlers) {
        handler(event);
      }
    };
    const emitError = (event?: unknown) => {
      if (disposed) {
        return;
      }
      for (const handler of errorHandlers) {
        handler(event);
      }
    };
    const emitMessage = (data: unknown, isBinary: boolean) => {
      for (const handler of messageHandlers) {
        handler(data, isBinary);
      }
    };

    let lastReceivedSequence = 0;
    let processing = false;
    const pending: Array<{ event: LocalDaemonTransportEvent; bytes: number }> = [];
    let pendingFrames = 0;
    let pendingBytes = 0;

    const dispose = () => {
      disposed = true;
      pending.length = 0;
      pendingFrames = 0;
      pendingBytes = 0;
      unlisten?.();
      unlisten = null;
    };
    const closeRpcSession = async () => {
      try {
        await rpc.closeSession(sessionId, generation);
      } catch {
        // Native teardown may already have removed this generation.
      }
    };
    const consumeEmbeddedMessage = async (event: LocalDaemonTransportEvent) => {
      const hasText = typeof event.text === "string";
      const hasBinary = typeof event.binaryBase64 === "string";
      if (hasText === hasBinary) throw new Error("Invalid embedded message payload");
      const data = hasText ? event.text! : decodeBase64ToBytes(event.binaryBase64!);
      for (const handler of messageHandlers) {
        await handler(data, hasBinary);
        if (disposed) return false;
      }
      return !disposed;
    };
    const failEmbedded = (error: unknown) => {
      if (disposed) return;
      emitError(error);
      dispose();
      emitClose({ ...identity, kind: "close", code: 1013, reason: "Reconnect and resynchronize" });
      void closeRpcSession();
    };
    const drainEmbedded = async () => {
      if (processing) return;
      processing = true;
      try {
        while (pending.length) {
          if (disposed) return;
          let acknowledged = 0;
          while (pending.length) {
            const { event, bytes } = pending.shift()!;
            if (!(await consumeEmbeddedMessage(event))) return;
            pendingFrames--;
            pendingBytes -= bytes;
            acknowledged = event.sequence!;
          }
          if (!disposed && acknowledged) {
            await rpc.ackMessage({ sessionId, generation: generation!, sequence: acknowledged });
          }
        }
      } catch (error) {
        failEmbedded(error);
      } finally {
        processing = false;
      }
    };
    const handleEmbeddedEvent = (payload: LocalDaemonTransportEvent) => {
      if (payload.generation !== generation) return;
      const sequence = payload.sequence;
      if (!Number.isSafeInteger(sequence) || sequence! < 1) {
        failEmbedded(new Error("Invalid embedded event sequence"));
        return;
      }
      if (sequence! <= lastReceivedSequence) return;
      if (payload.kind === "close") {
        dispose();
        emitClose(payload);
        return;
      }
      if (sequence !== lastReceivedSequence + 1) {
        failEmbedded(new Error("Embedded event sequence gap"));
        return;
      }
      lastReceivedSequence = sequence;
      if (payload.kind === "open") {
        emitOpen();
        return;
      }
      if (payload.kind !== "message" || !didEmitOpen) {
        failEmbedded(new Error(payload.error ?? "Unexpected embedded event"));
        return;
      }
      const bytes = new TextEncoder().encode(JSON.stringify(payload)).byteLength;
      if (pendingFrames >= 64 || bytes > 32 * 1024 * 1024 - pendingBytes) {
        failEmbedded(new Error("Embedded delivery overflow"));
        return;
      }
      pendingFrames++;
      pendingBytes += bytes;
      pending.push({ event: payload, bytes });
      void drainEmbedded();
    };

    const handleEvent = (payload: LocalDaemonTransportEvent) => {
      if (disposed || payload.sessionId !== sessionId) {
        return;
      }
      if (embedded) {
        handleEmbeddedEvent(payload);
        return;
      }
      if (payload.kind === "open") {
        emitOpen();
        return;
      }
      if (payload.kind === "message") {
        if (payload.messages && payload.messages.length > 0) {
          for (const msg of payload.messages) {
            if (msg) emitMessage(msg, false);
          }
          return;
        }
        if (payload.text) {
          emitMessage(payload.text, false);
          return;
        }
        if (payload.binaryBase64) {
          emitMessage(decodeBase64ToBytes(payload.binaryBase64), true);
        }
        return;
      }
      if (payload.kind === "close") {
        emitClose(payload);
        return;
      }
      emitError(payload.error ?? "Local daemon transport error");
    };

    void (async () => {
      try {
        const cleanup = await rpc.listenToEvents(
          handleEvent,
          generation ? { sessionId, generation } : undefined,
        );
        if (disposed) {
          cleanup();
          return;
        }
        unlisten = cleanup;

        await rpc.openSession({ ...identity, target });
      } catch (error) {
        if (embedded) failEmbedded(error);
        else emitError(error);
      }
    })();

    const transport: DaemonTransport = {
      ...(embedded ? { maxFrameBytes: 16 * 1024 * 1024 } : {}),
      send: (data) => {
        if (disposed || !didEmitOpen) {
          if (embedded) return Promise.reject(new Error("Embedded session is not open"));
          return;
        }
        if (typeof data === "string") {
          return rpc.sendMessage({ ...identity, text: data }).catch((error) => {
            emitError(error);
            throw error;
          });
        }
        const binaryBase64 = encodeBinaryToBase64(
          data instanceof ArrayBuffer ? data : new Uint8Array(data),
        );
        return rpc.sendMessage({ ...identity, binaryBase64 }).catch((error) => {
          emitError(error);
          throw error;
        });
      },
      close: () => {
        if (disposed) {
          return;
        }
        dispose();
        void closeRpcSession();
      },
      onMessage: (handler) => {
        messageHandlers.add(handler);
        return () => messageHandlers.delete(handler);
      },
      onOpen: (handler) => {
        openHandlers.add(handler);
        return () => openHandlers.delete(handler);
      },
      onClose: (handler) => {
        closeHandlers.add(handler);
        return () => closeHandlers.delete(handler);
      },
      onError: (handler) => {
        errorHandlers.add(handler);
        return () => errorHandlers.delete(handler);
      },
    };

    return transport;
  };
}
