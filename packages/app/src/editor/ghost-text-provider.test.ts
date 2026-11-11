import { describe, expect, it } from "vitest";
import { generatePredictiveGhostText } from "./ghost-text-provider";

describe("generatePredictiveGhostText", () => {
  it("suggests import completions for import statements", () => {
    const code = "import { Button";
    const suggestion = generatePredictiveGhostText(code, code.length);
    expect(suggestion).not.toBeNull();
    expect(suggestion?.suggestion).toContain('from "./button"');
  });

  it("suggests react hook boilerplates", () => {
    const code = "useEffect(";
    const suggestion = generatePredictiveGhostText(code, code.length);
    expect(suggestion).not.toBeNull();
    expect(suggestion?.suggestion).toContain("() => {");
    expect(suggestion?.suggestion).toContain("}, []);");
  });

  it("suggests try catch blocks", () => {
    const code = "try {";
    const suggestion = generatePredictiveGhostText(code, code.length);
    expect(suggestion).not.toBeNull();
    expect(suggestion?.suggestion).toContain("catch (error)");
  });

  it("returns null for short or ambiguous inputs", () => {
    expect(generatePredictiveGhostText("a", 1)).toBeNull();
    expect(generatePredictiveGhostText("", 0)).toBeNull();
  });
});
