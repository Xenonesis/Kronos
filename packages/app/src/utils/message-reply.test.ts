import { describe, expect, it } from "vitest";
import { formatQuoteReply } from "./message-reply";

describe("formatQuoteReply", () => {
  it("formats single-line text as blockquote with trailing newlines", () => {
    const input = "Hello world";
    expect(formatQuoteReply(input)).toBe("> Hello world\n\n");
  });

  it("prefixes every line with >", () => {
    const input = "Line 1\nLine 2\nLine 3";
    expect(formatQuoteReply(input)).toBe("> Line 1\n> Line 2\n> Line 3\n\n");
  });

  it("returns empty string for blank text", () => {
    expect(formatQuoteReply("   ")).toBe("");
  });
});
