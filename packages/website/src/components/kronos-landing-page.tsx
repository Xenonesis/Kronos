import * as React from "react";
import { useState, useCallback, useEffect } from "react";
import { ArrowRight, ChevronRight, Copy, Download } from "lucide-react";
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

const LATEST_RELEASE_API = "https://api.github.com/repos/Xenonesis/Kronos/releases/latest";
const PERSISTENT_DOWNLOAD_URL =
  "https://github.com/Xenonesis/Kronos/releases/latest/download/Kronos_x64-setup.exe";

interface ReleaseData {
  downloadUrl: string;
  version: string;
  size: string;
  filename: string;
}

function useKronosRelease(): ReleaseData {
  const [release, setRelease] = useState<ReleaseData>({
    downloadUrl: PERSISTENT_DOWNLOAD_URL,
    version: "v0.12.0",
    size: "97 MB",
    filename: "Kronos_0.12.0_x64-setup.exe",
  });

  useEffect(() => {
    async function fetchRelease() {
      try {
        const res = await fetch(LATEST_RELEASE_API);
        if (!res.ok) return;
        const data = (await res.json()) as {
          tag_name?: string;
          assets?: Array<{ name: string; size: number; browser_download_url?: string }>;
        };
        if (!data || !Array.isArray(data.assets)) return;
        const exeAsset =
          data.assets.find((a) => a.name.endsWith("-setup.exe")) ||
          data.assets.find((a) => a.name.endsWith(".exe"));
        if (exeAsset) {
          const mb = (exeAsset.size / (1024 * 1024)).toFixed(0) + " MB";
          setRelease({
            downloadUrl: exeAsset.browser_download_url || PERSISTENT_DOWNLOAD_URL,
            version: data.tag_name || "v0.12.0",
            size: mb,
            filename: exeAsset.name,
          });
        }
      } catch {
        // Maintain persistent fallback
      }
    }
    void fetchRelease();
  }, []);

  return release;
}

const FAQ_ITEMS = [
  {
    id: "faq-migration",
    code: "SEC / MIG-01",
    q: "MIGRATION PROTOCOL // CAN EXISTING PASEO WORKSPACES BE IMPORTED?",
    a: "AFFIRMATIVE. KRONOS RETAINS 100% RETROGRADE COMPATIBILITY WITH EXISTING CONFIGURATION DIRECTORIES (~/.paseo). LOCAL AGENT STATE, CREDENTIAL HASHES, AND WORKSPACE MATRICES ARE INGESTED AUTOMATICALLY WITHOUT RE-AUTHENTICATION.",
  },
  {
    id: "faq-ram",
    code: "SEC / MEM-02",
    q: "RESOURCE METRICS // WHY DOES KRONOS ELIMINATE 85% OF MEMORY OVERHEAD?",
    a: "PASEO EMPLOYS A LEGACY ELECTRON HARNESS SPAWNING INDEPENDENT CHROMIUM INSTANCES (600MB–1.2GB RESIDENT SET). KRONOS BINDS TO A PURE RUST TAURI V2 COMPOSITOR LINKED DIRECTLY TO NATIVE WEBVIEW2 SUBSYSTEMS, BOUNDING TOTAL RAM TO UNDER 100MB.",
  },
  {
    id: "faq-update",
    code: "SEC / UPD-03",
    q: "SYNCHRONIZATION // HOW DOES THE 1-CLICK AUTOMATED RUNNER OPERATE?",
    a: "RUNNING .\\update-desktop-exe.bat EXECUTES AN ATOMIC FETCH PIPELINE: PULLS UPSTREAM DELTAS, COMPILES INTERNAL PROTOCOL LIBRARIES, EXPORTS THE WEB ASSET RUNTIME, AND PACKAGES A FRESH PE32+ NSIS INSTALLER LOCALLY.",
  },
  {
    id: "faq-omp",
    code: "SEC / OMP-04",
    q: "DIAGNOSTICS // WHAT IS THE PURPOSE OF THE OMP SESSIONS TIMELINE?",
    a: "OMP (OPEN MULTI-AGENT PROTOCOL) TIMELINE PROVIDES FRAME-ACCURATE TELEMETRY INTO SUBAGENT STATE TREES, TOOL DISPATCH PAYLOADS, AST DIFF ARTIFACTS, AND DETERMINISTIC ROLLBACK POINTS BEFORE GIT COMMITS OCCUR.",
  },
  {
    id: "faq-offline",
    code: "SEC / SEC-05",
    q: "DATA SECURITY // ARE WORKSPACE BUILDS TRANSMITTED EXTERNALLY?",
    a: "NEGATIVE. KRONOS ENFORCES STRICT AIR-GAP PROTOCOLS. ALL DAEMON SOCKETS, DISPATCH PIPELINES, AND CONCURRENCY POOLS EXECUTE STRICTLY ON LOCALHOST (127.0.0.1) WITH ZERO OUTBOUND TELEMETRY SINKHOLES.",
  },
  {
    id: "faq-streaming",
    code: "SEC / STR-06",
    q: "RUNTIME INTEGRITY // HOW DOES KRONOS PREVENT 'NOT RESPONDING' FREEZES DURING STREAMING?",
    a: "KRONOS IMPLEMENTS A CLIENT-SIDE TOKEN UPDATE DEBOUNCE (300MS WINDOW), ATOMIC NATIVE SYSTEM TRAY RATE-LIMITING IN RUST, AN OPAQUE DIRECT3D SWAPCHAIN (ELIMINATING HEAVY WINDOWS MICA COMPOSITION DEADLOCKS), AND OFFLOADS TOAST NOTIFICATIONS TO ISOLATED BACKGROUND WORKER THREADS.",
  },
];

