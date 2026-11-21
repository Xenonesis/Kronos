import {
  closeLocalTransportSession,
  acknowledgeLocalTransportMessage,
  listenToLocalTransportEvents,
  openLocalTransportSession,
  sendLocalTransportMessage,
  type OpenLocalTransportSessionInput,
} from "./desktop-daemon";

export interface LocalDaemonTransportEvent {
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

export interface LocalDaemonTransportRpc {
  openSession(input: OpenLocalTransportSessionInput): Promise<void>;
  listenToEvents(
    handler: (event: LocalDaemonTransportEvent) => void,
    scope?: { sessionId: string; generation: string },
  ): Promise<() => void>;
  sendMessage(input: {
    sessionId: string;
    generation?: string;
    text?: string;
    binaryBase64?: string;
  }): Promise<void>;
  closeSession(sessionId: string, generation?: string): Promise<void>;
  ackMessage(input: { sessionId: string; generation: string; sequence: number }): Promise<void>;
}

export const defaultLocalDaemonTransportRpc: LocalDaemonTransportRpc = {
  openSession: openLocalTransportSession,
  listenToEvents: listenToLocalTransportEvents,
  sendMessage: sendLocalTransportMessage,
  closeSession: closeLocalTransportSession,
  ackMessage: acknowledgeLocalTransportMessage,
};
