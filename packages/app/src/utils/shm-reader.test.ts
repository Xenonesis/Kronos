import { describe, expect, it } from "vitest";
import {
  SHM_HEADER_SIZE,
  SHM_MAGIC,
  SHM_RECORD_HEADER_SIZE,
  SHM_WRAP_SENTINEL,
  ShmReader,
} from "./shm-reader";

describe("ShmReader", () => {
  it("extracts frames from a buffer with full KRONOSSM header", () => {
    const capacity = 1024;
    const totalSize = SHM_HEADER_SIZE + capacity;
    const buffer = new ArrayBuffer(totalSize);
    const view = new DataView(buffer);
    const bytes = new Uint8Array(buffer);

    // Set header
    view.setBigUint64(0, SHM_MAGIC, true);
    view.setBigUint64(8, 2n, true); // seq
    view.setBigUint64(16, 50n, true); // head
    view.setBigUint64(24, BigInt(capacity), true); // cap

    // Write record 1: sequence 1, payload "hello" (5 bytes)
    const payload1 = new TextEncoder().encode("hello");
    let offset = SHM_HEADER_SIZE;
    view.setUint32(offset, payload1.length, true);
    view.setBigUint64(offset + 4, 1n, true);
    bytes.set(payload1, offset + SHM_RECORD_HEADER_SIZE);
    offset += SHM_RECORD_HEADER_SIZE + payload1.length;

    // Write record 2: sequence 2, payload "world" (5 bytes)
    const payload2 = new TextEncoder().encode("world");
    view.setUint32(offset, payload2.length, true);
    view.setBigUint64(offset + 4, 2n, true);
    bytes.set(payload2, offset + SHM_RECORD_HEADER_SIZE);

    const reader = new ShmReader(buffer);

    // Read since 0 -> both chunks
    const chunksAll = reader.readChunksSince(0);
    expect(chunksAll).toHaveLength(2);
    expect(new TextDecoder().decode(chunksAll[0])).toBe("hello");
    expect(new TextDecoder().decode(chunksAll[1])).toBe("world");

    // Read since 1 -> only second chunk
    const chunksSince1 = reader.readChunksSince(1);
    expect(chunksSince1).toHaveLength(1);
    expect(new TextDecoder().decode(chunksSince1[0])).toBe("world");

    // Read since 2 -> empty
    const chunksSince2 = reader.readChunksSince(2);
    expect(chunksSince2).toHaveLength(0);

    // Read latest
    const latest = reader.readLatest();
    expect(latest).not.toBeNull();
    expect(new TextDecoder().decode(latest!)).toBe("world");
  });

  it("handles wrap sentinel correctly", () => {
    const capacity = 256;
    const totalSize = SHM_HEADER_SIZE + capacity;
    const buffer = new ArrayBuffer(totalSize);
    const view = new DataView(buffer);
    const bytes = new Uint8Array(buffer);

    view.setBigUint64(0, SHM_MAGIC, true);
    view.setBigUint64(24, BigInt(capacity), true);

    // Write record 1
    const payload1 = new Uint8Array([1, 2, 3]);
    let offset = SHM_HEADER_SIZE;
    view.setUint32(offset, payload1.length, true);
    view.setBigUint64(offset + 4, 10n, true);
    bytes.set(payload1, offset + SHM_RECORD_HEADER_SIZE);
    offset += SHM_RECORD_HEADER_SIZE + payload1.length;

    // Write wrap sentinel
    view.setUint32(offset, SHM_WRAP_SENTINEL, true);

    const reader = new ShmReader(buffer);
    const chunks = reader.readChunksSince(0);
    expect(chunks).toHaveLength(1);
    expect(Array.from(chunks[0])).toEqual([1, 2, 3]);
  });

  it("extracts frames from raw record buffer without header", () => {
    const recordSize = SHM_RECORD_HEADER_SIZE + 4;
    const buffer = new ArrayBuffer(recordSize);
    const view = new DataView(buffer);
    const bytes = new Uint8Array(buffer);

    view.setUint32(0, 4, true);
    view.setBigUint64(4, 42n, true);
    bytes.set([9, 8, 7, 6], SHM_RECORD_HEADER_SIZE);

    const reader = new ShmReader(buffer);
    const chunks = reader.readChunksSince(40);
    expect(chunks).toHaveLength(1);
    expect(Array.from(chunks[0])).toEqual([9, 8, 7, 6]);

    expect(reader.readChunksSince(42)).toHaveLength(0);
  });
});
