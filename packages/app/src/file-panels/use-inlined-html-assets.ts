import { useEffect, useState } from "react";
import type { DaemonClient } from "@getpaseo/client/internal/daemon-client";

export interface UseInlinedHtmlAssetsInput {
  html: string;
  cwd?: string | null;
  filePath?: string | null;
  client?: DaemonClient | null;
}

interface ParsedAsset {
  rawTag: string;
  sourcePath: string;
  resolvedPath: string;
}

interface ParsedHtmlAssets {
  stylesheets: ParsedAsset[];
  scripts: ParsedAsset[];
  images: ParsedAsset[];
}

const MAX_SCRIPT_INLINE_BYTES = 25 * 1024 * 1024; // 25 MB max per script
const MAX_IMAGE_INLINE_BYTES = 10 * 1024 * 1024; // 10 MB max per image

// Shared in-memory cache across tab switches and preview renders
const assetCache = new Map<string, { content: string; isDataUri?: boolean }>();

export function resolveRelativeAssetPath(baseFilePath: string, assetHref: string): string | null {
  if (/^(?:[a-z]+:|\/\/|#)/i.test(assetHref)) {
    return null;
  }
  const clean = assetHref.split(/[?#]/)[0]?.trim();
  if (!clean) {
    return null;
  }

  const normalizedBase = baseFilePath.replace(/\\/g, "/");
  const lastSlash = normalizedBase.lastIndexOf("/");
  const dir = lastSlash >= 0 ? normalizedBase.slice(0, lastSlash) : "";
  const combined = dir ? `${dir}/${clean}` : clean;
  const parts = combined.split("/");
  const stack: string[] = [];

  for (const part of parts) {
    if (!part || part === ".") {
      continue;
    }
    if (part === "..") {
      stack.pop();
    } else {
      stack.push(part);
    }
  }

  return stack.join("/");
}

export function parseHtmlAssetLinks(html: string, baseFilePath: string): ParsedHtmlAssets {
  const stylesheets: ParsedAsset[] = [];
  const scripts: ParsedAsset[] = [];
  const images: ParsedAsset[] = [];

  // Match <link ...> stylesheet tags
  const linkRegex = /<link\b([^>]*?)>/gi;
  let match: RegExpExecArray | null = null;
  while ((match = linkRegex.exec(html)) !== null) {
    const rawTag = match[0];
    const attrs = match[1] ?? "";
    if (!/\brel\s*=\s*["']?stylesheet["']?/i.test(attrs)) {
      continue;
    }
    const hrefMatch = attrs.match(/\bhref\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))/i);
    const href = hrefMatch ? hrefMatch[1] || hrefMatch[2] || hrefMatch[3] : null;
    if (href) {
      const resolvedPath = resolveRelativeAssetPath(baseFilePath, href);
      if (resolvedPath) {
        stylesheets.push({ rawTag, sourcePath: href, resolvedPath });
      }
    }
  }

  // Match <script ... src="..."> tags
  const scriptRegex =
    /<script\b([^>]*?)\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))([^>]*?)>(?:\s*<\/script>)?/gi;
  while ((match = scriptRegex.exec(html)) !== null) {
    const rawTag = match[0];
    const src = match[2] || match[3] || match[4];
    if (src) {
      const resolvedPath = resolveRelativeAssetPath(baseFilePath, src);
      if (resolvedPath) {
        scripts.push({ rawTag, sourcePath: src, resolvedPath });
      }
    }
  }

  // Match <img ... src="..."> tags
  const imgRegex = /<img\b([^>]*?)\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))([^>]*?)>/gi;
  while ((match = imgRegex.exec(html)) !== null) {
    const rawTag = match[0];
    const src = match[2] || match[3] || match[4];
    if (src) {
      const resolvedPath = resolveRelativeAssetPath(baseFilePath, src);
      if (resolvedPath) {
        images.push({ rawTag, sourcePath: src, resolvedPath });
      }
    }
  }

  return { stylesheets, scripts, images };
}

function uint8ArrayToBase64(bytes: Uint8Array): string {
  let binary = "";
  const len = bytes.byteLength;
  const chunkSize = 8192;
  for (let i = 0; i < len; i += chunkSize) {
    const chunk = bytes.subarray(i, Math.min(i + chunkSize, len));
    binary += String.fromCharCode.apply(null, chunk as unknown as number[]);
  }
  return btoa(binary);
}

