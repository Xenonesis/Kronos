import * as React from "react";
import { useState, useCallback } from "react";
import {
  Activity,
  ArrowRight,
  Boxes,
  Check,
  ChevronRight,
  Clock,
  Copy,
  Cpu,
  Download,
  Eye,
  Globe,
  History,
  RefreshCw,
  ShieldCheck,
  Sliders,
  Terminal,
  X,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { SiteHeader } from "~/components/site-header";
import { SiteFooter } from "~/components/site-footer";

const MOTION_FADE_DOWN = { opacity: 0, y: -10 };
const MOTION_FADE_UP = { opacity: 0, y: 15 };
const MOTION_FADE_IN = { opacity: 0 };
const MOTION_FADE_UP_LG = { opacity: 0, y: 30 };
const MOTION_VISIBLE = { opacity: 1, y: 0 };
const MOTION_VISIBLE_IN = { opacity: 1 };

const TRANSITION_DUR_05 = { duration: 0.5 };
const TRANSITION_DUR_06_D1 = { duration: 0.6, delay: 0.1 };
const TRANSITION_DUR_06_D2 = { duration: 0.6, delay: 0.2 };
const TRANSITION_DUR_06_D3 = { duration: 0.6, delay: 0.3 };
const TRANSITION_DUR_08_D4 = { duration: 0.8, delay: 0.4 };
const TRANSITION_DUR_08_D45 = { duration: 0.8, delay: 0.45 };

const FAQ_ITEMS = [
  {
    id: "faq-migration",
    q: "Can I migrate my existing Paseo workspaces and agents to Kronos?",
    a: "Yes, seamlessly! Kronos shares 100% backward compatibility with your existing ~/.paseo configurations, agent credentials, and local projects. Your previous agents and API keys are automatically recognized.",
  },
  {
    id: "faq-ram",
    q: "Why does Kronos consume so much less RAM than Paseo?",
    a: "Paseo ran on Electron, which bundles a full multi-process Chromium engine consuming 600MB–1.2GB RAM. Kronos runs on Tauri v2 with a pure Rust core and links directly to the operating system's native WebView2 compositor, reducing memory footprint to under 100MB.",
  },
  {
    id: "faq-update",
    q: "How does the 1-Click Update system work?",
    a: "We created a dedicated runner script (update-desktop-exe.bat). When upstream releases new agent features, running this script pulls the latest git commits, rebuilds internal protocol packages, exports the web app, and compiles a fresh NSIS setup executable automatically.",
  },
  {
    id: "faq-omp",
    q: "What is the OMP Sessions Explorer?",
    a: "OMP (Open Multi-Agent Protocol) sessions explorer provides first-class visual inspection of subagent execution trees. You can inspect exact diffs, tool call parameters, prompt histories, and roll back broken steps with visual feedback.",
  },
  {
    id: "faq-offline",
    q: "Does Kronos require a cloud connection or send telemetry?",
    a: "Zero external telemetry. Kronos is 100% local-first. All daemon sockets, terminal processes, and agent workspaces run entirely on your local machine.",
  },
];

const COMPARISON_ROWS = [
  {
    id: "dim-arch",
    title: "Desktop Architecture",
    subtitle: "Core executable & webview rendering engine",
    kronosTitle: "Tauri v2 + Native Rust Core",
    kronosDesc: "Uses native Windows WebView2. Slashing memory consumption to <100MB.",
    paseoTitle: "Legacy Electron Shell",
    paseoDesc: "Runs full bundled Chromium multi-process instance consuming 600MB–1.2GB RAM.",
  },
  {
    id: "dim-boot",
    title: "Startup Latency & Boot Speed",
    subtitle: "Time from click to interactive terminal",
    kronosTitle: "Instant Cold Boot (~380ms)",
    kronosDesc: "Rust binary wakes immediately with zero Chromium bootstrap lag.",
    paseoTitle: "Sluggish Boot (3.8s–5.5s)",
    paseoDesc: "Heavy JS preload scripts and Chromium GPU initialization wait times.",
  },
  {
    id: "dim-daemon",
    title: "Daemon Connection Resilience",
    subtitle: "Local Node daemon connection & socket stability",
    kronosTitle: "Daemon Health Gating",
    kronosDesc:
      'Window displays only after 200 OK health check. Dynamic persistent credentials eliminate "Host not connected" errors.',
    paseoTitle: "Race Condition & Flaky Drops",
    paseoDesc:
      'UI opens before daemon finishes socket binding, causing recurrent "Host is not connected" banners.',
  },
  {
    id: "dim-omp",
    title: "OMP Sessions & Timeline Explorer",
    subtitle: "Subagent execution inspection & step rollback",
    kronosTitle: "First-Class Native Explorer",
    kronosDesc:
      "Dedicated visual timeline, tool call diffing, step rollback, and multi-turn branch tree inspection built directly into the UI.",
    paseoTitle: "Basic Plain Logs Only",
    paseoDesc: "No dedicated OMP sessions route, no visual step diffs, and no state rollback view.",
  },
  {
    id: "dim-browser",
    title: "Browser Automation & DOM Grabbing",
    subtitle: "Web agent execution & visual markup inspection",
    kronosTitle: "Interactive Element Inspector",
    kronosDesc:
      "Interactive markup grab, live screencasting, Chromium cookie vault, and multi-pane split layout.",
    paseoTitle: "Static Preview Pane",
    paseoDesc:
      "Non-interactive HTML preview without DOM element targeting or automated cookie session injection.",
  },
  {
    id: "dim-palette",
    title: "Universal Command Palette (Ctrl+K)",
    subtitle: "Instant keyboard teleportation & agent actions",
    kronosTitle: "Integrated Command Hub",
    kronosDesc:
      "Fuzzy-find commands, switch active workspaces, dispatch background subagents, and route LLM providers with one shortcut.",
    paseoTitle: "Limited Fragmented Menus",
    paseoDesc: "Requires manual mouse clicks across scattered sidebar panes and dropdowns.",
  },
  {
    id: "dim-update",
    title: "Installation & Automated Updates",
    subtitle: "Packaging, distribution & git synchronization",
    kronosTitle: "1-Click Auto-Update Runner",
    kronosDesc:
      "Run update-desktop-exe.bat to automatically pull latest git updates and rebuild the NSIS setup executable.",
    paseoTitle: "Manual Multi-Step Rebuilds",
    paseoDesc:
      "No unified automation batch runner; prone to merge conflicts and manual workspace package build failures.",
  },
  {
    id: "dim-design",
    title: "Visual Design & Developer Ergonomics",
    subtitle: "Tactical aesthetics & high-contrast telemetry",
    kronosTitle: "Cyber Predator Stealth HUD",
    kronosDesc:
      "Chiseled titanium accents, glowing electric cyan telemetry vents, high-contrast OLED dark mode.",
    paseoTitle: "Generic Flat Grey Theme",
    paseoDesc: "Basic monochrome appearance with no hardware telemetry cues or visual hierarchy.",
  },
];

const BENTO_CARDS = [
  {
    id: "bento-rust",
    title: "Tauri v2 Native Rust Engine",
    desc: "Replaces heavy Chromium processes with native OS Webview2. Consumes under 100MB of RAM, keeping your CPU cool and battery intact during heavy agent builds.",
    icon: Cpu,
    colorClass: "bg-cyan-950 text-cyan-400 border-cyan-800/40",
  },
  {
    id: "bento-omp",
    title: "OMP Sessions Diff Timeline",
    desc: "Inspect every code modification, subagent reasoning step, and tool output. Rewind bad steps with one click before committing to your git history.",
    icon: History,
    colorClass: "bg-emerald-950 text-emerald-400 border-emerald-800/40",
  },
  {
    id: "bento-browser",
    title: "Headless Browser Inspector",
    desc: "Autonomous agents can interact with live web apps, click elements, extract DOM markup, and debug auth flows through the built-in browser pane.",
    icon: Globe,
    colorClass: "bg-blue-950 text-blue-400 border-blue-800/40",
  },
  {
    id: "bento-swarms",
    title: "Parallel Swarm Orchestration",
    desc: "Run Claude Code, OpenCode, Codex, and Pi simultaneously across multiple repositories. Each agent gets its own sandboxed terminal and state.",
    icon: Boxes,
    colorClass: "bg-purple-950 text-purple-400 border-purple-800/40",
  },
  {
    id: "bento-daemon",
    title: "Resilient Daemon Mesh",
    desc: "Local-first architecture keeps all source code on your machine. Dynamic local credentials eliminate disconnects and unauthorized socket probes.",
    icon: ShieldCheck,
    colorClass: "bg-amber-950 text-amber-400 border-amber-800/40",
  },
  {
    id: "bento-update",
    title: "1-Click Continuous Rebuilds",
    desc: "Stay continuously synchronized with upstream git changes. Just double-click update-desktop-exe.bat to pull, build, and package fresh installers.",
    icon: RefreshCw,
    colorClass: "bg-rose-950 text-rose-400 border-rose-800/40",
  },
];

function SwarmTerminalColumn() {
  return (
    <div className="md:col-span-7 rounded-xl border border-slate-800 bg-slate-900/60 p-4">
      <div className="mb-3 flex items-center justify-between border-b border-slate-800/80 pb-2">
        <div className="flex items-center gap-2 text-cyan-300">
          <Terminal className="h-4 w-4" />
          <span className="font-bold">PARALLEL AGENT TERMINAL (SWARM WAVE 2)</span>
        </div>
        <span className="rounded bg-cyan-950 px-2 py-0.5 text-[10px] text-cyan-400 border border-cyan-800/50">
          3 ACTIVE AGENTS
        </span>
      </div>
      <div className="space-y-2.5 text-slate-300 text-[11px] leading-relaxed">
        <div className="text-slate-500">
          $ kronos agent:dispatch --swarm=refactor-pipeline --concurrency=3
        </div>
        <div className="flex items-start gap-2">
          <span className="text-emerald-400 font-bold">[Agent: ClaudeCode]</span>
          <span>Tracing dependency tree for packages/server/src/server/omp-sessions-router.ts</span>
        </div>
        <div className="flex items-start gap-2">
          <span className="text-cyan-400 font-bold">[Agent: OpenCode]</span>
          <span>Writing AST mutation: Replacing obsolete imports in 14 modules.</span>
        </div>
        <div className="flex items-start gap-2">
          <span className="text-purple-400 font-bold">[Agent: OMP Scout]</span>
          <span>Verifying contract boundaries: Zero breaking changes in schema.</span>
        </div>
        <div className="rounded border border-emerald-500/20 bg-emerald-950/20 p-2 text-emerald-300">
          ✓ Step 4/4 Succeeded: Zero memory leak detected. Rust daemon thread pool steady at 4
          threads.
        </div>
      </div>
    </div>
  );
}

function SwarmInspectorColumn() {
  return (
    <div className="md:col-span-5 flex flex-col gap-2">
      <div className="flex-1 rounded-xl border border-slate-800 bg-slate-900/60 p-3">
        <div className="mb-2 flex items-center justify-between border-b border-slate-800/80 pb-1.5 text-slate-300">
          <div className="flex items-center gap-1.5 text-amber-300">
            <History className="h-3.5 w-3.5" />
            <span className="font-bold text-[11px]">OMP SESSIONS & TIMELINE</span>
          </div>
          <span className="text-[10px] text-slate-500">SESSION #01A0DD</span>
        </div>
        <div className="space-y-1.5 text-[10px]">
          <div className="flex items-center justify-between rounded bg-slate-800/40 p-1.5 text-slate-300">
            <span className="text-cyan-400">Step 1: Read AST Graph</span>
            <span className="text-slate-500">12ms · 4KB</span>
          </div>
          <div className="flex items-center justify-between rounded bg-slate-800/40 p-1.5 text-slate-300">
            <span className="text-cyan-400">Step 2: Diff Inspector Gated</span>
            <span className="text-emerald-400 font-bold">14 edits</span>
          </div>
          <div className="flex items-center justify-between rounded bg-cyan-950/40 border border-cyan-800/40 p-1.5 text-cyan-200">
            <span>Step 3: Verification Smoke Test</span>
            <span className="text-cyan-300">PASS (0.8s)</span>
          </div>
        </div>
      </div>
      <div className="flex-1 rounded-xl border border-slate-800 bg-slate-900/60 p-3">
        <div className="mb-2 flex items-center justify-between border-b border-slate-800/80 pb-1.5 text-slate-300">
          <div className="flex items-center gap-1.5 text-blue-400">
            <Eye className="h-3.5 w-3.5" />
            <span className="font-bold text-[11px]">BROWSER MARKUP OVERLAY</span>
          </div>
          <span className="text-[10px] text-emerald-400">INSPECT ON</span>
        </div>
        <div className="rounded border border-dashed border-cyan-500/40 bg-cyan-950/20 p-2 text-[10px] text-slate-300">
          <code>&lt;div id=&quot;app-root&quot; data-kronos-element=&quot;selected&quot;&gt;</code>
          <div className="mt-1 text-slate-400">Selector: #app-root &gt; .terminal-viewport</div>
        </div>
      </div>
    </div>
  );
}

function CockpitHeader() {
  return (
    <div className="flex items-center justify-between border-b border-slate-800/80 px-4 py-2.5">
      <div className="flex items-center gap-3">
        <div className="flex gap-1.5">
          <div className="h-3 w-3 rounded-full bg-red-500/80" />
          <div className="h-3 w-3 rounded-full bg-yellow-500/80" />
          <div className="h-3 w-3 rounded-full bg-green-500/80" />
        </div>
        <div className="flex items-center gap-2 border-l border-slate-800 pl-3">
          <img src="/kronos-logo.svg" alt="Kronos" className="h-4 w-4" />
          <span className="font-mono text-xs font-semibold text-slate-300">
            Kronos Workstation — active-swarm-01
          </span>
        </div>
      </div>
      <div className="flex items-center gap-4 text-xs font-mono text-slate-400">
        <div className="hidden sm:flex items-center gap-1.5 text-cyan-400">
          <Activity className="h-3.5 w-3.5" />
          <span>DAEMON: HEALTHY (200 OK)</span>
        </div>
        <div className="flex items-center gap-1.5 text-emerald-400">
          <Cpu className="h-3.5 w-3.5" />
          <span>RAM: 72.4 MB</span>
        </div>
        <div className="hidden md:flex items-center gap-1.5 text-slate-300">
          <Clock className="h-3.5 w-3.5" />
          <span>IPC: 1.4ms</span>
        </div>
      </div>
    </div>
  );
}

function CockpitWindow() {
  return (
    <motion.div
      initial={MOTION_FADE_UP_LG}
      animate={MOTION_VISIBLE}
      transition={TRANSITION_DUR_08_D45}
      className="mx-auto mt-14 max-w-6xl rounded-2xl border border-cyan-500/20 bg-slate-950/90 p-2 shadow-[0_20px_80px_rgba(0,0,0,0.8),0_0_50px_rgba(6,182,212,0.15)] backdrop-blur-2xl md:p-3"
    >
      <CockpitHeader />
      <div className="grid grid-cols-1 md:grid-cols-12 gap-2 p-2 font-mono text-xs">
        <SwarmTerminalColumn />
        <SwarmInspectorColumn />
      </div>
    </motion.div>
  );
}

function HeroHeader() {
  return (
    <div className="mx-auto flex max-w-5xl flex-col items-center text-center">
      <motion.div
        initial={MOTION_FADE_DOWN}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_05}
        className="mb-6 inline-flex items-center gap-2 rounded-full border border-cyan-500/30 bg-cyan-950/40 px-3.5 py-1.5 text-xs font-medium text-cyan-300 shadow-[0_0_15px_rgba(6,182,212,0.15)] backdrop-blur-md"
      >
        <span className="flex h-2 w-2 rounded-full bg-cyan-400 animate-pulse" />
        <span className="font-mono tracking-wider">KRONOS v0.12.0 APEX</span>
        <span className="text-cyan-600">|</span>
        <span className="text-slate-400">Tauri v2 Native Rust Core</span>
      </motion.div>

      <motion.h1
        initial={MOTION_FADE_UP}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_06_D1}
        className="text-4xl font-extrabold tracking-tight sm:text-6xl md:text-7xl lg:leading-[1.1]"
      >
        The Apex AI Developer Workstation &{" "}
        <span className="bg-gradient-to-r from-cyan-400 via-teal-300 to-blue-500 bg-clip-text text-transparent drop-shadow-[0_0_35px_rgba(6,182,212,0.3)]">
          Execution Engine
        </span>
      </motion.h1>

      <motion.p
        initial={MOTION_FADE_UP}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_06_D2}
        className="mt-6 max-w-3xl text-lg text-slate-300 md:text-xl font-normal leading-relaxed"
      >
        Engineered in <strong>Rust & Tauri v2</strong> with <strong>&lt;100MB RAM</strong> overhead.
        Zero Electron bloat. Features native OMP sessions timeline diffing, headless browser
        inspection, and unified multi-agent swarm orchestration.
      </motion.p>

      <motion.div
        initial={MOTION_FADE_UP}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_06_D3}
        className="mt-10 flex flex-wrap items-center justify-center gap-4"
      >
        <a
          href="#download"
          className="group relative inline-flex items-center gap-3 overflow-hidden rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 px-7 py-3.5 text-base font-semibold text-slate-950 shadow-[0_0_30px_rgba(6,182,212,0.35)] transition-all duration-200 hover:scale-105 hover:shadow-[0_0_45px_rgba(6,182,212,0.5)] active:scale-95"
        >
          <Download className="h-5 w-5 text-slate-950 transition-transform group-hover:-translate-y-0.5" />
          <span>Download Kronos (.exe)</span>
          <span className="rounded bg-black/20 px-1.5 py-0.5 text-xs font-mono font-bold text-slate-900">
            109 MB
          </span>
        </a>

        <a
          href="#compare"
          className="inline-flex items-center gap-2 rounded-xl border border-slate-700/80 bg-slate-900/80 px-6 py-3.5 text-base font-medium text-slate-200 backdrop-blur-md transition-all duration-200 hover:border-cyan-500/50 hover:bg-slate-800 hover:text-white"
        >
          <Sliders className="h-4 w-4 text-cyan-400" />
          <span>Why We Surpass Paseo</span>
          <ArrowRight className="h-4 w-4 text-slate-400 transition-transform group-hover:translate-x-1" />
        </a>
      </motion.div>

      <motion.div
        initial={MOTION_FADE_IN}
        animate={MOTION_VISIBLE_IN}
        transition={TRANSITION_DUR_08_D4}
        className="mt-12 flex flex-wrap items-center justify-center gap-6 text-xs text-slate-400 font-mono"
      >
        <div className="flex items-center gap-2">
          <Check className="h-4 w-4 text-cyan-400" />
          <span>85% Less Memory Than Electron</span>
        </div>
        <div className="flex items-center gap-2">
          <Check className="h-4 w-4 text-cyan-400" />
          <span>Instant Cold Boot (&lt;400ms)</span>
        </div>
        <div className="flex items-center gap-2">
          <Check className="h-4 w-4 text-cyan-400" />
          <span>100% Offline Compatible</span>
        </div>
        <div className="flex items-center gap-2">
          <Check className="h-4 w-4 text-cyan-400" />
          <span>1-Click Auto-Update Runner</span>
        </div>
      </motion.div>
    </div>
  );
}

