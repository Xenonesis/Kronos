import { describe, expect, test } from "vitest";
import {
  detectTerminalFailure,
  stripAnsi,
  TerminalOutputErrorBuffer,
} from "./terminal-error-detector";

describe("terminal-error-detector (FEAT-21)", () => {
  test("strips ANSI color escapes cleanly", () => {
    const colored = "\u001b[31merror TS2304:\u001b[39m Cannot find name 'foo'";
    expect(stripAnsi(colored)).toBe("error TS2304: Cannot find name 'foo'");
  });

  test("detects TypeScript, Rust, Python, and non-zero exit errors", () => {
    expect(detectTerminalFailure("src/app.ts(10,5): error TS2322: Type mismatch")).toContain(
      "error TS2322",
    );
    expect(detectTerminalFailure("error[E0425]: cannot find value `x` in this scope")).toContain(
      "error[E0425]",
    );
    expect(detectTerminalFailure("Traceback (most recent call last):\n  File x.py")).toContain(
      "Traceback",
    );
    expect(detectTerminalFailure("Process exited with code 1")).toContain("exited with code 1");
  });

  test("ignores normal build success output", () => {
    expect(detectTerminalFailure("Compiled successfully in 412ms\n0 errors")).toBeNull();
  });

  test("TerminalOutputErrorBuffer handles chunked bytes and resets on clear", () => {
    const buf = new TerminalOutputErrorBuffer();
    const enc = new TextEncoder();
    expect(buf.pushBytes(enc.encode("Running tests...\n"))).toBeNull();
    const hit = buf.pushBytes(enc.encode("\u001b[31mFAIL packages/app/foo.test.ts\u001b[0m"));
    expect(hit).toContain("FAIL packages/app/foo.test.ts");
    buf.clear();
    expect(buf.pushText("All green")).toBeNull();
  });
});
