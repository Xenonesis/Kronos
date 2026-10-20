import fs from "node:fs";
import path from "node:path";
import { defineConfig, loadEnv, type UserConfig } from "vite";
import tsConfigPaths from "vite-tsconfig-paths";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import { cloudflare } from "@cloudflare/vite-plugin";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

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
      if (!entry.name.endsWith(".md")) continue;
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
  const routesDir = path.join(__dirname, "src/routes");
  const reserved = new Set([
    "__root",
    "agents",
    "blog",
    "changelog",
    "docs",
    "download",
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
  const alternativesDir = path.join(__dirname, "src/routes/alternatives");
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
].map((routePath) => ({
  path: routePath,
}));

export default defineConfig(({ mode }): UserConfig => {
  const env = loadEnv(mode, __dirname, "");
  const siteHost = env.VITE_SITE_HOST || DEFAULT_SITE_HOST;
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
          enabled: true,
          failOnError: false,
        },
        pages: sitemapPages,
        sitemap: {
          host: siteHost,
        },
      }),
      react(),
      tailwindcss(),
    ],
  };
});