function applyInlinedAssets(html: string, cwd: string, parsed: ParsedHtmlAssets): string {
  let result = html;

  for (const sheet of parsed.stylesheets) {
    const cached = assetCache.get(`${cwd}:${sheet.resolvedPath}`);
    if (cached) {
      result = result.replace(
        sheet.rawTag,
        () => `<style data-inlined="${sheet.sourcePath}">\n${cached.content}\n</style>`,
      );
    }
  }

  for (const sc of parsed.scripts) {
    const cached = assetCache.get(`${cwd}:${sc.resolvedPath}`);
    if (cached) {
      result = result.replace(
        sc.rawTag,
        () => `<script data-inlined="${sc.sourcePath}">\n${cached.content}\n</script>`,
      );
    }
  }

  for (const img of parsed.images) {
    const cached = assetCache.get(`${cwd}:${img.resolvedPath}`);
    if (cached?.isDataUri) {
      const updatedTag = img.rawTag.replace(
        /\bsrc\s*=\s*(?:"[^"]+"|'[^']+'|[^\s>]+)/i,
        () => `src="${cached.content}"`,
      );
      result = result.replace(img.rawTag, () => updatedTag);
    }
  }

  return result;
}

export function useInlinedHtmlAssets({
  html,
  cwd,
  filePath,
  client,
}: UseInlinedHtmlAssetsInput): string {
  const [inlinedHtml, setInlinedHtml] = useState(() => {
    if (!cwd || !filePath || !client) {
      return html;
    }
    const parsed = parseHtmlAssetLinks(html, filePath);
    return applyInlinedAssets(html, cwd, parsed);
  });

  useEffect(() => {
    if (!cwd || !filePath || !client) {
      setInlinedHtml(html);
      return;
    }

    let isDisposed = false;
    const parsed = parseHtmlAssetLinks(html, filePath);

    // Initial pass with whatever is already cached
    const initiallyInlined = applyInlinedAssets(html, cwd, parsed);
    setInlinedHtml(initiallyInlined);

    const pendingAssets: Array<{
      resolvedPath: string;
      kind: "stylesheet" | "script" | "image";
    }> = [];

    for (const sheet of parsed.stylesheets) {
      if (!assetCache.has(`${cwd}:${sheet.resolvedPath}`)) {
        pendingAssets.push({ resolvedPath: sheet.resolvedPath, kind: "stylesheet" });
      }
    }
    for (const sc of parsed.scripts) {
      if (!assetCache.has(`${cwd}:${sc.resolvedPath}`)) {
        pendingAssets.push({ resolvedPath: sc.resolvedPath, kind: "script" });
      }
    }
    for (const img of parsed.images) {
      if (!assetCache.has(`${cwd}:${img.resolvedPath}`)) {
        pendingAssets.push({ resolvedPath: img.resolvedPath, kind: "image" });
      }
    }

    if (pendingAssets.length === 0) {
      return;
    }

    const loadAssets = async () => {
      await Promise.all(
        pendingAssets.map(async (asset) => {
          try {
            let maxBytes: number | undefined;
            if (asset.kind === "script") {
              maxBytes = MAX_SCRIPT_INLINE_BYTES;
            } else if (asset.kind === "image") {
              maxBytes = MAX_IMAGE_INLINE_BYTES;
            }

            const fileResult = await client.readFile(cwd, asset.resolvedPath, undefined, maxBytes);

            if (isDisposed) {
              return;
            }

            if (asset.kind === "image") {
              const mime = fileResult.mime || "image/png";
              const base64 = uint8ArrayToBase64(fileResult.bytes);
              assetCache.set(`${cwd}:${asset.resolvedPath}`, {
                content: `data:${mime};base64,${base64}`,
                isDataUri: true,
              });
            } else {
              const text = new TextDecoder().decode(fileResult.bytes);
              assetCache.set(`${cwd}:${asset.resolvedPath}`, {
                content: text,
              });
            }
          } catch {
            // Gracefully ignore missing or inaccessible relative files
          }
        }),
      );
      if (!isDisposed) {
        setInlinedHtml(applyInlinedAssets(html, cwd, parsed));
      }
    };
    void loadAssets();
    return () => {
      isDisposed = true;
    };
  }, [html, cwd, filePath, client]);

  return inlinedHtml;
}
