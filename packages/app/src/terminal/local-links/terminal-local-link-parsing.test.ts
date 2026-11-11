import { describe, expect, it } from "vitest";
import { detectDevServerUrls, detectTerminalLocalLinks } from "./terminal-local-link-parsing";

describe("detectDevServerUrls", () => {
  it("detects localhost ports from Vite, Next.js, and Metro dev servers", () => {
    const viteOut = "  ➜  Local:   http://localhost:5173/";
    const detected = detectDevServerUrls(viteOut);
    expect(detected).toEqual([{ url: "http://localhost:5173/", port: 5173 }]);

    const nextOut = "- Local:        http://127.0.0.1:3000/app";
    const detectedNext = detectDevServerUrls(nextOut);
    expect(detectedNext).toEqual([{ url: "http://localhost:3000/app", port: 3000 }]);

    const barePort = "Ready on localhost:8080";
    const detectedBare = detectDevServerUrls(barePort);
    expect(detectedBare).toEqual([{ url: "http://localhost:8080", port: 8080 }]);
  });
});

describe("detectTerminalLocalLinks", () => {
  it("detects VS Code-style filename and line suffixes", () => {
    expect(detectTerminalLocalLinks("file.ts:42")).toMatchObject([
      {
        path: { index: 0, text: "file.ts" },
        suffix: { row: 42, col: undefined, rowEnd: undefined },
      },
    ]);
  });

  it("detects line and column suffixes", () => {
    expect(detectTerminalLocalLinks("src/file.ts:42:7")).toMatchObject([
      {
        path: { index: 0, text: "src/file.ts" },
        suffix: { row: 42, col: 7, rowEnd: undefined },
      },
    ]);
  });

  it("detects quoted Python traceback paths", () => {
    expect(detectTerminalLocalLinks('  File "pkg/file.py", line 12')).toMatchObject([
      {
        path: { index: 8, text: "pkg/file.py" },
        suffix: { row: 12 },
      },
    ]);
  });

  it("detects paths without suffixes", () => {
    expect(detectTerminalLocalLinks("changed packages/app/src/file.ts")).toMatchObject([
      {
        path: { index: 8, text: "packages/app/src/file.ts" },
        suffix: undefined,
      },
    ]);
  });
});
