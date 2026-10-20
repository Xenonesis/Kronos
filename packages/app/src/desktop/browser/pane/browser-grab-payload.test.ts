import { describe, expect, it } from "vitest";
import {
  type BrowserGrabAccessibility,
  type BrowserGrabComputedStyles,
  type BrowserGrabPayload,
  type BrowserGrabTarget,
  formatAccessibility,
  formatComputedStyles,
  formatGrabPayloadAsText,
} from "./browser-grab-payload";

describe("formatComputedStyles", () => {
  it("formats all style properties into a CSS style string", () => {
    const styles: BrowserGrabComputedStyles = {
      display: "flex",
      position: "relative",
      color: "rgb(255, 255, 255)",
      backgroundColor: "rgb(34, 197, 94)",
      fontSize: "14px",
      fontWeight: "600",
      padding: "8px 16px",
      margin: "0px",
      border: "1px solid rgb(22, 163, 74)",
    };

    const formatted = formatComputedStyles(styles);
    expect(formatted).toBe(
      "display: flex; position: relative; color: rgb(255, 255, 255); background-color: rgb(34, 197, 94); font-size: 14px; font-weight: 600; padding: 8px 16px; margin: 0px; border: 1px solid rgb(22, 163, 74)",
    );
  });

  it("omits empty or missing style properties", () => {
    const styles: BrowserGrabComputedStyles = {
      display: "block",
      position: "",
      color: "#000",
      backgroundColor: "",
      fontSize: "16px",
      fontWeight: "",
      padding: "",
      margin: "",
      border: "",
    };

    const formatted = formatComputedStyles(styles);
    expect(formatted).toBe("display: block; color: #000; font-size: 16px");
  });
});

describe("formatAccessibility", () => {
  it("formats role, aria-label, and accessible name", () => {
    const acc: BrowserGrabAccessibility = {
      role: "button",
      ariaLabel: "Submit form",
      accessibleName: "Submit",
    };

    const formatted = formatAccessibility(acc);
    expect(formatted).toBe('role="button", aria-label="Submit form", name="Submit"');
  });

  it("handles partial accessibility fields", () => {
    expect(formatAccessibility({ role: "navigation" })).toBe('role="navigation"');
    expect(formatAccessibility({ ariaLabel: "Close dialog" })).toBe('aria-label="Close dialog"');
    expect(formatAccessibility({ accessibleName: "Cancel" })).toBe('name="Cancel"');
  });

  it("returns empty string when all fields are empty or undefined", () => {
    expect(formatAccessibility({})).toBe("");
  });
});

