import { ZodRealError } from "zod";
import type { z } from "zod";
import { WSOutboundEnvelopeSchema, sessionOutboundMessageOptions } from "./ws-outbound-envelope.js";
import type { SessionOutboundMessage, WSOutboundMessage } from "../messages.js";

type ValidationResult<T> = { success: true; data: T } | { success: false; error: z.ZodError };

interface GeneratedValidator<T> {
  safeParse(input: unknown): ValidationResult<T>;
}

// Plain Zod fallback schema lookup map
const sessionSchemaMap = new Map<string, z.ZodTypeAny>();
for (const option of sessionOutboundMessageOptions) {
  sessionSchemaMap.set((option as any).shape.type.value, option);
}

let aotEnvelopeValidator: GeneratedValidator<WSOutboundMessage> | null = null;
let aotSessionValidators: Record<
  SessionOutboundMessage["type"],
  GeneratedValidator<SessionOutboundMessage>
> | null = null;
let aotLoadPromise: Promise<void> | null = null;

/**
 * Pre-warms or eagerly loads the zod-aot compiled validators.
 * When resolved, subsequent calls to `validateWSOutboundMessage` use the AOT compiled functions.
 */
export function loadWSOutboundAotValidators(): Promise<void> {
  if (aotLoadPromise) return aotLoadPromise;
  aotLoadPromise = import("../generated/validation/ws-outbound.aot.js")
    .then((mod) => {
      aotEnvelopeValidator = mod.WSOutboundEnvelope as GeneratedValidator<WSOutboundMessage>;
      aotSessionValidators = mod.SessionOutboundMessageValidators as Record<
        SessionOutboundMessage["type"],
        GeneratedValidator<SessionOutboundMessage>
      >;
    })
    .catch((err) => {
      console.warn("Failed to load zod-aot validators, using runtime zod schemas", err);
    });
  return aotLoadPromise;
}

export function isWSOutboundAotLoaded(): boolean {
  return aotEnvelopeValidator !== null && aotSessionValidators !== null;
}

// The message validator reports paths relative to the message, nested issues included.
function underMessage(issue: z.core.$ZodIssue): z.core.$ZodIssue {
  const path = ["message", ...issue.path];
  switch (issue.code) {
    case "invalid_union":
      return issue.inclusive === false
        ? { ...issue, path }
        : { ...issue, path, errors: issue.errors.map((branch) => branch.map(underMessage)) };
    case "invalid_key":
    case "invalid_element":
      return { ...issue, path, issues: issue.issues.map(underMessage) };
    default:
      return { ...issue, path };
  }
}

export function validateWSOutboundMessage(input: unknown): ValidationResult<WSOutboundMessage> {
  if (aotEnvelopeValidator && aotSessionValidators) {
    const envelope = aotEnvelopeValidator.safeParse(input);
    if (!envelope.success || envelope.data.type !== "session") {
      return envelope;
    }
    const message = envelope.data.message;
    const validator = aotSessionValidators[message.type];
    if (!validator) {
      return envelope;
    }
    const result = validator.safeParse(message);
    if (!result.success) {
      return { success: false, error: new ZodRealError(result.error.issues.map(underMessage)) };
    }
    // Message validators return a new object when they apply defaults.
    if (result.data === message) {
      return envelope;
    }
    return { success: true, data: { ...envelope.data, message: result.data } };
  }

  // Runtime Zod fallback when AOT is not yet loaded
  const envelope = WSOutboundEnvelopeSchema.safeParse(input);
  if (!envelope.success || envelope.data.type !== "session") {
    return envelope as ValidationResult<WSOutboundMessage>;
  }
  const rawMessage = (input as { message?: unknown })?.message;
  const messageType = (rawMessage as { type?: string })?.type ?? envelope.data.message.type;
  const validator = sessionSchemaMap.get(messageType);
  if (!validator) {
    return envelope as ValidationResult<WSOutboundMessage>;
  }
  const result = validator.safeParse(rawMessage ?? envelope.data.message);
  if (!result.success) {
    return { success: false, error: new ZodRealError(result.error.issues.map(underMessage)) };
  }
  if (result.data === rawMessage) {
    return {
      success: true,
      data: { ...envelope.data, message: result.data as SessionOutboundMessage },
    };
  }
  return {
    success: true,
    data: { ...envelope.data, message: result.data as SessionOutboundMessage },
  };
}