function TelemetryBar() {
  return (
    <section className="border-y border-slate-800/80 bg-slate-950/60 py-10 backdrop-blur-md">
      <div className="mx-auto grid max-w-6xl grid-cols-2 gap-6 px-4 md:grid-cols-4">
        <div className="flex flex-col items-center justify-center p-4 text-center">
          <span className="text-3xl font-extrabold text-cyan-400 sm:text-4xl md:text-5xl font-mono">
            &lt;100MB
          </span>
          <span className="mt-2 text-sm font-medium text-slate-300">RAM Footprint</span>
          <span className="text-xs text-slate-500">8.5x lower than Electron</span>
        </div>
        <div className="flex flex-col items-center justify-center p-4 text-center">
          <span className="text-3xl font-extrabold text-emerald-400 sm:text-4xl md:text-5xl font-mono">
            380ms
          </span>
          <span className="mt-2 text-sm font-medium text-slate-300">Cold Boot Time</span>
          <span className="text-xs text-slate-500">Instant Rust initialization</span>
        </div>
        <div className="flex flex-col items-center justify-center p-4 text-center">
          <span className="text-3xl font-extrabold text-blue-400 sm:text-4xl md:text-5xl font-mono">
            32+
          </span>
          <span className="mt-2 text-sm font-medium text-slate-300">Parallel Swarm Tasks</span>
          <span className="text-xs text-slate-500">Zero UI thread blocking</span>
        </div>
        <div className="flex flex-col items-center justify-center p-4 text-center">
          <span className="text-3xl font-extrabold text-purple-400 sm:text-4xl md:text-5xl font-mono">
            109 MB
          </span>
          <span className="mt-2 text-sm font-medium text-slate-300">Lean NSIS Installer</span>
          <span className="text-xs text-slate-500">Stripped self-contained setup</span>
        </div>
      </div>
    </section>
  );
}

