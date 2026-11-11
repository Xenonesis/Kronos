import { describe, it, expect, vi } from "vitest";
import { hasMentions, resolvePromptMentions, resolvePromptMentionsAsync } from "./mention-resolver";

describe("mention-resolver", () => {
  it("detects mentions accurately with hasMentions", () => {
    expect(hasMentions("check this")).toBe(false);
    expect(hasMentions("look at @git:diff")).toBe(true);
    expect(hasMentions("look at @git:status")).toBe(true);
    expect(hasMentions("look at @terminal:last-output")).toBe(true);
    expect(hasMentions("look at @file:src/App.tsx")).toBe(true);
    expect(hasMentions("look at @folder:src/components/")).toBe(true);
  });

  it("resolves synchronous context payloads", () => {
    const prompt = "Can you analyze @git:diff and check @git:status and @terminal:last-output?";
    const resolved = resolvePromptMentions(prompt, {
      gitDiff: "+ const x = 1;",
      gitStatus: "M src/App.tsx",
      terminalOutput: "Error: failed to compile",
    });

    expect(resolved).toContain("[Attached Context: Uncommitted Git Diff]");
    expect(resolved).toContain("+ const x = 1;");
    expect(resolved).toContain("[Attached Context: Git Working Tree Status]");
    expect(resolved).toContain("M src/App.tsx");
    expect(resolved).toContain("[Attached Context: Terminal Output]");
    expect(resolved).toContain("Error: failed to compile");
  });

  it("resolves @file mentions asynchronously with client.readFile", async () => {
    const fakeClient = {
      readFile: vi.fn(async (_cwd: string, path: string) => {
        return {
          bytes: new TextEncoder().encode(`export const App = () => <div>Hello ${path}</div>;`),
        };
      }),
    };

    const prompt = "Please review @file:src/App.tsx for any issues";
    const resolved = await resolvePromptMentionsAsync(prompt, {
      cwd: "/repo",
      client: fakeClient,
    });

    expect(fakeClient.readFile).toHaveBeenCalledWith("/repo", "src/App.tsx", undefined, 131072);
    expect(resolved).toContain("[Attached Context: file:src/App.tsx]");
    expect(resolved).toContain("```tsx");
    expect(resolved).toContain("export const App = () => <div>Hello src/App.tsx</div>;");
  });

  it("resolves @folder mentions asynchronously with client.getDirectorySuggestions", async () => {
    const fakeClient = {
      getDirectorySuggestions: vi.fn(async () => {
        return {
          entries: [
            { path: "src/components/Button.tsx", kind: "file" },
            { path: "src/components/Modal.tsx", kind: "file" },
            { path: "src/components/ui", kind: "directory" },
          ],
        };
      }),
    };

    const prompt = "What is inside @folder:src/components/ ?";
    const resolved = await resolvePromptMentionsAsync(prompt, {
      cwd: "/repo",
      client: fakeClient,
    });

    expect(fakeClient.getDirectorySuggestions).toHaveBeenCalledWith({
      cwd: "/repo",
      query: "src/components",
      limit: 40,
      includeFiles: true,
      includeDirectories: true,
    });
    expect(resolved).toContain("[Attached Context: directory:src/components/]");
    expect(resolved).toContain("- src/components/Button.tsx (file)");
    expect(resolved).toContain("- src/components/Modal.tsx (file)");
    expect(resolved).toContain("- src/components/ui (directory)");
  });

  it("handles fallback gracefully if client fails to read file", async () => {
    const fakeClient = {
      readFile: vi.fn(async () => {
        throw new Error("File not found");
      }),
    };

    const prompt = "Inspect @file:missing.ts";
    const resolved = await resolvePromptMentionsAsync(prompt, {
      cwd: "/repo",
      client: fakeClient,
    });

    expect(resolved).toContain("[Attached Context: file:missing.ts (File reference preserved)]");
  });
});