const COMPARISON_ROWS = [
  {
    id: "dim-arch",
    code: "SPEC // D01",
    dimension: "DESKTOP ENGINE ARCHITECTURE",
    kronosSpec: "TAURI V2 + NATIVE RUST SUBSYSTEM",
    kronosDesc: "Hardware-accelerated OS Webview2. Stripped executable. Zero Electron baggage.",
    kronosStatus: "[+] PASS: 74MB IDLE",
    paseoSpec: "LEGACY ELECTRON / BUNDLED CHROMIUM",
    paseoDesc: "Bloated multi-process Chromium shell. Continuous CPU throttling and battery drain.",
    paseoStatus: "[-] OBSOLETE: 840MB IDLE",
  },
  {
    id: "dim-boot",
    code: "SPEC // D02",
    dimension: "COLD BOOT TIME TO TERMINAL READY",
    kronosSpec: "0.38 SECONDS (INSTANT INITIALIZATION)",
    kronosDesc: "Pre-compiled Rust runtime executes binary initialization immediately.",
    kronosStatus: "[+] PASS: SUB-SECOND",
    paseoSpec: "4.20 SECONDS (CHROMIUM ENGINE SPIN-UP)",
    paseoDesc: "Heavy V8 heap initialization, Node loader overhead, and preload script blocking.",
    paseoStatus: "[-] PENALTY: 11X SLOWER",
  },
  {
    id: "dim-daemon",
    code: "SPEC // D03",
    dimension: "DAEMON SOCKET CONNECTION RESILIENCE",
    kronosSpec: "GATED 200_OK HEALTH HANDSHAKE",
    kronosDesc:
      "Window render gated by HTTP health polling. Persistent dynamic local credential mesh.",
    kronosStatus: "[+] PASS: 0% DROPOUT",
    paseoSpec: "UNSYNCHRONIZED SOCKET RACE CONDITION",
    paseoDesc:
      "Window surfaces before daemon finishes binding port, triggering recurrent error banners.",
    paseoStatus: "[-] FAILURE: UNSTABLE",
  },
  {
    id: "dim-omp",
    code: "SPEC // D04",
    dimension: "OMP MULTI-AGENT SESSIONS TIMELINE",
    kronosSpec: "DEDICATED NATIVE TIMELINE & DIFF INSPECTOR",
    kronosDesc: "Visual tool invocation diffs, rollback checkpoints, and token telemetry trees.",
    kronosStatus: "[+] PASS: FULL TELEMETRY",
    paseoSpec: "UNSTRUCTURED STDOUT TEXT STREAM",
    paseoDesc: "No timeline navigation, no structured diff verification, no state rewind controls.",
    paseoStatus: "[-] ABSENT: RAW LOGS ONLY",
  },
  {
    id: "dim-browser",
    code: "SPEC // D05",
    dimension: "BROWSER WORKSPACE & DOM INSPECTION",
    kronosSpec: "INTERACTIVE ELEMENT & MARKUP GRABBER",
    kronosDesc:
      "Target selector crosshairs, live screencasting, and Chromium cookie vault injection.",
    kronosStatus: "[+] PASS: INTERACTIVE",
    paseoSpec: "STATIC SANDBOXED HTML PREVIEW",
    paseoDesc:
      "Read-only viewport without DOM element interrogation or automated cookie migration.",
    paseoStatus: "[-] DEFICIENT: PASSIVE",
  },
  {
    id: "dim-palette",
    code: "SPEC // D06",
    dimension: "COMMAND BUS TELEPORTATION (CTRL+K)",
    kronosSpec: "UNIVERSAL TACTICAL COMMAND MATRIX",
    kronosDesc: "Instant subagent routing, fuzzy workspace switching, and model re-allocation.",
    kronosStatus: "[+] PASS: 100% KEYBOARD",
    paseoSpec: "FRAGMENTED MULTI-CLICK SIDEBARS",
    paseoDesc: "Requires manual mouse navigation across nested panels and unlinked submenus.",
    paseoStatus: "[-] FRICTION: MANUAL",
  },
  {
    id: "dim-update",
    code: "SPEC // D07",
    dimension: "RELEASE SYNCHRONIZATION PIPELINE",
    kronosSpec: "1-CLICK ATOMIC REBUILD RUNNER",
    kronosDesc: "Executes git pull, workspace package builds, and NSIS compilation in one command.",
    kronosStatus: "[+] PASS: AUTOMATED",
    paseoSpec: "MANUAL MULTI-TIER TERMINAL BUILDS",
    paseoDesc: "Prone to package desynchronization and missed compiler pre-requisites.",
    paseoStatus: "[-] HIGH RISK: MANUAL",
  },
  {
    id: "dim-design",
    code: "SPEC // D08",
    dimension: "TACTICAL ERGONOMICS & VISUAL SYNTHESIS",
    kronosSpec: "INDUSTRIAL BRUTALISM & TELEMETRY HUD",
    kronosDesc:
      "Monolithic uppercase typography, rigid 1px blueprint grids, and hazard red alerts.",
    kronosStatus: "[+] PASS: MAXIMUM DENSITY",
    paseoSpec: "GENERIC LOW-CONTRAST MONOCHROME",
    paseoDesc: "Unstructured spacing, low visual hierarchy, and missing hardware status readouts.",
    paseoStatus: "[-] BLAND: LOW CONTRAST",
  },
  {
    id: "dim-streaming",
    code: "SPEC // D09",
    dimension: "AGENT STREAMING & IPC EVENT PIPELINE",
    kronosSpec: "DEBOUNCED IPC + DIRECT3D OPAQUE SWAPCHAIN",
    kronosDesc:
      "300ms tray debouncing, AtomicUsize native guard, and async notification threads prevent DWM composition stalls during high-velocity LLM token streaming.",
    kronosStatus: "[+] PASS: 60FPS ROCK SOLID",
    paseoSpec: "UNTHROTTLED WIN32 IPC & MICA COMPOSITION STALLS",
    paseoDesc:
      "Fires Shell_NotifyIconW on every streaming token delta; synchronous Toast alerts deadlock the UI thread under agent load.",
    paseoStatus: "[-] CRASH RISK: 'NOT RESPONDING'",
  },
];