function ComparisonRow({ row }: { row: (typeof COMPARISON_ROWS)[0] }) {
  return (
    <tr className="hover:bg-slate-900/30 transition-colors">
      <td className="p-4 md:p-5 font-medium text-slate-200">
        <div className="font-semibold text-white">{row.title}</div>
        <div className="text-xs text-slate-400 mt-0.5">{row.subtitle}</div>
      </td>
      <td className="p-4 md:p-5 bg-cyan-950/15 border-x border-cyan-500/20 text-slate-200 font-medium">
        <div className="flex items-start gap-2.5">
          <Check className="h-5 w-5 text-emerald-400 shrink-0 mt-0.5" />
          <div>
            <strong className="text-emerald-300">{row.kronosTitle}</strong>
            <p className="text-xs text-slate-400 mt-1">{row.kronosDesc}</p>
          </div>
        </div>
      </td>
      <td className="p-4 md:p-5 text-slate-400">
        <div className="flex items-start gap-2.5">
          <X className="h-5 w-5 text-red-400 shrink-0 mt-0.5" />
          <div>
            <span className="text-red-300 font-medium">{row.paseoTitle}</span>
            <p className="text-xs text-slate-500 mt-1">{row.paseoDesc}</p>
          </div>
        </div>
      </td>
    </tr>
  );
}

