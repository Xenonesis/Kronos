import { json, type Request, type RequestHandler, type Response } from "express";
import { randomBytes } from "node:crypto";
import http from "node:http";
import https from "node:https";
import { URL } from "node:url";
import type { Logger } from "pino";
import { type DaemonAuthConfig, extractHttpBearerToken, isBearerTokenValidAsync } from "./auth.js";
import { matchesLocalCredential } from "./local-credential.js";
import { readChromiumCookiesForHost } from "./browser-tools/chromium-cookie-paths.js";
const DESKTOP_USER_AGENT =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

// Host-keyed session cookie jar for imported or remembered cookies
const sessionCookieJar = new Map<string, string>();

export function setDomainCookies(host: string, cookies: string): void {
  sessionCookieJar.set(host.toLowerCase(), cookies);
}

export function getDomainCookies(host: string): string | undefined {
  return sessionCookieJar.get(host.toLowerCase());
}

export function sanitizeProxyHeaders(
  rawHeaders: http.IncomingHttpHeaders,
): Record<string, string | string[] | undefined> {
  const headers: Record<string, string | string[] | undefined> = { ...rawHeaders };

  // Remove framing restrictions
  delete headers["x-frame-options"];
  delete headers["X-Frame-Options"];
  delete headers["content-security-policy-report-only"];

  if (headers["content-security-policy"]) {
    const csp = String(headers["content-security-policy"]);
    headers["content-security-policy"] = csp.replace(/frame-ancestors[^;]+;?\s*/gi, "").trim();
  }

  // Set permissive CORS
  headers["access-control-allow-origin"] = "*";
  headers["access-control-allow-methods"] = "GET, POST, OPTIONS, HEAD";
  headers["access-control-allow-headers"] = "*";

  return headers;
}

