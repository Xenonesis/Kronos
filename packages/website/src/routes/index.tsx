import { createFileRoute } from "@tanstack/react-router";
import { KronosLandingPage } from "~/components/kronos-landing-page";
import { pageMeta } from "~/meta";

export const Route = createFileRoute("/")({
  head: () =>
    pageMeta(
      "Kronos – The Apex AI Developer Workstation & Autonomous Agent Engine",
      "High-performance native Rust AI workstation for Claude Code, Codex, OpenCode, Pi, and OMP swarms. Built with Tauri v2 under 100MB RAM. Zero Electron bloat. 1-click self-updating setup.",
      "/",
    ),
  component: KronosLandingPage,
});