function ComparisonTableHeader() {
  return (
    <tr className="border-b border-slate-800 bg-slate-900/70 text-xs font-mono uppercase tracking-wider text-slate-400">
      <th className="p-4 md:p-5 w-1/3">Capability / Dimension</th>
      <th className="p-4 md:p-5 w-1/3 bg-cyan-950/20 text-cyan-300 font-bold border-x border-cyan-500/20">
        <div className="flex items-center gap-2">
          <img src="/kronos-logo.svg" alt="Kronos" className="h-4 w-4" />
          <span>KRONOS (Apex Workstation)</span>
        </div>
      </th>
      <th className="p-4 md:p-5 w-1/3 text-slate-400">
        <div className="flex items-center gap-2">
          <div className="h-2 w-2 rounded-full bg-slate-500" />
          <span>PASEO (Legacy Desktop)</span>
        </div>
      </th>
    </tr>
  );
}

function ComparisonTable() {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-left border-collapse">
        <thead>
          <ComparisonTableHeader />
        </thead>
        <tbody className="divide-y divide-slate-800/80 text-sm">
          {COMPARISON_ROWS.map((row) => (
            <ComparisonRow key={row.id} row={row} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ComparisonSection() {
  return (
    <section id="compare" className="scroll-mt-20 py-24 md:py-32">
      <div className="mx-auto max-w-6xl px-4">
        <div className="text-center">
          <div className="inline-flex items-center gap-2 rounded-full border border-cyan-500/30 bg-cyan-950/30 px-3 py-1 text-xs font-semibold text-cyan-400">
            <Sliders className="h-3.5 w-3.5" />
            <span>HEAD-TO-HEAD ENGINEERING AUDIT</span>
          </div>
          <h2 className="mt-4 text-3xl font-bold tracking-tight sm:text-5xl text-white">
            How <span className="text-cyan-400">Kronos</span> Completely Surpasses{" "}
            <span className="text-slate-400 line-through">Paseo</span>
          </h2>
          <p className="mx-auto mt-4 max-w-2xl text-slate-400 text-base md:text-lg">
            We took the concept of an agentic development environment and re-engineered it from the
            ground up: replacing heavy Electron bloat with a razor-sharp Rust core, adding
            first-class OMP timeline diffing, and eliminating connection failures.
          </p>
        </div>
        <div className="mt-14 overflow-hidden rounded-2xl border border-slate-800 bg-slate-950/80 shadow-[0_0_50px_rgba(0,0,0,0.5)]">
          <ComparisonTable />
        </div>
      </div>
    </section>
  );
}

function BentoCard({ card }: { card: (typeof BENTO_CARDS)[0] }) {
  const Icon = card.icon;
  return (
    <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-6 transition-all duration-300 hover:border-cyan-500/40 hover:bg-slate-900/70 hover:shadow-[0_0_30px_rgba(6,182,212,0.15)]">
      <div
        className={`flex h-12 w-12 items-center justify-center rounded-xl border ${card.colorClass}`}
      >
        <Icon className="h-6 w-6" />
      </div>
      <h3 className="mt-5 text-lg font-bold text-white">{card.title}</h3>
      <p className="mt-2 text-sm text-slate-400 leading-relaxed">{card.desc}</p>
    </div>
  );
}

function BentoSection() {
  return (
    <section
      id="features"
      className="scroll-mt-20 py-20 bg-slate-950/40 border-t border-slate-800/60"
    >
      <div className="mx-auto max-w-6xl px-4">
        <div className="text-center">
          <h2 className="text-3xl font-bold tracking-tight text-white sm:text-4xl">
            Engineered For Power Developers
          </h2>
          <p className="mt-3 text-slate-400">
            Every system in Kronos is built for zero friction and maximum throughput.
          </p>
        </div>
        <div className="mt-14 grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {BENTO_CARDS.map((card) => (
            <BentoCard key={card.id} card={card} />
          ))}
        </div>
      </div>
    </section>
  );
}

function SnippetTabs({
  activeTab,
  onSelectBatch,
  onSelectPowershell,
  onSelectClone,
  onCopy,
  copied,
}: {
  activeTab: "batch" | "powershell" | "clone";
  onSelectBatch: () => void;
  onSelectPowershell: () => void;
  onSelectClone: () => void;
  onCopy: () => void;
  copied: boolean;
}) {
  return (
    <div className="flex items-center justify-between border-b border-slate-800 bg-slate-900/60 px-4 py-2.5">
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onSelectBatch}
          className={`text-xs font-mono px-3 py-1 rounded transition-colors ${
            activeTab === "batch"
              ? "bg-cyan-950 text-cyan-300 border border-cyan-800/50"
              : "text-slate-400 hover:text-white"
          }`}
        >
          Windows 1-Click Update
        </button>
        <button
          type="button"
          onClick={onSelectPowershell}
          className={`text-xs font-mono px-3 py-1 rounded transition-colors ${
            activeTab === "powershell"
              ? "bg-cyan-950 text-cyan-300 border border-cyan-800/50"
              : "text-slate-400 hover:text-white"
          }`}
        >
          NPM Command
        </button>
        <button
          type="button"
          onClick={onSelectClone}
          className={`text-xs font-mono px-3 py-1 rounded transition-colors ${
            activeTab === "clone"
              ? "bg-cyan-950 text-cyan-300 border border-cyan-800/50"
              : "text-slate-400 hover:text-white"
          }`}
        >
          Git Clone
        </button>
      </div>

      <button
        type="button"
        onClick={onCopy}
        className="flex items-center gap-1.5 text-xs text-slate-400 hover:text-cyan-300 transition-colors"
      >
        <Copy className="h-3.5 w-3.5" />
        <span>{copied ? "Copied!" : "Copy"}</span>
      </button>
    </div>
  );
}