const BENTO_CARDS = [
  {
    id: "bento-rust",
    unit: "UNIT // SYS-01",
    title: "TAURI V2 RUST BACKBONE",
    spec: "RESIDENT SET: < 100MB",
    desc: "Replaces heavy multi-process Chromium containers with direct native OS Webview2 integration. Zero memory leakage under sustained 12-hour multi-agent build waves.",
  },
  {
    id: "bento-omp",
    unit: "UNIT // TEL-02",
    title: "OMP TIMELINE & DIFF MATRIX",
    spec: "INSPECTION: AST-ACCURATE",
    desc: "Inspect every file mutation, tool invocation parameter, and subagent reasoning step. Roll back failed speculative execution branches before permanent git commits.",
  },
  {
    id: "bento-browser",
    unit: "UNIT // DOM-03",
    title: "TACTICAL BROWSER HEADLESS PANE",
    spec: "AUTOMATION: CDP RECEPTOR",
    desc: "Autonomous agents query live DOM hierarchies, click verified coordinates, capture markup snapshots, and inject session cookies from host Chromium stores.",
  },
  {
    id: "bento-swarms",
    unit: "UNIT // SWR-04",
    title: "PARALLEL CONCURRENT SWARMS",
    spec: "DISPATCH POOL: 32 THREADS",
    desc: "Execute Claude Code, OpenCode, Codex, and Pi simultaneously across multiple isolated worktrees with sandboxed environmental variables and shared IPC channels.",
  },
  {
    id: "bento-daemon",
    unit: "UNIT // MSH-05",
    title: "RESILIENT DAEMON HEALTH MESH",
    spec: "HANDSHAKE: GATED 200_OK",
    desc: "Eliminates 'Host not connected' socket race conditions via pre-flight health polling and persistent filesystem credential verification at boot.",
  },
  {
    id: "bento-update",
    unit: "UNIT // BLD-06",
    title: "1-CLICK ATOMIC REBUILD RUNNER",
    spec: "PACKAGING: NSIS SETUP .EXE",
    desc: "Integrated update-desktop-exe.bat script fetches upstream commits, compiles internal packages, and outputs a freshly certified Windows installer in one stroke.",
  },
];

