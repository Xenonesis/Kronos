import { describe, expect, it } from "vitest";
import { stripMarkdownForSpeech } from "./speech-synthesis";

describe("stripMarkdownForSpeech", () => {
  it("removes code fences and leaves speech indicator", () => {
    const input = "Here is the code:\n```typescript\nconst x = 10;\n```\nDone.";
    const result = stripMarkdownForSpeech(input);
    expect(result).toBe("Here is the code: Code block omitted. Done.");
  });

  it("strips bold, italics, and inline code", () => {
    const input = "This is **bold** and *italic* and `inlineCode`.";
    const result = stripMarkdownForSpeech(input);
    expect(result).toBe("This is bold and italic and inlineCode.");
  });

  it("converts links to their anchor text", () => {
    const input = "Visit [Google](https://google.com) for details.";
    const result = stripMarkdownForSpeech(input);
    expect(result).toBe("Visit Google for details.");
  });

  it("strips blockquotes and headings", () => {
    const input = "# Title\n\n> Quoted text\n\nBody paragraph.";
    const result = stripMarkdownForSpeech(input);
    expect(result).toBe("Title Quoted text. Body paragraph.");
  });
});