function SnippetCode({ activeTab }: { activeTab: "batch" | "powershell" | "clone" }) {
  if (activeTab === "batch") {
    return (
      <>
        <div className="text-slate-500"># Double click in file explorer or run in CMD:</div>
        <div className="text-cyan-300 font-bold mt-1">.\\update-desktop-exe.bat</div>
        <div className="text-slate-500 mt-2">
          # Automatically pulls git, builds workspace packages, and compiles the .exe installer.
        </div>
      </>
    );
  }
  if (activeTab === "powershell") {
    return (
      <>
        <div className="text-slate-500"># Run in root repository directory:</div>
        <div className="text-cyan-300 font-bold mt-1">npm run desktop:update-build</div>
      </>
    );
  }
  return (
    <>
      <div className="text-slate-500"># Clone our official repository fork:</div>
      <div className="text-cyan-300 font-bold mt-1">
        git clone https://github.com/Xenonesis/paseo.git
      </div>
    </>
  );
}

function SnippetCard() {
  const [activeTab, setActiveTab] = useState<"batch" | "powershell" | "clone">("batch");
  const [copied, setCopied] = useState(false);

  const onCopy = useCallback(() => {
    let cmd = ".\\update-desktop-exe.bat";
    if (activeTab === "powershell") {
      cmd = "npm run desktop:update-build";
    } else if (activeTab === "clone") {
      cmd = "git clone https://github.com/Xenonesis/paseo.git";
    }
    navigator.clipboard.writeText(cmd);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [activeTab]);

  const onSelectBatch = useCallback(() => setActiveTab("batch"), []);
  const onSelectPowershell = useCallback(() => setActiveTab("powershell"), []);
  const onSelectClone = useCallback(() => setActiveTab("clone"), []);

  return (
    <div className="mt-10 w-full max-w-2xl rounded-xl border border-slate-800 bg-slate-950/90 text-left overflow-hidden">
      <SnippetTabs
        activeTab={activeTab}
        onSelectBatch={onSelectBatch}
        onSelectPowershell={onSelectPowershell}
        onSelectClone={onSelectClone}
        onCopy={onCopy}
        copied={copied}
      />
      <div className="p-4 font-mono text-xs text-slate-300 leading-relaxed overflow-x-auto">
        <SnippetCode activeTab={activeTab} />
      </div>
    </div>
  );
}

function InstallSection() {
  return (
    <section id="download" className="scroll-mt-20 py-24 md:py-32">
      <div className="mx-auto max-w-5xl px-4">
        <div className="rounded-3xl border border-cyan-500/30 bg-gradient-to-b from-slate-900 via-slate-950 to-slate-950 p-8 shadow-[0_0_80px_rgba(6,182,212,0.15)] md:p-14">
          <div className="flex flex-col items-center text-center">
            <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-cyan-950 text-cyan-400 border border-cyan-500/40 shadow-[0_0_20px_rgba(6,182,212,0.3)]">
              <Download className="h-8 w-8" />
            </div>
            <h2 className="mt-6 text-3xl font-bold tracking-tight text-white sm:text-5xl">
              Deploy Kronos On Your Workstation
            </h2>
            <p className="mt-3 max-w-xl text-slate-400 text-base md:text-lg">
              Production-ready NSIS installer for Windows 10 & 11 x64. Packaged with bundled runtime
              dependencies.
            </p>

            <div className="mt-8 flex flex-wrap items-center justify-center gap-4">
              <a
                href="/download"
                className="inline-flex items-center gap-3 rounded-xl bg-cyan-400 px-8 py-4 font-bold text-slate-950 shadow-[0_0_30px_rgba(6,182,212,0.4)] transition-all hover:bg-cyan-300 hover:scale-105 active:scale-95"
              >
                <Download className="h-5 w-5" />
                <span>Download Kronos 0.12.0 (.exe)</span>
              </a>
            </div>

            <div className="mt-4 text-xs font-mono text-slate-400">
              SHA-256 Verified · 109.3 MB · Architecture: x64 Windows
            </div>

            <SnippetCard />
          </div>
        </div>
      </div>
    </section>
  );
}

function FaqItem({
  item,
  isOpen,
  onToggle,
}: {
  item: (typeof FAQ_ITEMS)[0];
  isOpen: boolean;
  onToggle: (id: string) => void;
}) {
  const handleClick = useCallback(() => {
    onToggle(item.id);
  }, [onToggle, item.id]);

  return (
    <div className="overflow-hidden rounded-xl border border-slate-800 bg-slate-900/40 transition-colors hover:border-slate-700">
      <button
        type="button"
        onClick={handleClick}
        className="flex w-full items-center justify-between p-5 text-left font-semibold text-slate-200"
      >
        <span>{item.q}</span>
        <ChevronRight
          className={`h-4 w-4 text-cyan-400 transition-transform duration-200 ${
            isOpen ? "rotate-90" : ""
          }`}
        />
      </button>
      <AnimatePresence>
        {isOpen && (
          <div className="px-5 pb-5 text-sm text-slate-400 leading-relaxed border-t border-slate-800/40 pt-3">
            {item.a}
          </div>
        )}
      </AnimatePresence>
    </div>
  );
}

function FaqSection() {
  const [openId, setOpenId] = useState<string | null>("faq-migration");
  const handleToggle = useCallback((id: string) => {
    setOpenId((prev) => (prev === id ? null : id));
  }, []);

  return (
    <section className="border-t border-slate-800/80 py-20 bg-slate-950/60">
      <div className="mx-auto max-w-4xl px-4">
        <div className="text-center">
          <h2 className="text-3xl font-bold tracking-tight text-white sm:text-4xl">
            Frequently Asked Questions
          </h2>
          <p className="mt-3 text-slate-400">
            Everything you need to know about switching to Kronos.
          </p>
        </div>
        <div className="mt-12 space-y-4">
          {FAQ_ITEMS.map((item) => (
            <FaqItem
              key={item.id}
              item={item}
              isOpen={openId === item.id}
              onToggle={handleToggle}
            />
          ))}
        </div>
      </div>
    </section>
  );
}
export function KronosLandingPage() {
  return (
    <div className="relative w-full overflow-hidden text-slate-100 selection:bg-cyan-500/30 selection:text-cyan-200">
      <div className="pointer-events-none absolute -top-40 left-1/2 -z-10 h-[650px] w-[1000px] -translate-x-1/2 rounded-full bg-gradient-to-b from-cyan-500/15 via-blue-600/10 to-transparent blur-[120px]" />
      <div className="pointer-events-none absolute top-[1200px] -left-60 -z-10 h-[500px] w-[600px] rounded-full bg-cyan-600/10 blur-[140px]" />
      <div className="pointer-events-none absolute top-[2400px] -right-60 -z-10 h-[600px] w-[600px] rounded-full bg-blue-500/10 blur-[150px]" />

      <div className="mx-auto max-w-6xl px-4 pt-6">
        <SiteHeader />
      </div>

      <section className="relative pt-6 pb-20 md:pt-12 md:pb-32">
        <HeroHeader />
        <CockpitWindow />
      </section>

      <TelemetryBar />
      <ComparisonSection />
      <BentoSection />
      <InstallSection />
      <FaqSection />

      <div className="border-t border-slate-800/80 bg-slate-950 py-12">
        <SiteFooter width="wide" sponsors={false} />
      </div>
    </div>
  );
}