function SwarmTerminalColumn() {
  return (
    <div className="md:col-span-7 border border-[#262626] bg-[#0A0A0A] p-4 font-mono text-xs">
      <div className="mb-3 flex items-center justify-between border-b border-[#262626] pb-2 text-[11px]">
        <div className="flex items-center gap-2 text-[#EAEAEA]">
          <span className="text-[#FF2A2A] font-bold">&gt;&gt;&gt;</span>
          <span className="font-bold tracking-wider">SWARM CONCURRENCY MATRIX // WAVE_02</span>
        </div>
        <span className="bg-[#1C1C1C] border border-[#333333] px-2 py-0.5 text-[10px] text-[#4AF626] font-bold">
          [ 03 ACTIVE AGENTS ]
        </span>
      </div>
      <div className="space-y-2 text-[#888888] text-[11px] leading-relaxed">
        <div className="text-[#EAEAEA]">
          $ kronos agent:dispatch --swarm=refactor-pipeline --concurrency=3
        </div>
        <div className="text-[#EAEAEA]">
          <span className="text-[#4AF626] font-bold">[AGENT: CLAUDE_CODE]</span> TRACING DEPENDENCY
          GRAPH: /omp-sessions-router.ts
        </div>
        <div className="text-[#EAEAEA]">
          <span className="text-[#4AF626] font-bold">[AGENT: OPEN_CODE]</span> EXECUTING AST
          MUTATION: REFACTORING 14 EXPORTS
        </div>
        <div className="text-[#EAEAEA]">
          <span className="text-[#FF2A2A] font-bold">[AGENT: OMP_SCOUT]</span> VERIFYING CONTRACT
          BOUNDARIES: ZERO BREAKING CHANGES
        </div>
        <div className="border border-[#262626] bg-[#121212] p-2 text-[#4AF626] font-mono text-[10px]">
          [STATUS: 200_OK] CYCLE TIME: 780MS // MEMORY DELTA: 0.00MB // KERNEL THREADS: 4
        </div>
      </div>
    </div>
  );
}

function SwarmInspectorColumn() {
  return (
    <div className="md:col-span-5 flex flex-col gap-2 font-mono text-xs">
      <div className="flex-1 border border-[#262626] bg-[#0A0A0A] p-3">
        <div className="mb-2 flex items-center justify-between border-b border-[#262626] pb-1 text-[11px]">
          <span className="font-bold text-[#EAEAEA]">[ TELEMETRY: OMP SESSIONS ]</span>
          <span className="text-[#888888] text-[10px]">ID: #01A0DD</span>
        </div>
        <div className="space-y-1 text-[10px]">
          <div className="flex justify-between border-b border-[#1C1C1C] py-1 text-[#888888]">
            <span className="text-[#EAEAEA]">STEP 01: READ_AST_GRAPH</span>
            <span className="text-[#888888]">12MS</span>
          </div>
          <div className="flex justify-between border-b border-[#1C1C1C] py-1 text-[#888888]">
            <span className="text-[#EAEAEA]">STEP 02: DIFF_INSPECTOR</span>
            <span className="text-[#4AF626] font-bold">+14 / -0</span>
          </div>
          <div className="flex justify-between py-1 text-[#888888]">
            <span className="text-[#EAEAEA]">STEP 03: VERIFICATION_SMOKE</span>
            <span className="text-[#4AF626] font-bold">[ PASS ]</span>
          </div>
        </div>
      </div>
      <div className="flex-1 border border-[#262626] bg-[#0A0A0A] p-3">
        <div className="mb-2 flex items-center justify-between border-b border-[#262626] pb-1 text-[11px]">
          <span className="font-bold text-[#EAEAEA]">[ DOM INSPECTOR RECEPTOR ]</span>
          <span className="text-[#4AF626] text-[10px] font-bold">[ ACTIVE ]</span>
        </div>
        <div className="border border-[#333333] bg-[#121212] p-2 text-[10px] text-[#888888]">
          <div className="text-[#EAEAEA]">
            &lt;div id=&quot;app-root&quot; data-kronos-element=&quot;selected&quot;&gt;
          </div>
          <div className="text-[#FF2A2A] mt-1">TARGET: #app-root &gt; .terminal-viewport</div>
        </div>
      </div>
    </div>
  );
}

function CockpitHeader() {
  return (
    <div className="flex items-center justify-between border-b border-[#262626] bg-[#121212] px-4 py-2 text-xs font-mono">
      <div className="flex items-center gap-3">
        <span className="bg-[#FF2A2A] text-black font-bold px-1.5 py-0.5 text-[10px]">
          TERMINAL / 01
        </span>
        <span className="text-[#EAEAEA] font-bold tracking-wider">
          KRONOS WORKSTATION // ACTIVE SWARM SESSION
        </span>
      </div>
      <div className="flex items-center gap-4 text-[11px] text-[#888888]">
        <div>
          SYS_HEALTH: <span className="text-[#4AF626] font-bold">[ 200_OK ]</span>
        </div>
        <div>
          RAM: <span className="text-[#EAEAEA] font-bold">72.4 MB</span>
        </div>
        <div>
          IPC_LATENCY: <span className="text-[#EAEAEA] font-bold">1.4 MS</span>
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
      className="mx-auto mt-12 max-w-6xl border border-[#262626] bg-[#121212] p-2 rounded-none shadow-none"
    >
      <CockpitHeader />
      <div className="grid grid-cols-1 md:grid-cols-12 gap-2 p-2">
        <SwarmTerminalColumn />
        <SwarmInspectorColumn />
      </div>
    </motion.div>
  );
}

