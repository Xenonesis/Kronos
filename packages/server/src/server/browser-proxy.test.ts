import { afterEach, describe, expect, it } from "vitest";
import express from "express";
import http from "node:http";
import type { AddressInfo } from "node:net";
import {
  createBrowserProxyHandler,
  injectBaseTagIntoHtml,
  sanitizeProxyHeaders,
} from "./browser-proxy";

const servers: http.Server[] = [];

afterEach(async () => {
  await Promise.all(
    servers.splice(0).map((server) => {
      const { promise, resolve, reject } = Promise.withResolvers<void>();
      server.close((error) => (error ? reject(error) : resolve()));
      server.closeAllConnections();
      return promise;
    }),
  );
});

async function listen(server: http.Server): Promise<string> {
  servers.push(server);
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
  await promise;
  return `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
}

async function navigationFixture(password?: string) {
  const requests: Array<{ path: string; method: string; headers: http.IncomingHttpHeaders }> = [];
  let onceRequests = 0;
  const upstream = await listen(
    http.createServer((req, res) => {
      requests.push({ path: req.url!, method: req.method!, headers: req.headers });
      if (req.url === "/redirect") {
        res.writeHead(302, { location: "/once" }).end();
      } else if (req.url === "/once") {
        res.setHeader("set-cookie", "website_cookie=not-a-daemon-cookie");
        onceRequests += 1;
        res.writeHead(onceRequests === 1 ? 200 : 410, { "content-type": "text/html" });
        res.end("<html><head></head><body>single-use page</body></html>");
      } else if (req.url === "/echo") {
        const chunks: Buffer[] = [];
        req.on("data", (chunk: Buffer) => chunks.push(chunk));
        req.on("end", () => {
          res.writeHead(200, { "content-type": "text/plain" });
          res.end(Buffer.concat(chunks));
        });
      } else {
        res.writeHead(200, { "content-type": "application/json" });
        res.end(
          JSON.stringify({
            authorization: req.headers.authorization ?? null,
            cookie: req.headers.cookie ?? null,
          }),
        );
      }
    }),
  );
  let localCredential: string | null = "native-local-credential";
  const app = express();
  // Like bootstrap, the proxy owns its native operation's JSON/auth handling.
  app.use(
    "/api/browser-proxy",
    createBrowserProxyHandler(undefined, { password, localCredential: () => localCredential }),
  );
  const daemon = await listen(http.createServer(app));
  const issue = (url: string, authorization = "Bearer native-local-credential") =>
    fetch(`${daemon}/api/browser-proxy/navigation`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization,
        cookie: "daemon_secret=private",
      },
      body: JSON.stringify({ url }),
    });
  const importCookies = (url: string, cookies: string) =>
    fetch(`${daemon}/api/browser-proxy/cookies`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: "Bearer native-local-credential",
      },
      body: JSON.stringify({ host: new URL(url).host, cookies }),
    });
  return {
    daemon,
    upstream,
    requests,
    issue,
    importCookies,
    rotateCredential: () => {
      localCredential = null;
    },
  };
}

describe("native browser navigation on listening HTTP servers", () => {
  it("rejects missing and incorrect credentials without treating desktop Origin as authentication", async () => {
    const fixture = await navigationFixture();
    for (const authorization of ["", "Bearer incorrect"]) {
      const response = await fetch(`${fixture.daemon}/api/browser-proxy/navigation`, {
        method: "POST",
        headers: {
          authorization,
          origin: "http://tauri.localhost",
          "content-type": "application/json",
        },
        body: JSON.stringify({ url: `${fixture.upstream}/once` }),
      });
      expect(response.status).toBe(401);
    }
    expect(fixture.requests).toEqual([]);
  });

  it("accepts a configured password and fails closed after the local credential is withdrawn", async () => {
    const fixture = await navigationFixture(
      "$2b$12$OLxyuuP9uLK30Uzc4wQX0O6liuU/Q1t5P2b0Ebf36mULvpVK3DRZW",
    );
    expect(
      (await fixture.issue(`${fixture.upstream}/headers`, "Bearer correct-password")).status,
    ).toBe(200);
    expect(
      (await fixture.issue(`${fixture.upstream}/headers`, "Bearer wrong-password")).status,
    ).toBe(401);
    fixture.rotateCredential();
    expect((await fixture.issue(`${fixture.upstream}/headers`)).status).toBe(401);
  });

  it("retains the final single-use GET behind an exact-target GET-only single-use ticket", async () => {
    const fixture = await navigationFixture();
    const issued = await fixture.issue(`${fixture.upstream}/once`);
    expect(issued.status).toBe(200);
    const capability = (await issued.json()).url as string;
    const ticketUrl = new URL(capability, fixture.daemon);
    expect(ticketUrl.pathname).toBe("/api/browser-proxy");
    expect(ticketUrl.searchParams.get("url")).toBe(`${fixture.upstream}/once`);
    expect(ticketUrl.searchParams.get("ticket")).toMatch(/^[A-Za-z0-9_-]{43}$/);
    const tampered = new URL(ticketUrl);
    tampered.searchParams.set("url", `${fixture.upstream}/headers`);
    expect((await fetch(tampered)).status).toBe(403);
    expect((await fetch(ticketUrl, { method: "HEAD" })).status).toBe(405);
    const page = await fetch(ticketUrl);
    expect(page.status).toBe(200);
    expect(page.headers.get("cache-control")).toBe("no-store");
    expect(page.headers.get("set-cookie")).toBeNull();
    expect(await page.text()).toContain("single-use page");
    expect((await fetch(ticketUrl)).status).toBe(403);
    expect(fixture.requests.map(({ path, method }) => ({ path, method }))).toEqual([
      { path: "/once", method: "GET" },
    ]);
  });

  it("follows redirects only during authenticated issuance without refetching the final page", async () => {
    const fixture = await navigationFixture();
    const issued = await fixture.issue(`${fixture.upstream}/redirect`);
    expect(issued.status).toBe(200);
    const ticketUrl = new URL((await issued.json()).url, fixture.daemon);
    expect(ticketUrl.searchParams.get("url")).toBe(`${fixture.upstream}/once`);
    const page = await fetch(ticketUrl);
    expect(page.status).toBe(200);
    expect(await page.text()).toContain(`<base href="${fixture.upstream}/once">`);
    expect(fixture.requests.map(({ path }) => path)).toEqual(["/redirect", "/once"]);
  });

  it("does not let a page ticket issue cross-target capabilities or import/read another host's cookies", async () => {
    const first = await navigationFixture();
    const other = await navigationFixture();
    expect((await first.importCookies(other.upstream, "other_session=secret")).status).toBe(200);
    const issued = await first.issue(`${first.upstream}/headers`);
    const ticketUrl = new URL((await issued.json()).url, first.daemon);
    const ticket = ticketUrl.searchParams.get("ticket")!;
    const crossTarget = new URL(ticketUrl);
    crossTarget.searchParams.set("url", `${other.upstream}/headers`);
    expect((await fetch(crossTarget)).status).toBe(403);
    const mint = await fetch(`${first.daemon}/api/browser-proxy/navigation?ticket=${ticket}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ url: `${other.upstream}/headers` }),
    });
    expect(mint.status).toBe(403);
    const mutate = await fetch(`${first.daemon}/api/browser-proxy/cookies?ticket=${ticket}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ host: new URL(other.upstream).host, cookies: "overwritten=yes" }),
    });
    expect(mutate.status).toBe(403);
    expect(other.requests).toEqual([]);
    const page = await fetch(ticketUrl);
    expect(await page.json()).toEqual({ authorization: null, cookie: null });
    expect((await first.issue(`${other.upstream}/headers`)).status).toBe(200);
    expect(other.requests[0]?.headers.cookie).toBe("other_session=secret");
  });

  it("imports supplied website cookies on the listening process without forwarding daemon headers or cookies", async () => {
    const fixture = await navigationFixture();
    const imported = await fixture.importCookies(fixture.upstream, "website_session=actual-value");
    expect(await imported.json()).toMatchObject({ ok: true, count: 1 });
    const issued = await fixture.issue(`${fixture.upstream}/headers`);
    const page = await fetch(new URL((await issued.json()).url, fixture.daemon), {
      headers: { authorization: "Bearer native-local-credential", cookie: "daemon_secret=private" },
    });
    expect(await page.json()).toEqual({
      authorization: null,
      cookie: "website_session=actual-value",
    });
    expect(fixture.requests[0]?.headers.origin).toBeUndefined();
  });

  it("rejects an oversized retained response without issuing a capability", async () => {
    const fixture = await navigationFixture();
    const large = await listen(
      http.createServer((_req, res) => {
        res.end(Buffer.alloc(8 * 1024 * 1024 + 1, "x"));
      }),
    );
    const response = await fixture.issue(large);
    expect(response.status).toBe(502);
    expect(await response.json()).toMatchObject({ error: "Failed to load target website" });
  });

  it("rejects non-HTTP and credential-bearing targets before making upstream requests", async () => {
    const fixture = await navigationFixture();
    for (const url of [
      "file:///etc/passwd",
      "ftp://example.com/data",
      "not a url",
      "http://user:password@127.0.0.1/",
    ]) {
      expect((await fixture.issue(url)).status).toBe(400);
    }
    expect(fixture.requests).toEqual([]);
  });

  it("bounds outstanding tickets and frees a slot when a ticket is redeemed", async () => {
    const fixture = await navigationFixture();
    const capabilities: string[] = [];
    for (let index = 0; index < 8; index += 1) {
      const response = await fixture.issue(`${fixture.upstream}/headers`);
      expect(response.status).toBe(200);
      capabilities.push((await response.json()).url);
    }
    expect((await fixture.issue(`${fixture.upstream}/headers`)).status).toBe(429);
    expect(fixture.requests).toHaveLength(8);
    expect((await fetch(new URL(capabilities[0]!, fixture.daemon))).status).toBe(200);
    expect((await fixture.issue(`${fixture.upstream}/headers`)).status).toBe(200);
    expect(fixture.requests).toHaveLength(9);
  });

  it("preserves bare-query legacy requests but never forwards daemon authorization or cookies", async () => {
    const fixture = await navigationFixture();
    const response = await fetch(
      `${fixture.daemon}/api/browser-proxy?url=${encodeURIComponent(`${fixture.upstream}/headers`)}`,
      {
        headers: {
          authorization: "Bearer native-local-credential",
          cookie: "daemon_secret=private",
        },
      },
    );
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ authorization: null, cookie: null });
  });

  it("keeps legacy upstream JSON POST bodies intact", async () => {
    const fixture = await navigationFixture();
    const body = JSON.stringify({ website: "form", value: 42 });
    const response = await fetch(
      `${fixture.daemon}/api/browser-proxy?url=${encodeURIComponent(`${fixture.upstream}/echo`)}`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body,
      },
    );
    expect(response.status).toBe(200);
    expect(await response.text()).toBe(body);
    expect(fixture.requests[0]?.method).toBe("POST");
  });
});

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
