import type { Request, RequestHandler, Response } from "express";
import http from "node:http";
import https from "node:https";
import { URL } from "node:url";
import type { Logger } from "pino";
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

function buildOutboundCookieHeader(req: Request, parsedUrl: URL): string | undefined {
  // Attach custom imported cookies for this domain if present
  const customCookies = getDomainCookies(parsedUrl.host) || getDomainCookies(parsedUrl.hostname);
  if (!customCookies) {
    return req.headers.cookie;
  }
  return `${req.headers.cookie ? `${req.headers.cookie}; ` : ""}${customCookies}`;
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

export function createBrowserProxyHandler(logger?: Logger): RequestHandler {
  return (req: Request, res: Response): void => {
    // Cookie management and local browser auto-import endpoint
    if (handleCookieRequest(req, res)) {
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
    const outboundCookie = buildOutboundCookieHeader(req, parsedUrl);

    const proxyRequest = client.request(
      parsedUrl,
      {
        method: req.method,
        headers: {
          ...req.headers,
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