function HeroHeader({ release }: { release: ReleaseData }) {
  return (
    <div className="mx-auto flex max-w-5xl flex-col items-center text-center font-mono">
      <motion.div
        initial={MOTION_FADE_DOWN}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_05}
        className="mb-6 flex flex-col items-center gap-4"
      >
        <img
          src={`${import.meta.env.BASE_URL}kronos-logo.svg`}
          alt="Kronos Apex Logo"
          className="w-16 h-16 sm:w-20 sm:h-20 drop-shadow-[0_0_24px_rgba(0,240,255,0.4)]"
        />
        <div className="inline-flex items-center gap-3 border border-[#262626] bg-[#121212] px-4 py-1 text-xs tracking-widest text-[#EAEAEA] uppercase">
          <span className="h-2 w-2 bg-[#FF2A2A]" />
          <span>[ CLASSIFIED / SPEC-D12 ]</span>
          <span className="text-[#444444]">{"///"}</span>
          <span className="text-[#888888]">TAURI_V2_NATIVE_RUST_CORE</span>
        </div>
      </motion.div>
      <motion.h1
        initial={MOTION_FADE_UP}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_06_D1}
        className="text-5xl font-black uppercase tracking-[-0.04em] sm:text-7xl md:text-8xl leading-[0.88] text-[#EAEAEA]"
      >
        KRONOS
        <span className="block text-2xl sm:text-4xl md:text-5xl font-bold tracking-tight text-[#FF2A2A] mt-2">
          AUTONOMOUS EXECUTION ENGINE
        </span>
      </motion.h1>

      <motion.p
        initial={MOTION_FADE_UP}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_06_D2}
        className="mt-6 max-w-3xl text-sm md:text-base text-[#888888] uppercase tracking-wider leading-relaxed"
      >
        {
          "[ SPECIFICATION: NATIVE RUST WORKSPACE // RAM ALLOCATION < 100MB // ZERO CHROMIUM RESIDUE // FRAME-ACCURATE OMP TIMELINE DIFFING // DETERMINISTIC SWARM CONCURRENCY ]"
        }
      </motion.p>

      <motion.div
        initial={MOTION_FADE_UP}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_06_D3}
        className="mt-8 flex flex-wrap items-center justify-center gap-4 text-xs font-mono font-bold"
      >
        <a
          href={release.downloadUrl}
          className="inline-flex items-center gap-3 bg-[#FF2A2A] px-6 py-3 text-black hover:bg-white transition-colors tracking-widest uppercase"
        >
          <Download className="h-4 w-4 text-black" />
          <span>&gt;&gt;&gt; GET_INSTALLER_X64.EXE</span>
          <span className="bg-black text-white px-1.5 py-0.5 text-[10px]">{release.size}</span>
        </a>

        <a
          href="#compare"
          className="inline-flex items-center gap-2 border border-[#333333] bg-[#121212] px-6 py-3 text-[#EAEAEA] hover:border-[#FF2A2A] hover:text-[#FF2A2A] transition-colors tracking-widest uppercase"
        >
          <span>{"/// VIEW_TECHNICAL_AUDIT"}</span>
          <ArrowRight className="h-4 w-4" />
        </a>
      </motion.div>

      <motion.div
        initial={MOTION_FADE_IN}
        animate={MOTION_VISIBLE_IN}
        transition={TRANSITION_DUR_08_D4}
        className="mt-10 flex flex-wrap items-center justify-center gap-6 text-[11px] text-[#888888] uppercase tracking-widest border-t border-[#262626] pt-6"
      >
        <div>[+] 85% LESS MEMORY THAN ELECTRON</div>
        <div>[+] COLD_BOOT: 380MS</div>
        <div>[+] 100% LOCALHOST / AIR-GAPPED</div>
        <div>[+] ATOMIC 1-CLICK COMPILER</div>
      </motion.div>
    </div>
  );
}

