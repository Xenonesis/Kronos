import { describe, it, expect } from "vitest";
import { predictSmartCompletion } from "./ghost-text-autocomplete";

describe("ghost-text-autocomplete predictor logic", () => {
  it("predicts useState hook completion in TypeScript", () => {
    const prediction = predictSmartCompletion("const [", "", "app.tsx");
    expect(prediction).toBe("state, setState] = useState(false);");
  });

  it("predicts try-catch block completion in JavaScript", () => {
    const prediction = predictSmartCompletion("try {", "", "server.js");
    expect(prediction).toContain("catch (error)");
  });

  it("predicts async function completion in Rust", () => {
    const prediction = predictSmartCompletion("pub async fn ", "", "main.rs");
    expect(prediction).toContain("Result<(), String>");
  });

  it("predicts if __name__ == '__main__' in Python", () => {
    const prediction = predictSmartCompletion("if __name__ ==", "", "script.py");
    expect(prediction).toBe(' "__main__":\n    main()');
  });

  it("predicts HTML/JSX closing tags", () => {
    const prediction = predictSmartCompletion("<div>", "", "component.tsx");
    expect(prediction).toBe("</div>");
  });

  it("returns null when no pattern matches", () => {
    const prediction = predictSmartCompletion("hello world", "", "test.ts");
    expect(prediction).toBeNull();
  });
});
