import { describe, expect, it } from "vitest";
import { injectBaseTagIntoHtml, sanitizeProxyHeaders } from "./browser-proxy";

describe("sanitizeProxyHeaders", () => {
  it("strips x-frame-options and removes frame-ancestors from csp", () => {
    const rawHeaders = {
      "x-frame-options": "SAMEORIGIN",
      "X-Frame-Options": "DENY",
      "content-security-policy": "default-src 'self'; frame-ancestors 'none'; script-src *",
      "content-type": "text/html",
    };
    const sanitized = sanitizeProxyHeaders(rawHeaders);

    expect(sanitized["x-frame-options"]).toBeUndefined();
    expect(sanitized["X-Frame-Options"]).toBeUndefined();
    expect(sanitized["access-control-allow-origin"]).toBe("*");
    expect(sanitized["content-security-policy"]).toBe("default-src 'self'; script-src *");
  });
});

describe("injectBaseTagIntoHtml", () => {
  it("injects <base href> tag inside <head>", () => {
    const html =
      "<!DOCTYPE html><html><head><title>Test</title></head><body><h1>Hello</h1></body></html>";
    const result = injectBaseTagIntoHtml(html, "https://google.com/search");

    expect(result).toContain('<base href="https://google.com/search">');
    expect(result).toContain("<title>Test</title>");
  });

  it("removes X-Frame-Options meta tags", () => {
    const html =
      '<html><head><meta http-equiv="X-Frame-Options" content="DENY"></head><body>Hi</body></html>';
    const result = injectBaseTagIntoHtml(html, "https://example.com");

    expect(result).not.toContain('http-equiv="X-Frame-Options"');
    expect(result).toContain('<base href="https://example.com/">');
  });

  it("falls back to prepending <base> if no <head> exists", () => {
    const html = "<div>Bare HTML content</div>";
    const result = injectBaseTagIntoHtml(html, "https://example.com/app");

    expect(result.startsWith('<base href="https://example.com/app">')).toBe(true);
  });
});