function TelemetryBar({ release }: { release: ReleaseData }) {
  return (
    <section className="border-y border-[#262626] bg-[#121212] py-6 font-mono text-xs">
      <div className="mx-auto grid max-w-6xl grid-cols-2 gap-px bg-[#262626] md:grid-cols-4">
        <div className="bg-[#0A0A0A] p-5 text-center">
          <div className="text-[10px] text-[#888888] tracking-widest uppercase">
            [ METRIC_01: MEMORY ]
          </div>
          <div className="mt-2 text-3xl font-black text-[#EAEAEA] tracking-tight sm:text-4xl">
            &lt;100MB
          </div>
          <div className="mt-1 text-[10px] text-[#4AF626] font-bold">8.5X LESS THAN ELECTRON</div>
        </div>

        <div className="bg-[#0A0A0A] p-5 text-center">
          <div className="text-[10px] text-[#888888] tracking-widest uppercase">
            [ METRIC_02: INIT_CYCLE ]
          </div>
          <div className="mt-2 text-3xl font-black text-[#EAEAEA] tracking-tight sm:text-4xl">
            0.38 SEC
          </div>
          <div className="mt-1 text-[10px] text-[#4AF626] font-bold">INSTANT RUST COLD BOOT</div>
        </div>

        <div className="bg-[#0A0A0A] p-5 text-center">
          <div className="text-[10px] text-[#888888] tracking-widest uppercase">
            [ METRIC_03: CONCURRENCY ]
          </div>
          <div className="mt-2 text-3xl font-black text-[#EAEAEA] tracking-tight sm:text-4xl">
            32 THREADS
          </div>
          <div className="mt-1 text-[10px] text-[#4AF626] font-bold">PARALLEL SWARM RUNTIME</div>
        </div>

        <div className="bg-[#0A0A0A] p-5 text-center">
          <div className="text-[10px] text-[#888888] tracking-widest uppercase">
            [ METRIC_04: BINARY_SIZE ]
          </div>
          <div className="mt-2 text-3xl font-black text-[#EAEAEA] tracking-tight sm:text-4xl">
            {release.size}
          </div>
          <div className="mt-1 text-[10px] text-[#4AF626] font-bold">STRIPPED PE32+ NSIS</div>
        </div>
      </div>
    </section>
  );
}

function ComparisonRow({ row }: { row: (typeof COMPARISON_ROWS)[0] }) {
  return (
    <tr className="border-b border-[#262626] bg-[#0A0A0A] text-xs font-mono hover:bg-[#121212] transition-colors">
      <td className="p-4 border-r border-[#262626] text-[#EAEAEA]">
        <div className="text-[10px] text-[#888888]">{row.code}</div>
        <div className="font-bold text-[#EAEAEA] tracking-wider mt-0.5">{row.dimension}</div>
      </td>
      <td className="p-4 border-r border-[#262626] bg-[#121212]">
        <div className="flex items-center gap-2">
          <span className="font-bold text-[#EAEAEA]">{row.kronosSpec}</span>
          <span className="text-[#4AF626] font-bold text-[10px]">{row.kronosStatus}</span>
        </div>
        <p className="mt-1 text-[11px] text-[#888888] leading-relaxed">{row.kronosDesc}</p>
      </td>
      <td className="p-4 bg-[#0A0A0A]">
        <div className="flex items-center gap-2">
          <span className="font-bold text-[#888888] line-through">{row.paseoSpec}</span>
          <span className="text-[#FF2A2A] font-bold text-[10px]">{row.paseoStatus}</span>
        </div>
        <p className="mt-1 text-[11px] text-[#555555] leading-relaxed">{row.paseoDesc}</p>
      </td>
    </tr>
  );
}

