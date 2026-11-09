import { mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";

const __dirname = dirname(fileURLToPath(import.meta.url));
const protocolRoot = resolve(__dirname, "..");
const schemasDir = resolve(protocolRoot, "schemas");
const inboundDir = resolve(schemasDir, "inbound");
const outboundDir = resolve(schemasDir, "outbound");

await mkdir(inboundDir, { recursive: true });
await mkdir(outboundDir, { recursive: true });

const messages = await import("../dist/messages.js");

function sanitizeFilename(typeTag) {
  return typeTag.replace(/[/:*?"<>|.]/g, "-");
}

const manifest = {
  version: messages.PROTOCOL_VERSION ?? 1,
  generatedAt: new Date().toISOString(),
  inbound: {},
  outbound: {},
  topLevel: {
    wsInbound: [],
    wsOutbound: [],
  },
};

let totalInbound = 0;
let totalOutbound = 0;

for (const schema of messages.SessionInboundMessageSchema.options) {
  const jsonSchema = z.toJSONSchema(schema, { unrepresentable: "any" });
  const typeTag = schema.shape?.type?.value ?? schema.shape?.type?._def?.value ?? "unknown";
  const filename = `${sanitizeFilename(typeTag)}.json`;

  await writeFile(resolve(inboundDir, filename), JSON.stringify(jsonSchema, null, 2));
  manifest.inbound[typeTag] = {
    file: `inbound/${filename}`,
    required: jsonSchema.required ?? [],
    properties: Object.keys(jsonSchema.properties ?? {}),
  };
  totalInbound++;
}

for (const schema of messages.SessionOutboundMessageSchema.options) {
  const jsonSchema = z.toJSONSchema(schema, { unrepresentable: "any" });
  const typeTag = schema.shape?.type?.value ?? schema.shape?.type?._def?.value ?? "unknown";
  const filename = `${sanitizeFilename(typeTag)}.json`;

  await writeFile(resolve(outboundDir, filename), JSON.stringify(jsonSchema, null, 2));
  manifest.outbound[typeTag] = {
    file: `outbound/${filename}`,
    required: jsonSchema.required ?? [],
    properties: Object.keys(jsonSchema.properties ?? {}),
  };
  totalOutbound++;
}

for (const schema of messages.WSInboundMessageSchema.options) {
  const jsonSchema = z.toJSONSchema(schema, { unrepresentable: "any" });
  const typeTag = schema.shape?.type?.value ?? schema.shape?.type?._def?.value ?? "ws-inbound";
  manifest.topLevel.wsInbound.push({
    type: typeTag,
    schema: jsonSchema,
  });
}

for (const schema of messages.WSOutboundMessageSchema.options) {
  const jsonSchema = z.toJSONSchema(schema, { unrepresentable: "any" });
  const typeTag = schema.shape?.type?.value ?? schema.shape?.type?._def?.value ?? "ws-outbound";
  manifest.topLevel.wsOutbound.push({
    type: typeTag,
    schema: jsonSchema,
  });
}

await writeFile(resolve(schemasDir, "manifest.json"), JSON.stringify(manifest, null, 2));

console.log(
  `[export-json-schemas] Successfully exported ${totalInbound} inbound and ${totalOutbound} outbound JSON schemas (total ${totalInbound + totalOutbound + manifest.topLevel.wsInbound.length + manifest.topLevel.wsOutbound.length}) to ${schemasDir}`,
);
