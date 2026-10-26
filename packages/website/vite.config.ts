import fs from "node:fs";
import path from "node:path";
import { defineConfig, loadEnv, type UserConfig } from "vite";
import tsConfigPaths from "vite-tsconfig-paths";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import { cloudflare } from "@cloudflare/vite-plugin";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { languageAlternates, LOCALES, localizePath } from "./src/i18n/locale";

const repoRoot = path.resolve(__dirname, "../..");
const DEFAULT_SITE_HOST = "https://xenonesis.github.io/Kronos";

/**
 * The site is served from a GitHub Pages subpath, so the production build has to
 * emit `/Kronos/`-prefixed asset and route URLs. The e2e suite, however, builds
 * and previews the same app at the server root, where a `/Kronos/` base makes
 * every route 404 and every link point at a path the test server does not
 * serve. The Pages build runs in `pages` mode, which loads `.env.pages` and
 * sets `VITE_BASE_PATH`; every other build gets the root.
 */
function resolveBasePath(env: Record<string, string>): string {
  return env.VITE_BASE_PATH ?? "/";
}

const siteHost = DEFAULT_SITE_HOST;
const routesDir = path.join(__dirname, "src/routes/{-$locale}");
// A translation such as `cli.zh-CN.md` is the same page as `cli.md` in another language.
const TRANSLATION_FILE = /\.[A-Za-z]{2,3}(?:-[A-Za-z0-9]+)*\.md$/;

function discoverDocsRoutes(): string[] {
  const docsDir = path.join(repoRoot, "public-docs");
  if (!fs.existsSync(docsDir)) return ["/docs"];
  const routes = new Set<string>(["/docs"]);
  const walk = (dir: string) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(full);
        continue;
      }
      if (!entry.name.endsWith(".md") || TRANSLATION_FILE.test(entry.name)) continue;
      const rel = path
        .relative(docsDir, full)
        .replace(/\.md$/, "")
        .replace(/\/index$/, "");
      if (rel === "index" || rel === "") continue;
      routes.add(`/docs/${rel.split(path.sep).join("/")}`);
    }
  };
  walk(docsDir);
  return [...routes].sort();
}

function discoverAgentRoutes(): string[] {
  const reserved = new Set([
    "route",
    "agents",
    "blog",
    "changelog",
    "docs",
    "download",
    "download_.thanks",
    "hub",
    "index",
    "sponsor",
    "privacy",
    "terms",
  ]);
  return fs
    .readdirSync(routesDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".tsx"))
    .map((entry) => entry.name.replace(/\.tsx$/, ""))
    .filter((name) => !reserved.has(name))
    .sort()
    .map((slug) => `/${slug}`);
}

function discoverAlternativeRoutes(): string[] {
  const alternativesDir = path.join(routesDir, "alternatives");
  if (!fs.existsSync(alternativesDir)) return [];
  const slugs = fs
    .readdirSync(alternativesDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".tsx"))
    .map((entry) => entry.name.replace(/\.tsx$/, ""))
    .sort();
  return ["/alternatives", ...slugs.map((slug) => `/alternatives/${slug}`)];
}

function discoverBlogRoutes(): string[] {
  const postsDir = path.join(__dirname, "posts");
  if (!fs.existsSync(postsDir)) return ["/blog"];
  const slugs = fs
    .readdirSync(postsDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".md"))
    .filter((entry) => !TRANSLATION_FILE.test(entry.name))
    .map((entry) => entry.name.replace(/\.md$/, ""))
    .sort();
  return ["/blog", ...slugs.map((slug) => `/blog/${slug}`)];
}

// Author and detail URLs are served by /sitemap-plugins.xml as the registry changes.
function discoverPluginRoutes(): string[] {
  return ["/plugins"];
}

const sitemapPages = [
  "/",
  "/agents",
  "/changelog",
  "/download",
  "/hub",
  "/privacy",
  "/terms",
  ...discoverAgentRoutes(),
  ...discoverAlternativeRoutes(),
  ...discoverPluginRoutes(),
  ...discoverDocsRoutes(),
  ...discoverBlogRoutes(),
].flatMap((routePath) => {
  // Every page exists in each language; each URL lists all of them as alternates.
  const alternateRefs = languageAlternates(routePath).map((alternate) => ({
    hreflang: alternate.hreflang,
    href: `${siteHost}${alternate.path}`,
  }));
  return LOCALES.map((locale) => ({
    path: localizePath(routePath, locale),
    sitemap: { alternateRefs },
  }));
});

export default defineConfig(({ mode }): UserConfig => {
  const env = loadEnv(mode, __dirname, "");
  const activeSiteHost = env.VITE_SITE_HOST || DEFAULT_SITE_HOST;
  return {
    base: resolveBasePath(env),
    server: {
      host: "0.0.0.0",
      port: 8082,
      strictPort: false,
      fs: {
        allow: [repoRoot],
      },
      watch: {
        ignored: ["**/.tanstack/**"],
      },
    },
    plugins: [
      cloudflare({ viteEnvironment: { name: "ssr" } }),
      tsConfigPaths({ projects: [path.join(__dirname, "tsconfig.json")] }),
      tanstackStart({
        router: {
          quoteStyle: "double",
          semicolons: true,
        },
        prerender: {
          // Only the static Pages build prerenders. The worker build renders
          // per request, and prerendering there would cost a full crawl for
          // output nothing reads.
          enabled: mode === "pages",
          failOnError: false,
          // The thanks page renders the file the query string names, so a
          // prerendered copy would carry whichever file the crawler happened to
          // land on. It is a one-shot download hand-off, not a page worth
          // prerendering.
          filter: (page: { path: string }) => !page.path.startsWith("/download/thanks"),
        },
        pages: sitemapPages,
        sitemap: {
          host: activeSiteHost,
        },
      }),
      react(),
      tailwindcss(),
    ],
  };
});
