import { describe, expect, it } from "vitest";
import { parseHtmlAssetLinks, resolveRelativeAssetPath } from "./use-inlined-html-assets";

describe("resolveRelativeAssetPath", () => {
  it("resolves sibling files correctly", () => {
    expect(resolveRelativeAssetPath("website/index.html", "styles.css")).toBe("website/styles.css");
    expect(resolveRelativeAssetPath("website/index.html", "./styles.css")).toBe(
      "website/styles.css",
    );
    expect(resolveRelativeAssetPath("index.html", "styles.css")).toBe("styles.css");
  });

  it("resolves parent and nested paths correctly", () => {
    expect(resolveRelativeAssetPath("website/pages/index.html", "../styles.css")).toBe(
      "website/styles.css",
    );
    expect(resolveRelativeAssetPath("website/index.html", "css/sub/styles.css")).toBe(
      "website/css/sub/styles.css",
    );
  });

  it("ignores external schemes and protocol-relative URLs", () => {
    expect(
      resolveRelativeAssetPath("website/index.html", "https://cdn.example.com/styles.css"),
    ).toBeNull();
    expect(
      resolveRelativeAssetPath("website/index.html", "http://cdn.example.com/styles.css"),
    ).toBeNull();
    expect(
      resolveRelativeAssetPath("website/index.html", "//cdn.example.com/styles.css"),
    ).toBeNull();
    expect(resolveRelativeAssetPath("website/index.html", "#anchor")).toBeNull();
  });

  it("strips query params and hashes from paths", () => {
    expect(resolveRelativeAssetPath("website/index.html", "styles.css?v=123#test")).toBe(
      "website/styles.css",
    );
  });
});

describe("parseHtmlAssetLinks", () => {
  it("extracts stylesheets, scripts, and images", () => {
    const html = `
      <!DOCTYPE html>
      <html>
      <head>
        <link rel="stylesheet" href="styles.css">
        <link href='theme.css' rel='stylesheet'>
        <link rel="icon" href="favicon.ico">
      </head>
      <body>
        <img src="assets/logo.png" alt="logo">
        <script src="leads.js"></script>
        <script type="module" src="app.js"></script>
      </body>
      </html>
    `;

    const parsed = parseHtmlAssetLinks(html, "website/index.html");

    expect(parsed.stylesheets).toHaveLength(2);
    expect(parsed.stylesheets[0]?.sourcePath).toBe("styles.css");
    expect(parsed.stylesheets[0]?.resolvedPath).toBe("website/styles.css");
    expect(parsed.stylesheets[1]?.sourcePath).toBe("theme.css");
    expect(parsed.stylesheets[1]?.resolvedPath).toBe("website/theme.css");

    expect(parsed.scripts).toHaveLength(2);
    expect(parsed.scripts[0]?.sourcePath).toBe("leads.js");
    expect(parsed.scripts[0]?.resolvedPath).toBe("website/leads.js");
    expect(parsed.scripts[1]?.sourcePath).toBe("app.js");
    expect(parsed.scripts[1]?.resolvedPath).toBe("website/app.js");

    expect(parsed.images).toHaveLength(1);
    expect(parsed.images[0]?.sourcePath).toBe("assets/logo.png");
    expect(parsed.images[0]?.resolvedPath).toBe("website/assets/logo.png");
  });
});