export function injectBaseTagIntoHtml(html: string, targetUrl: string): string {
  const urlObj = new URL(targetUrl);
  const baseHref = urlObj.toString();

  // Strip meta frame-busting tags
  let cleaned = html.replace(/<meta[^>]+http-equiv=["']X-Frame-Options["'][^>]*>/gi, "");

  // Inject <base href="..."> into <head>
  if (/<head[^>]*>/i.test(cleaned)) {
    return cleaned.replace(/(<head[^>]*>)/i, `$1\n<base href="${baseHref}">`);
  }

  // Fallback if no head tag exists
  return `<base href="${baseHref}">\n${cleaned}`;
}

interface CookieRequestBody {
  host?: string;
  cookies?: string;
  browser?: "chrome" | "edge" | "brave";
}

/** Handles the cookie import/remember endpoint. Returns true when the request was answered. */
function handleCookieRequest(req: Request, res: Response): boolean {
  if (req.method !== "POST" || !req.path.endsWith("/cookies")) {
    return false;
  }

  const { host, cookies, browser } = (req.body as CookieRequestBody) || {};
  if (browser && host) {
    const imported = readChromiumCookiesForHost(browser, host);
    if (imported.cookies) {
      setDomainCookies(host, imported.cookies);
    }
    res.json({ ok: true, host, count: imported.count, browser });
    return true;
  }
  if (host && typeof cookies === "string") {
    setDomainCookies(host, cookies);
    res.json({ ok: true, host, count: cookies.split(";").length });
    return true;
  }
  res.status(400).json({ error: "Missing host, cookies, or browser in body" });
  return true;
}

function parseProxyTargetUrl(rawTarget: string): URL | null {
  const normalized =
    rawTarget.startsWith("http://") || rawTarget.startsWith("https://")
      ? rawTarget
      : `https://${rawTarget}`;
  try {
    return new URL(normalized);
  } catch {
    return null;
  }
}

function buildOutboundCookieHeader(parsedUrl: URL): string | undefined {
  // Daemon-origin cookies are never website cookies. Only explicitly imported,
  // host-specific website sessions may cross this boundary.
  return getDomainCookies(parsedUrl.host) || getDomainCookies(parsedUrl.hostname);
}

const NAVIGATION_TICKET_TTL_MS = 30_000;
const MAX_NAVIGATION_TICKETS = 8;
const MAX_NAVIGATION_BODY_BYTES = 8 * 1024 * 1024;

interface NavigationResponse {
  url: URL;
  statusCode: number;
  headers: http.IncomingHttpHeaders;
  body: Buffer;
}

interface NavigationTicket {
  expiresAt: number;
  response: NavigationResponse;
}

async function resolveNavigationResponse(initialUrl: URL): Promise<NavigationResponse> {
  let url = initialUrl;
  const signal = AbortSignal.timeout(15_000);
  for (let redirects = 0; ; redirects += 1) {
    let resolve!: (value: http.IncomingMessage) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<http.IncomingMessage>((res, rej) => {
      resolve = res;
      reject = rej;
    });
    const cookies = buildOutboundCookieHeader(url);
    const request = (url.protocol === "https:" ? https : http).get(
      url,
      {
        signal,
        headers: {
          "user-agent": DESKTOP_USER_AGENT,
          accept: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
          "accept-encoding": "identity",
          ...(cookies ? { cookie: cookies } : {}),
        },
      },
      resolve,
    );
    request.on("error", reject);
    const response = await promise;
    const statusCode = response.statusCode ?? 200;
    if ([301, 302, 303, 307, 308].includes(statusCode) && response.headers.location) {
      response.destroy();
      if (redirects >= 10) {
        throw new Error("Too many website redirects");
      }
      url = new URL(response.headers.location, url);
      if (!["http:", "https:"].includes(url.protocol) || url.username || url.password) {
        throw new Error("Unsupported website redirect");
      }
      continue;
    }
    const chunks: Buffer[] = [];
    let length = 0;
    for await (const chunk of response) {
      const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      length += bytes.length;
      if (length > MAX_NAVIGATION_BODY_BYTES) {
        response.destroy();
        throw new Error("Website navigation response exceeds 8 MiB");
      }
      chunks.push(bytes);
    }
    return { url, statusCode, headers: response.headers, body: Buffer.concat(chunks, length) };
  }
}

function respondWithNavigation(response: NavigationResponse, res: Response): void {
  const headers = sanitizeProxyHeaders(response.headers);
  // Website Set-Cookie must not become a daemon-origin session cookie.
  delete headers["set-cookie"];
  delete headers["content-length"];
  delete headers["transfer-encoding"];
  headers["cache-control"] = "no-store";
  const isHtml = String(headers["content-type"] ?? "").includes("text/html");
  if (isHtml) {
    headers["content-type"] = "text/html; charset=utf-8";
  }
  res.writeHead(response.statusCode, headers);
  res.end(
    isHtml
      ? injectBaseTagIntoHtml(response.body.toString("utf8"), response.url.href)
      : response.body,
  );
}

/** Streams an HTML upstream response through the base-tag injector. */
function respondWithInjectedHtml(
  upstreamRes: http.IncomingMessage,
  res: Response,
  parsedUrl: URL,
  statusCode: number,
  sanitizedHeaders: Record<string, string | string[] | undefined>,
): void {
  const bodyChunks: Buffer[] = [];
  upstreamRes.on("data", (chunk: Buffer) => {
    bodyChunks.push(chunk);
  });

  upstreamRes.on("end", () => {
    const rawHtml = Buffer.concat(bodyChunks).toString("utf-8");
    const transformedHtml = injectBaseTagIntoHtml(rawHtml, parsedUrl.toString());

    delete sanitizedHeaders["content-length"];
    sanitizedHeaders["content-type"] = "text/html; charset=utf-8";

    res.writeHead(statusCode, sanitizedHeaders);
    res.end(transformedHtml);
  });
}

function handleUpstreamResponse(
  upstreamRes: http.IncomingMessage,
  res: Response,
  parsedUrl: URL,
): void {
  const statusCode = upstreamRes.statusCode || 200;

  // Handle redirects (301, 302, 303, 307, 308)
  if (statusCode >= 300 && statusCode < 400 && upstreamRes.headers.location) {
    try {
      const redirectTarget = new URL(upstreamRes.headers.location, parsedUrl).toString();
      res.redirect(`/api/browser-proxy?url=${encodeURIComponent(redirectTarget)}`);
      return;
    } catch {
      // If location is invalid, fall through
    }
  }

  const sanitizedHeaders = sanitizeProxyHeaders(upstreamRes.headers);
  const contentType = String(upstreamRes.headers["content-type"] || "");

  if (contentType.includes("text/html")) {
    respondWithInjectedHtml(upstreamRes, res, parsedUrl, statusCode, sanitizedHeaders);
    return;
  }

  res.writeHead(statusCode, sanitizedHeaders);
  upstreamRes.pipe(res);
}

export function createBrowserProxyHandler(
  logger?: Logger,
  auth?: DaemonAuthConfig,
): RequestHandler {
  // These limits also bound abandoned capabilities; expired entries are pruned
  // before issuance and cannot be redeemed. Pending fetches reserve a slot.
  const tickets = new Map<string, NavigationTicket>();
  let pendingNavigations = 0;
  const parseJson = json({ limit: "64kb" });

  return (req: Request, res: Response, next): void => {
    // A supplied ticket never falls through into the legacy unauthenticated
    // proxy/cookie routes, even when invalid, expired, or attached to another op.
    if (req.query.ticket !== undefined) {
      if (req.path !== "/" && req.path !== "") {
        res.status(403).json({ error: "Invalid navigation ticket" });
        return;
      }
      if (req.method !== "GET") {
        res.setHeader("Allow", "GET");
        res.status(405).json({ error: "Navigation tickets require GET" });
        return;
      }
      const token = req.query.ticket;
      const ticket = typeof token === "string" ? tickets.get(token) : undefined;
      if (!ticket || ticket.expiresAt <= Date.now() || req.query.url !== ticket.response.url.href) {
        if (ticket && ticket.expiresAt <= Date.now() && typeof token === "string") {
          tickets.delete(token);
        }
        res.status(403).json({ error: "Invalid navigation ticket" });
        return;
      }
      tickets.delete(token as string);
      respondWithNavigation(ticket.response, res);
      return;
    }

    if (req.path === "/navigation") {
      if (req.method !== "POST") {
        res.setHeader("Allow", "POST");
        res.status(405).json({ error: "Navigation issuance requires POST" });
        return;
      }
      void (async () => {
        const token = extractHttpBearerToken(req.header("authorization"));
        const localCredential = auth?.localCredential?.();
        const isLocal =
          token !== null && !!localCredential && matchesLocalCredential(localCredential, token);
        // Unlike the legacy proxy, issuance always needs a real credential.
        // The password helper intentionally allows missing configured passwords,
        // so call it only when a password is actually configured.
        const isPassword =
          !isLocal &&
          token !== null &&
          !!auth?.password &&
          (await isBearerTokenValidAsync({ password: auth.password, token }));
        if (!isLocal && !isPassword) {
          res.status(401).json({ error: "Unauthorized" });
          return;
        }
        parseJson(req, res, (error?: unknown) => {
          if (error) {
            next(error);
            return;
          }
          const rawTarget = req.body?.url;
          const url =
            typeof rawTarget === "string" && URL.canParse(rawTarget) ? new URL(rawTarget) : null;
          if (!url || !["http:", "https:"].includes(url.protocol) || url.username || url.password) {
            res.status(400).json({ error: "Invalid navigation URL" });
            return;
          }
          for (const [key, ticket] of tickets) {
            if (ticket.expiresAt <= Date.now()) tickets.delete(key);
          }
          if (tickets.size + pendingNavigations >= MAX_NAVIGATION_TICKETS) {
            res.status(429).json({ error: "Too many pending website navigations" });
            return;
          }
          pendingNavigations += 1;
          void resolveNavigationResponse(url).then(
            (response) => {
              pendingNavigations -= 1;
              if (res.destroyed) return;
              const ticket = randomBytes(32).toString("base64url");
              tickets.set(ticket, { expiresAt: Date.now() + NAVIGATION_TICKET_TTL_MS, response });
              res.setHeader("Cache-Control", "no-store");
              res.json({
                url: `/api/browser-proxy?url=${encodeURIComponent(response.url.href)}&ticket=${ticket}`,
              });
            },
            (error: Error) => {
              pendingNavigations -= 1;
              logger?.warn({ err: error.message, url: url.href }, "Browser navigation failed");
              if (!res.destroyed) {
                res
                  .status(502)
                  .json({ error: "Failed to load target website", details: error.message });
              }
            },
          );
        });
      })().catch(next);
      return;
    }

    // Parse only operation bodies, never a legacy upstream POST stream.
    if (req.method === "POST" && req.path.endsWith("/cookies")) {
      parseJson(req, res, (error?: unknown) => {
        if (error) next(error);
        else handleCookieRequest(req, res);
      });
      return;
    }

    const rawTarget = req.query.url;
    if (!rawTarget || typeof rawTarget !== "string") {
      res.status(400).json({ error: "Missing required 'url' query parameter" });
      return;
    }

    const parsedUrl = parseProxyTargetUrl(rawTarget);
    if (!parsedUrl) {
      res.status(400).json({ error: "Invalid URL parameter" });
      return;
    }

    const client = parsedUrl.protocol === "https:" ? https : http;
    const outboundCookie = buildOutboundCookieHeader(parsedUrl);
    const outboundHeaders = { ...req.headers };
    delete outboundHeaders.authorization;
    delete outboundHeaders["proxy-authorization"];
    delete outboundHeaders.cookie;
    delete outboundHeaders.origin;

    const proxyRequest = client.request(
      parsedUrl,
      {
        method: req.method,
        headers: {
          ...outboundHeaders,
          host: parsedUrl.host,
          "user-agent": DESKTOP_USER_AGENT,
          accept:
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
          "accept-encoding": "identity", // Disable gzip/br to allow easy <base> injection
          referer: parsedUrl.origin,
          ...(outboundCookie ? { cookie: outboundCookie } : {}),
        },
      },
      (upstreamRes) => {
        handleUpstreamResponse(upstreamRes, res, parsedUrl);
      },
    );

    proxyRequest.on("error", (err) => {
      logger?.warn(
        { err: err.message, url: parsedUrl.toString() },
        "Browser proxy upstream connection error",
      );
      res.status(502).json({ error: "Failed to connect to target website", details: err.message });
    });

    if (req.method !== "GET" && req.method !== "HEAD") {
      req.pipe(proxyRequest);
    } else {
      proxyRequest.end();
    }
  };
}
