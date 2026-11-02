import { WebSocket } from "ws";
import type { Logger } from "pino";

export interface KernelBridgeOptions {
  url?: string;
  logger?: Logger;
  enabled?: boolean;
}

export interface KernelBridgeResponse<T = unknown> {
  id: string;
  type: string;
  success: boolean;
  data?: T;
  error?: string;
}

const KERNEL_DELEGATED_PREFIXES: readonly string[] = [
  "ast/",
  "crdt/",
  "ioring/",
  "shm/",
  "speech/",
  "telemetry/",
  "search/",
  "vfs/",
  "git/",
];

export class KernelBridge {
  private readonly url: string;
  private readonly logger?: Logger;
  private readonly enabled: boolean;
  private ws: WebSocket | null = null;
  private pendingRequests = new Map<
    string,
    {
      resolve: (value: KernelBridgeResponse) => void;
      reject: (reason: Error) => void;
      timer: NodeJS.Timeout;
    }
  >();

  constructor(options: KernelBridgeOptions = {}) {
    this.url = options.url ?? process.env.KRONOS_KERNEL_IPC_URL ?? "ws://127.0.0.1:6768/ws";
    this.logger = options.logger;
    this.enabled =
      options.enabled ??
      (process.env.KRONOS_KERNEL_IPC === "1" || Boolean(process.env.KRONOS_KERNEL_IPC_URL));
  }

  public isEnabled(): boolean {
    return this.enabled;
  }

  public shouldDelegate(messageType: string): boolean {
    if (!this.enabled) return false;
    return KERNEL_DELEGATED_PREFIXES.some((prefix) => messageType.startsWith(prefix));
  }

  public async connect(): Promise<void> {
    if (!this.enabled || this.ws) return;

    const ws = new WebSocket(this.url);
    this.ws = ws;

    await new Promise<void>((resolve, reject) => {
      const connectTimeout = setTimeout(() => {
        reject(new Error(`Timeout connecting to Rust kernel bridge at ${this.url}`));
      }, 5000);

      ws.on("open", () => {
        clearTimeout(connectTimeout);
        this.logger?.info({ url: this.url }, "Connected to native Rust kernel bridge");
        resolve();
      });

      ws.on("message", (data: WebSocket.RawData) => {
        this.handleMessage(data.toString());
      });

      ws.on("error", (err: Error) => {
        clearTimeout(connectTimeout);
        this.logger?.warn({ err: err.message }, "Rust kernel bridge error");
        reject(err);
      });

      ws.on("close", () => {
        this.ws = null;
        this.flushPending(new Error("Kernel bridge connection closed"));
      });
    });
  }

  public async dispatch<T = unknown>(
    id: string,
    messageType: string,
    payload: unknown,
    timeoutMs = 10_000,
  ): Promise<KernelBridgeResponse<T>> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      await this.connect();
    }

    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error("Rust kernel bridge is not connected");
    }

    const envelope = {
      id,
      type: messageType,
      payload,
    };

    return new Promise<KernelBridgeResponse<T>>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(
          new Error(`Kernel bridge request ${id} (${messageType}) timed out after ${timeoutMs}ms`),
        );
      }, timeoutMs);

      this.pendingRequests.set(id, {
        resolve: resolve as (value: KernelBridgeResponse) => void,
        reject,
        timer,
      });

      this.ws?.send(JSON.stringify(envelope), (err) => {
        if (err) {
          clearTimeout(timer);
          this.pendingRequests.delete(id);
          reject(err);
        }
      });
    });
  }

  public close(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.flushPending(new Error("Kernel bridge closed by caller"));
  }

  private handleMessage(raw: string): void {
    try {
      const parsed = JSON.parse(raw) as KernelBridgeResponse;
      const id = parsed.id;
      if (id && this.pendingRequests.has(id)) {
        const req = this.pendingRequests.get(id);
        if (req) {
          clearTimeout(req.timer);
          this.pendingRequests.delete(id);
          req.resolve(parsed);
        }
      }
    } catch (error) {
      this.logger?.warn({ err: error, raw }, "Failed to parse incoming kernel bridge frame");
    }
  }

  private flushPending(error: Error): void {
    for (const [, req] of this.pendingRequests) {
      clearTimeout(req.timer);
      req.reject(error);
    }
    this.pendingRequests.clear();
  }
}
