import { describe, expect, it } from "vitest";
import {
  extractFallbackSymbols,
  findSymbolAtLine,
  flattenSymbols,
  extractAstSymbols,
} from "./extract-symbols";
import type { AstSymbol } from "./types";

describe("AST Symbol Extraction & Navigation", () => {
  it("extracts symbols using fallback regex parser for TypeScript", () => {
    const code = `
export function calculateSum(a: number, b: number): number {
  return a + b;
}

export class OrderService {
  process() {}
}

export interface UserProfile {
  id: string;
}

export type ThemeMode = "light" | "dark";
    `;

    const symbols = extractFallbackSymbols(code, "service.ts");
    expect(symbols.length).toBe(4);
    expect(symbols[0].name).toBe("calculateSum");
    expect(symbols[0].kind).toBe("function");
    expect(symbols[1].name).toBe("OrderService");
    expect(symbols[1].kind).toBe("class");
    expect(symbols[2].name).toBe("UserProfile");
    expect(symbols[2].kind).toBe("interface");
    expect(symbols[3].name).toBe("ThemeMode");
    expect(symbols[3].kind).toBe("type_alias");
  });

  it("extracts symbols for Python", () => {
    const code = `
class AgentRunner:
    def execute(self):
        pass

def standalone_helper():
    return 42
    `;

    const symbols = extractFallbackSymbols(code, "runner.py");
    expect(symbols.length).toBe(3);
    expect(symbols[0].name).toBe("AgentRunner");
    expect(symbols[0].kind).toBe("class");
    expect(symbols[1].name).toBe("execute");
    expect(symbols[1].kind).toBe("function");
    expect(symbols[2].name).toBe("standalone_helper");
    expect(symbols[2].kind).toBe("function");
  });

  it("extracts symbols for Rust", () => {
    const code = `
pub struct KernelState {
    pub count: usize,
}

pub enum EngineMode {
    Fast,
    Strict,
}

pub async fn start_daemon() {
}
    `;

    const symbols = extractFallbackSymbols(code, "kernel.rs");
    expect(symbols.length).toBe(3);
    expect(symbols[0].name).toBe("KernelState");
    expect(symbols[0].kind).toBe("class");
    expect(symbols[1].name).toBe("EngineMode");
    expect(symbols[1].kind).toBe("enum");
    expect(symbols[2].name).toBe("start_daemon");
    expect(symbols[2].kind).toBe("function");
  });

  it("finds the deepest symbol at a given line number", () => {
    const symbols: AstSymbol[] = [
      {
        name: "MyClass",
        kind: "class",
        range: {
          start_line: 10,
          start_col: 1,
          end_line: 50,
          end_col: 1,
          start_byte: 0,
          end_byte: 0,
        },
        exported: true,
        children: [
          {
            name: "calculateMetrics",
            kind: "method",
            range: {
              start_line: 15,
              start_col: 3,
              end_line: 30,
              end_col: 3,
              start_byte: 0,
              end_byte: 0,
            },
            exported: false,
            children: [],
          },
        ],
      },
      {
        name: "standaloneFn",
        kind: "function",
        range: {
          start_line: 60,
          start_col: 1,
          end_line: 80,
          end_col: 1,
          start_byte: 0,
          end_byte: 0,
        },
        exported: true,
        children: [],
      },
    ];

    expect(findSymbolAtLine(symbols, 20)?.name).toBe("calculateMetrics");
    expect(findSymbolAtLine(symbols, 11)?.name).toBe("MyClass");
    expect(findSymbolAtLine(symbols, 70)?.name).toBe("standaloneFn");
    expect(findSymbolAtLine(symbols, 90)).toBeNull();
  });

  it("flattens nested symbols with display names", () => {
    const symbols: AstSymbol[] = [
      {
        name: "UserManager",
        kind: "class",
        range: {
          start_line: 1,
          start_col: 1,
          end_line: 100,
          end_col: 1,
          start_byte: 0,
          end_byte: 0,
        },
        exported: true,
        children: [
          {
            name: "getUser",
            kind: "method",
            range: {
              start_line: 10,
              start_col: 3,
              end_line: 20,
              end_col: 3,
              start_byte: 0,
              end_byte: 0,
            },
            exported: false,
            children: [],
          },
        ],
      },
    ];

    const flat = flattenSymbols(symbols);
    expect(flat.length).toBe(2);
    expect(flat[0].displayName).toBe("UserManager");
    expect(flat[1].displayName).toBe("UserManager > getUser");
  });

  it("delegates to native invoke when window.paseoDesktop is available", async () => {
    const mockInvoke = async (cmd: string, args: unknown) => {
      if (cmd === "analyze_ast") {
        return {
          root_kind: "program",
          node_count: 42,
          declarations: ["doWork"],
          symbols: [
            {
              name: "doWork",
              kind: "function",
              range: {
                start_line: 5,
                start_col: 1,
                end_line: 10,
                end_col: 1,
                start_byte: 0,
                end_byte: 0,
              },
              exported: true,
              children: [],
            },
          ],
          imports: [],
          errors: [],
        };
      }
      return null;
    };

    // @ts-expect-error Mock window
    globalThis.window = {
      paseoDesktop: {
        invoke: mockInvoke,
      },
    };

    const res = await extractAstSymbols("function doWork() {}", "test.ts");
    expect(res.length).toBe(1);
    expect(res[0].name).toBe("doWork");
    expect(res[0].range.start_line).toBe(5);

    // @ts-expect-error Reset window
    delete globalThis.window;
  });
});
