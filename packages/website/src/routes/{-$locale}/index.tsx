import { createFileRoute } from "@tanstack/react-router";
import { KronosLandingPage } from "~/components/kronos-landing-page";
import { routeLocale } from "~/i18n";
import { pageMeta } from "~/meta";

export const Route = createFileRoute("/{-$locale}/")({
  head: ({ params }) => {
    const locale = routeLocale(params);
    return pageMeta({
      title: "Kronos – The Apex AI Developer Workstation & Autonomous Agent Engine",
      description: "High-performance native Rust AI workstation for Claude Code, Codex, OpenCode, Pi, and OMP swarms. Built with Tauri v2 under 100MB RAM. Zero Electron bloat. 1-click self-updating setup.",
      path: "/",
      locale,
    });
  },
  component: KronosLandingPage,
});