function ComparisonTableHeader() {
  return (
    <tr className="border-b border-[#262626] bg-[#121212] text-xs font-mono uppercase tracking-widest text-[#888888]">
      <th className="p-4 w-1/3 border-r border-[#262626]">[ PARAMETER // DIMENSION ]</th>
      <th className="p-4 w-1/3 border-r border-[#262626] text-[#EAEAEA] font-bold bg-[#1C1C1C]">
        [+] KRONOS (APEX TAURI V2 RUST)
      </th>
      <th className="p-4 w-1/3 text-[#888888]">[-] PASEO (LEGACY ELECTRON CHROMIUM)</th>
    </tr>
  );
}

function ComparisonTable() {
  return (
    <div className="overflow-x-auto border border-[#262626]">
      <table className="w-full text-left border-collapse">
        <thead>
          <ComparisonTableHeader />
        </thead>
        <tbody>
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
    <section id="compare" className="scroll-mt-10 py-16 font-mono">
      <div className="mx-auto max-w-6xl px-4">
        <div className="border border-[#262626] bg-[#121212] p-4 text-xs uppercase mb-6 flex flex-col md:flex-row md:items-center md:justify-between gap-3">
          <div className="flex items-center gap-2">
            <span className="bg-[#FF2A2A] text-black font-bold px-2 py-0.5 text-[10px]">
              AUDIT REPORT
            </span>
            <span className="font-bold text-[#EAEAEA] tracking-wider">
              ARCHITECTURAL VERIFICATION MATRIX // REF: KRN-VS-PSO
            </span>
          </div>
          <div className="text-[10px] text-[#888888]">
            VERDICT:{" "}
            <span className="text-[#4AF626] font-bold">[ PASEO OBSOLETE // KRONOS PASS ]</span>
          </div>
        </div>

        <ComparisonTable />
      </div>
    </section>
  );
}

function BentoCard({ card }: { card: (typeof BENTO_CARDS)[0] }) {
  return (
    <div className="border border-[#262626] bg-[#0A0A0A] p-5 font-mono hover:border-[#FF2A2A] transition-colors">
      <div className="flex items-center justify-between border-b border-[#262626] pb-2 text-[10px] text-[#888888]">
        <span>{card.unit}</span>
        <span className="text-[#FF2A2A] font-bold">{card.spec}</span>
      </div>
      <h3 className="mt-4 text-sm font-black text-[#EAEAEA] tracking-wider uppercase">
        {card.title}
      </h3>
      <p className="mt-2 text-xs text-[#888888] leading-relaxed uppercase">{card.desc}</p>
    </div>
  );
}

function BentoSection() {
  return (
    <section
      id="features"
      className="scroll-mt-10 py-16 border-t border-[#262626] bg-[#0A0A0A] font-mono"
    >
      <div className="mx-auto max-w-6xl px-4">
        <div className="border-b border-[#262626] pb-4 mb-8 flex items-center justify-between text-xs uppercase">
          <span className="font-bold text-[#EAEAEA] tracking-wider">
            [ SUBSYSTEM SPECIFICATIONS ]
          </span>
          <span className="text-[#888888]">SECTION 02 // ARCHITECTURE</span>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
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
    <div className="flex items-center justify-between border-b border-[#262626] bg-[#121212] px-4 py-2 font-mono text-xs">
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onSelectBatch}
          className={`px-3 py-1 text-[11px] font-bold uppercase transition-colors ${
            activeTab === "batch"
              ? "bg-[#FF2A2A] text-black"
              : "border border-[#333333] text-[#888888] hover:text-white"
          }`}
        >
          [ 1-CLICK BAT ]
        </button>
        <button
          type="button"
          onClick={onSelectPowershell}
          className={`px-3 py-1 text-[11px] font-bold uppercase transition-colors ${
            activeTab === "powershell"
              ? "bg-[#FF2A2A] text-black"
              : "border border-[#333333] text-[#888888] hover:text-white"
          }`}
        >
          [ NPM SCRIPT ]
        </button>
        <button
          type="button"
          onClick={onSelectClone}
          className={`px-3 py-1 text-[11px] font-bold uppercase transition-colors ${
            activeTab === "clone"
              ? "bg-[#FF2A2A] text-black"
              : "border border-[#333333] text-[#888888] hover:text-white"
          }`}
        >
          [ GIT REPO ]
        </button>
      </div>

      <button
        type="button"
        onClick={onCopy}
        className="flex items-center gap-1.5 border border-[#333333] px-2 py-1 text-[11px] text-[#888888] hover:text-white transition-colors"
      >
        <Copy className="h-3 w-3" />
        <span>{copied ? "[ COPIED ]" : "[ COPY ]"}</span>
      </button>
    </div>
  );
}

function SnippetCode({ activeTab }: { activeTab: "batch" | "powershell" | "clone" }) {
  if (activeTab === "batch") {
    return (
      <>
        <div className="text-[#555555]"># EXECUTE AUTOMATED COMPILER & RELEASE PACKAGER:</div>
        <div className="text-[#EAEAEA] font-bold mt-1">&gt; .\\update-desktop-exe.bat</div>
        <div className="text-[#555555] mt-2">
          # PIPELINE: PULLS GIT DELTAS -&gt; BUILDS WORKSPACES -&gt; OUTPUTS NSIS .EXE
        </div>
      </>
    );
  }
  if (activeTab === "powershell") {
    return (
      <>
        <div className="text-[#555555]"># EXECUTE VIA REPOSITORY PACKAGE MANAGER:</div>
        <div className="text-[#EAEAEA] font-bold mt-1">&gt; npm run desktop:update-build</div>
      </>
    );
  }
  return (
    <>
      <div className="text-[#555555]"># CLONE OFFICIAL HARDENED FORK:</div>
      <div className="text-[#EAEAEA] font-bold mt-1">
        &gt; git clone https://github.com/Xenonesis/Kronos.git
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
      cmd = "git clone https://github.com/Xenonesis/Kronos.git";
    }
    navigator.clipboard.writeText(cmd);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [activeTab]);

  const onSelectBatch = useCallback(() => setActiveTab("batch"), []);
  const onSelectPowershell = useCallback(() => setActiveTab("powershell"), []);
  const onSelectClone = useCallback(() => setActiveTab("clone"), []);

  return (
    <div className="mt-8 w-full max-w-2xl border border-[#262626] bg-[#0A0A0A] text-left">
      <SnippetTabs
        activeTab={activeTab}
        onSelectBatch={onSelectBatch}
        onSelectPowershell={onSelectPowershell}
        onSelectClone={onSelectClone}
        onCopy={onCopy}
        copied={copied}
      />
      <div className="p-4 font-mono text-xs text-[#EAEAEA] leading-relaxed overflow-x-auto">
        <SnippetCode activeTab={activeTab} />
      </div>
    </div>
  );
}

