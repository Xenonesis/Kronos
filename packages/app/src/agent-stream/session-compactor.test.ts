import { describe, expect, it } from "vitest";
import { compactSessionHistory, type RawChatMessage } from "./session-compactor";

describe("compactSessionHistory", () => {
  it("leaves short sessions untouched", () => {
    const shortHistory: RawChatMessage[] = [
      { role: "user", content: "Hello" },
      { role: "assistant", content: "Hi there! How can I help?" },
    ];
    const result = compactSessionHistory(shortHistory);
    expect(result.tokensFreedEstimate).toBe(0);
    expect(result.condensedMessages.length).toBe(2);
  });

  it("condenses long middle conversation while preserving initial instruction and recent turns", () => {
    const longHistory: RawChatMessage[] = [
      { role: "user", content: "Build a full stack web app with auth and database" },
      { role: "assistant", content: "Created src/auth/login.ts with JWT validation." },
      { role: "user", content: "Now add database tables" },
      { role: "assistant", content: "Modified src/db/schema.prisma with User and Session models." },
      { role: "user", content: "Add the API route" },
      { role: "assistant", content: "Created src/api/users.ts handler." },
      { role: "user", content: "Can you fix the type error in user service?" },
      { role: "assistant", content: "Fixed TypeError in user service." },
    ];

    const result = compactSessionHistory(longHistory);
    expect(result.tokensFreedEstimate).toBeGreaterThan(0);
    // Preserves first message
    expect(result.condensedMessages[0].content).toContain("Build a full stack web app");
    // Contains condensed summary
    expect(result.summary).toContain("Compacted Context Summary");
    expect(result.summary).toContain("schema.prisma");
    // Preserves recent 3 messages
    expect(result.condensedMessages[result.condensedMessages.length - 1].content).toContain(
      "Fixed TypeError in user service.",
    );
  });
});