describe("formatGrabPayloadAsText", () => {
  const createBaseTarget = (overrides?: Partial<BrowserGrabTarget>): BrowserGrabTarget => ({
    tag: "button",
    selector: "body > div.app > button#action-btn",
    rect: {
      left: 100,
      top: 200,
      width: 120,
      height: 40,
    },
    textContent: "Click me",
    ...overrides,
  });

  it("formats minimal payload with required fields only", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000/dashboard",
      target: createBaseTarget(),
    };

    const text = formatGrabPayloadAsText(payload);

    expect(text).toContain("Attached browser context from http://localhost:3000/dashboard:");
    expect(text).toContain("- Selected element: <button>");
    expect(text).toContain("- Selector: body > div.app > button#action-btn");
    expect(text).toContain("- Dimensions: 120x40 at (100, 200)");
    expect(text).toContain('- Text content: "Click me"');

    // Optional fields should NOT be present
    expect(text).not.toContain("- Component:");
    expect(text).not.toContain("- Source:");
    expect(text).not.toContain("- Nearby context:");
    expect(text).not.toContain("- Computed styles:");
    expect(text).not.toContain("- Accessibility:");
    expect(text).not.toContain("- HTML:");
  });

  it("includes page title when provided in payload", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000/settings",
      title: "Settings Overview",
      target: createBaseTarget(),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).toContain(
      "Attached browser context from http://localhost:3000/settings (Settings Overview):",
    );
  });

  it("includes React component and source when present", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        component: "PrimaryActionButton",
        source: "src/components/button.tsx:42:5",
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).toContain("- Component: PrimaryActionButton");
    expect(text).toContain("- Source: src/components/button.tsx:42:5");
  });

  it("omits React component and source lines when absent", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        component: undefined,
        source: undefined,
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).not.toContain("- Component:");
    expect(text).not.toContain("- Source:");
  });

  it("includes only component when source is absent", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        component: "HeaderNav",
        source: undefined,
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).toContain("- Component: HeaderNav");
    expect(text).not.toContain("- Source:");
  });

  it("includes computed styles when present", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        computedStyles: {
          display: "inline-flex",
          position: "static",
          color: "#ffffff",
          backgroundColor: "#3b82f6",
          fontSize: "14px",
          fontWeight: "500",
          padding: "6px 12px",
          margin: "4px",
          border: "none",
        },
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).toContain(
      "- Computed styles: display: inline-flex; position: static; color: #ffffff; background-color: #3b82f6; font-size: 14px; font-weight: 500; padding: 6px 12px; margin: 4px; border: none",
    );
  });

  it("omits computed styles line when absent", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        computedStyles: undefined,
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).not.toContain("- Computed styles:");
  });

  it("includes accessibility when present", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        accessibility: {
          role: "button",
          ariaLabel: "Close modal",
          accessibleName: "Close",
        },
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).toContain(
      '- Accessibility: role="button", aria-label="Close modal", name="Close"',
    );
  });

  it("omits accessibility line when absent", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        accessibility: undefined,
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).not.toContain("- Accessibility:");
  });

  it("includes nearby context and HTML snippet when present", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        nearbyContext: 'Parent: <form>, Siblings: <input type="text">',
        html: '<button type="submit" class="btn btn-primary" id="submit-btn">Submit</button>',
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).toContain('- Nearby context: Parent: <form>, Siblings: <input type="text">');
    expect(text).toContain(
      '- HTML:\n<button type="submit" class="btn btn-primary" id="submit-btn">Submit</button>',
    );
  });

  it("correctly formats id and multiple class names in selected element tag", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        tag: "button",
        id: "checkout-btn",
        className: "btn btn-primary btn-lg active",
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).toContain(
      "- Selected element: <button#checkout-btn.btn.btn-primary.btn-lg.active>",
    );
  });

  it("rounds dimension numbers cleanly", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:3000",
      target: createBaseTarget({
        rect: {
          left: 10.4,
          top: 20.8,
          width: 99.6,
          height: 49.2,
        },
      }),
    };

    const text = formatGrabPayloadAsText(payload);
    expect(text).toContain("- Dimensions: 100x49 at (10, 21)");
  });

  it("generates a complete formatted payload matching Orca specification", () => {
    const payload: BrowserGrabPayload = {
      url: "http://localhost:5173/admin",
      title: "Admin Console",
      target: {
        tag: "button",
        id: "save-changes",
        className: "btn btn-success",
        selector: "main.admin-content > div.form-actions > button#save-changes",
        rect: {
          left: 350,
          top: 600,
          width: 140,
          height: 44,
        },
        textContent: "Save Changes",
        component: "SaveButton",
        source: "src/admin/components/save-button.tsx:18:3",
        nearbyContext:
          'Parent: <div class="form-actions">, Siblings: <button class="btn btn-secondary">',
        computedStyles: {
          display: "inline-block",
          position: "relative",
          color: "#ffffff",
          backgroundColor: "#16a34a",
          fontSize: "14px",
          fontWeight: "600",
          padding: "10px 20px",
          margin: "0px",
          border: "1px solid #15803d",
        },
        accessibility: {
          role: "button",
          ariaLabel: "Save current form changes",
          accessibleName: "Save Changes",
        },
        html: '<button id="save-changes" class="btn btn-success">Save Changes</button>',
      },
      timestamp: 1727700000000,
    };

    const result = formatGrabPayloadAsText(payload);

    const expected = [
      "Attached browser context from http://localhost:5173/admin (Admin Console):",
      "- Selected element: <button#save-changes.btn.btn-success>",
      "- Component: SaveButton",
      "- Source: src/admin/components/save-button.tsx:18:3",
      "- Selector: main.admin-content > div.form-actions > button#save-changes",
      "- Dimensions: 140x44 at (350, 600)",
      '- Text content: "Save Changes"',
      '- Nearby context: Parent: <div class="form-actions">, Siblings: <button class="btn btn-secondary">',
      "- Computed styles: display: inline-block; position: relative; color: #ffffff; background-color: #16a34a; font-size: 14px; font-weight: 600; padding: 10px 20px; margin: 0px; border: 1px solid #15803d",
      '- Accessibility: role="button", aria-label="Save current form changes", name="Save Changes"',
      "- HTML:",
      '<button id="save-changes" class="btn btn-success">Save Changes</button>',
    ].join("\n");

    expect(result).toBe(expected);
  });
});