function InstallSection({ release }: { release: ReleaseData }) {
  return (
    <section
      id="download"
      className="scroll-mt-10 py-16 border-t border-[#262626] bg-[#0A0A0A] font-mono"
    >
      <div className="mx-auto max-w-5xl px-4">
        <div className="border border-[#262626] bg-[#121212] p-8 text-center">
          <div className="inline-block bg-[#FF2A2A] text-black font-bold px-3 py-1 text-xs tracking-widest uppercase mb-4">
            DEPLOYMENT MATRIX // VER {release.version.replace(/^v/, "")}
          </div>
          <h2 className="text-3xl font-black uppercase tracking-tight text-[#EAEAEA] sm:text-4xl">
            INSTALL KRONOS WORKSTATION
          </h2>
          <p className="mt-2 text-xs text-[#888888] uppercase tracking-wider max-w-xl mx-auto">
            PRODUCTION-CERTIFIED PE32+ NSIS INSTALLER FOR WINDOWS 10 / 11 X64. PACKAGED WITH LOCAL
            NODE RUNTIME.
          </p>

          <div className="mt-6 flex flex-wrap items-center justify-center gap-4">
            <a
              href={release.downloadUrl}
              className="inline-flex items-center gap-3 bg-[#FF2A2A] px-8 py-4 font-bold text-black hover:bg-white transition-colors tracking-widest text-xs uppercase"
            >
              <Download className="h-4 w-4" />
              <span>&gt;&gt;&gt; DOWNLOAD {release.filename.toUpperCase()}</span>
              <span className="bg-black text-white px-1.5 py-0.5 text-[10px]">{release.size}</span>
            </a>
          </div>

          <div className="mt-3 text-[10px] text-[#555555]">
            SHA256: VERIFIED · PACKAGING: NSIS X64 · TARGET: LOCALHOST AIRGAP
          </div>

          <SnippetCard />
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
    <div className="border border-[#262626] bg-[#0A0A0A]">
      <button
        type="button"
        onClick={handleClick}
        className="flex w-full items-center justify-between p-4 text-left font-bold text-xs uppercase tracking-wider text-[#EAEAEA] hover:bg-[#121212] transition-colors"
      >
        <span className="flex items-center gap-2">
          <span className="text-[#FF2A2A]">{item.code}</span>
          <span>{item.q}</span>
        </span>
        <ChevronRight
          className={`h-4 w-4 text-[#FF2A2A] transition-transform duration-200 ${
            isOpen ? "rotate-90" : ""
          }`}
        />
      </button>
      <AnimatePresence>
        {isOpen && (
          <div className="px-4 pb-4 text-xs text-[#888888] leading-relaxed border-t border-[#262626] pt-3 uppercase">
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
    <section className="border-t border-[#262626] py-16 bg-[#0A0A0A] font-mono">
      <div className="mx-auto max-w-4xl px-4">
        <div className="border-b border-[#262626] pb-3 mb-6 flex items-center justify-between text-xs uppercase">
          <span className="font-bold text-[#EAEAEA] tracking-wider">
            [ OPERATOR MANUAL // FAQ ]
          </span>
          <span className="text-[#888888]">DOCUMENTATION SECTION 04</span>
        </div>
        <div className="space-y-2">
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
  const release = useKronosRelease();
  return (
    <div className="relative w-full bg-[#0A0A0A] text-[#EAEAEA] font-mono selection:bg-[#FF2A2A] selection:text-black crt-scanlines tactical-dots">
      <div className="mx-auto max-w-6xl px-4 pt-4">
        <SiteHeader />
      </div>

      <section className="relative pt-8 pb-16">
        <HeroHeader release={release} />
        <CockpitWindow />
      </section>

      <TelemetryBar release={release} />
      <ComparisonSection />
      <BentoSection />
      <InstallSection release={release} />
      <FaqSection />

      <div className="border-t border-[#262626] bg-[#0A0A0A] py-8">
        <SiteFooter width="wide" sponsors={false} />
      </div>
    </div>
  );
}
