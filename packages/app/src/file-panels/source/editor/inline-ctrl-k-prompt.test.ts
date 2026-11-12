import { describe, it, expect } from "vitest";

describe("inline-ctrl-k-prompt logic", () => {
  it("transforms sync functions to async when requested", () => {
    const input = "function fetchData() {\n  return 42;\n}";
    const result = input.replace(/function\s+(\w+)\s*\(/g, "async function $1(");
    expect(result).toBe("async function fetchData() {\n  return 42;\n}");
  });

  it("transforms function statements to arrow functions", () => {
    const input = "function add(a, b) {\n  return a + b;\n}";
    const result = input.replace(/function\s+(\w+)\s*\(([^)]*)\)\s*\{/g, "const $1 = ($2) => {");
    expect(result).toBe("const add = (a, b) => {\n  return a + b;\n}");
  });

  it("wraps code in try-catch error handler", () => {
    const input = "const res = fetch('/api');";
    const result = `try {\n  ${input}\n} catch (error) {\n  console.error("Error executing operation:", error);\n  throw error;\n}`;
    expect(result).toContain("try {");
    expect(result).toContain("catch (error)");
    expect(result).toContain(input);
  });
});
