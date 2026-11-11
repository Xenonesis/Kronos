import { describe, it, expect } from "vitest";
import { parseCustomWorkflowFile, interpolateWorkflowVariables } from "./custom-workflow-loader";

describe("custom-workflow-loader", () => {
  it("parses markdown file with YAML frontmatter", () => {
    const content = `---
name: audit-auth
description: Deeply audits authentication flow and JWT validation
---
Please audit the following authentication code:
$SELECTION
File: $ACTIVE_FILE
`;
    const res = parseCustomWorkflowFile(".kronos/commands/audit-auth.md", content);
    expect(res.name).toBe("audit-auth");
    expect(res.description).toBe("Deeply audits authentication flow and JWT validation");
    expect(res.template).toContain("Please audit the following authentication code");
  });

  it("interpolates template variables correctly", () => {
    const template = "Refactor $SELECTION in $ACTIVE_FILE against diff:\n$DIFF";
    const context = {
      selection: "function login() {}",
      activeFile: "src/auth.ts",
      diff: "+ const token = true;",
    };
    const interpolated = interpolateWorkflowVariables(template, context);
    expect(interpolated).toContain("function login() {}");
    expect(interpolated).toContain("src/auth.ts");
    expect(interpolated).toContain("+ const token = true;");
  });
});
