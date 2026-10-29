/**
 * Zero-Copy Shared Memory Ring Buffer (SMRB) Frame Reader.
 * Extracts binary payload frames directly from ArrayBuffer / Uint8Array slices
 * without JSON serialization, UTF-8 string decoding, or heap copying overhead.
 *
 * Header Layout (32 bytes):
 * - 0..8:   Magic (0x4B524F4E4F53534D = "KRONOSSM")
 * - 8..16:  Atomic write sequence (u64, little-endian)
 * - 16..24: Atomic write head offset (u64, little-endian)
 * - 24..32: Capacity of the ring buffer body (u64, little-endian)
 *
 * Record Frame Layout:
 * - 0..4:   Payload length (u32, little-endian)
 * - 4..12:  Sequence ID (u64, little-endian)
 * - 12..N:  Raw payload bytes
 */

export const SHM_MAGIC = 0x4b524f4e4f53534dn; // "KRONOSSM"
export const SHM_HEADER_SIZE = 32;
export const SHM_RECORD_HEADER_SIZE = 12;
export const SHM_WRAP_SENTINEL = 0xffffffff;

export interface ShmRecordFrame {
  sequence: number;
  payload: Uint8Array;
}

export class ShmReader {
  private buffer: Uint8Array;
  private view: DataView;

  constructor(buffer: ArrayBuffer | Uint8Array) {
    if (buffer instanceof Uint8Array) {
      this.buffer = buffer;
      this.view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
    } else {
      this.buffer = new Uint8Array(buffer);
      this.view = new DataView(buffer);
    }
  }

  /**
   * Replaces or updates the underlying buffer without reallocating the reader instance.
   */
  public updateBuffer(buffer: ArrayBuffer | Uint8Array): void {
    if (buffer instanceof Uint8Array) {
      this.buffer = buffer;
      this.view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
    } else {
      this.buffer = new Uint8Array(buffer);
      this.view = new DataView(buffer);
    }
  }

  /**
   * Reads and extracts all payload chunks whose sequence ID is strictly greater than `lastSequence`.
   * Returns an array of Uint8Array payload frames ordered by sequence ID ascending.
   * Frame extraction is zero-copy (views directly into the underlying buffer memory).
   */
  public readChunksSince(lastSequence: number): Uint8Array[] {
    const frames = this.readFramesSince(lastSequence);
    return frames.map((frame: ShmRecordFrame): Uint8Array => frame.payload);
  }

  /**
   * Reads and extracts all frames (including sequence ID and zero-copy payload)
   * whose sequence ID is strictly greater than `lastSequence`.
   */
  public readFramesSince(lastSequence: number): ShmRecordFrame[] {
    const totalBytes = this.buffer.byteLength;
    if (totalBytes < SHM_RECORD_HEADER_SIZE) {
      return [];
    }

    let bodyStart = 0;
    let bodyLimit = totalBytes;

    // Check for KRONOSSM header
    if (totalBytes >= SHM_HEADER_SIZE) {
      const magicLE = this.view.getBigUint64(0, true);
      const magicBE = this.view.getBigUint64(0, false);
      if (magicLE === SHM_MAGIC || magicBE === SHM_MAGIC) {
        bodyStart = SHM_HEADER_SIZE;
        const capacity = Number(this.view.getBigUint64(24, true));
        if (capacity > 0 && bodyStart + capacity <= totalBytes) {
          bodyLimit = bodyStart + capacity;
        }
      }
    }

    const records: ShmRecordFrame[] = [];
    let p = bodyStart;

    while (p + SHM_RECORD_HEADER_SIZE <= bodyLimit) {
      const length = this.view.getUint32(p, true);

      // Wrap sentinel: ring buffer wrapped to offset 0 (bodyStart)
      if (length === SHM_WRAP_SENTINEL) {
        break;
      }

      const seqBig = this.view.getBigUint64(p + 4, true);

      // Unwritten / zero-filled region
      if (length === 0 && seqBig === 0n) {
        // Skip ahead by 4 bytes to find next potential record
        p += 4;
        continue;
      }

      // Corrupt or out-of-bounds length
      if (p + SHM_RECORD_HEADER_SIZE + length > bodyLimit) {
        break;
      }

      const sequence = Number(seqBig);
      if (sequence > lastSequence) {
        const payload = this.buffer.subarray(
          p + SHM_RECORD_HEADER_SIZE,
          p + SHM_RECORD_HEADER_SIZE + length,
        );
        records.push({ sequence, payload });
      }

      p += SHM_RECORD_HEADER_SIZE + length;
    }

    // Sort by sequence ascending to ensure strict chronological delivery
    records.sort((a: ShmRecordFrame, b: ShmRecordFrame): number => a.sequence - b.sequence);
    return records;
  }

  /**
   * Reads the latest payload frame available in the buffer, or null if empty.
   */
  public readLatest(): Uint8Array | null {
    const frames = this.readFramesSince(-1);
    if (frames.length === 0) {
      return null;
    }
    return frames[frames.length - 1].payload;
  }
}
