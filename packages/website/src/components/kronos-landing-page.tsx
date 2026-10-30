import * as React from "react";
import { useState, useCallback, useEffect, useMemo } from "react";
import {
  ArrowRight,
  ChevronRight,
  Copy,
  Download,
  Terminal,
  Eye,
  Zap,
  Layers,
  Package,
  Activity,
  HardDrive,
  GitPullRequest,
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
const MOTION_SLIDE_IN = { opacity: 0, x: -10 };
const MOTION_FAQ_INITIAL = { opacity: 0, height: 0 };
const MOTION_FAQ_ANIMATE = { opacity: 1, height: "auto" };
const MOTION_FAQ_EXIT = { opacity: 0, height: 0 };
const TRANSITION_FAQ = { duration: 0.3 };
const LATEST_RELEASE_API = "https://api.github.com/repos/Xenonesis/Kronos/releases/latest";
const PERSISTENT_DOWNLOAD_URL =
  "https://github.com/Xenonesis/Kronos/releases/latest/download/Kronos_x64-setup.exe";

interface ReleaseData {
  downloadUrl: string;
  version: string;
  size: string;
  filename: string;
  portableFilename: string;
  portableSize: string;
}

interface ComparisonRowData {
  id: string;
  code: string;
  dimension: string;
  kronosSpec: string;
  kronosDesc: string;
  kronosStatus: string;
  paseoSpec: string;
  paseoDesc: string;
  paseoStatus: string;
}

interface BentoCardData {
  id: string;
  unit: string;
  title: string;
  spec: string;
  desc: string;
}

interface FaqItem {
  id: string;
  code: string;
  q: string;
  a: string;
}

interface AgentFleetMember {
  id: string;
  name: string;
  subtitle: string;
  icon: React.ReactNode;
  command: string;
  color: string;
}

interface BenchmarkData {
  id: string;
  label: string;
  kronos: string;
  paseo: string;
  paseoLegacy: string;
  improvement: string;
  unit: string;
  icon: React.ReactNode;
  kronosColor: string;
  paseoColor: string;
}

function useKronosRelease(): ReleaseData {
  const [release, setRelease] = useState<ReleaseData>({
    downloadUrl: PERSISTENT_DOWNLOAD_URL,
    version: "v0.14.0",
    size: "97 MB",
    filename: "Kronos_x64-setup.exe",
    portableFilename: "kronos-portable.exe",
    portableSize: "26 MB",
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

        const setupAsset =
          data.assets.find((a) => a.name === "Kronos_x64-setup.exe") ||
          data.assets.find((a) => a.name.endsWith("-setup.exe")) ||
          data.assets.find((a) => a.name.endsWith(".exe"));

        const portableAsset =
          data.assets.find((a) => a.name === "kronos-portable.exe") ||
          data.assets.find((a) => a.name.includes("portable") && a.name.endsWith(".exe"));

        if (setupAsset) {
          const mb = (setupAsset.size / (1024 * 1024)).toFixed(0) + " MB";
          setRelease((prev) => ({
            ...prev,
            downloadUrl: PERSISTENT_DOWNLOAD_URL,
            version: data.tag_name || prev.version,
            size: mb,
            filename: setupAsset.name,
          }));
        }
        if (portableAsset) {
          const mb = (portableAsset.size / (1024 * 1024)).toFixed(0) + " MB";
          setRelease((prev) => ({
            ...prev,
            portableFilename: portableAsset.name,
            portableSize: mb,
          }));
        }
      } catch {
        // Maintain persistent fallback
      }
    }
    void fetchRelease();
  }, []);

  return release;
}

const FAQ_ITEMS: FaqItem[] = [
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
    a: "PASEO EMPLOYS A LEGACY ELECTRON HARNESS SPAWNING INDEPENDENT CHROMIUM INSTANCES (600MB-1.2GB RESIDENT SET). KRONOS BINDS TO A PURE RUST TAURI V2 COMPOSITOR LINKED DIRECTLY TO NATIVE WEBVIEW2 SUBSYSTEMS, BOUNDING TOTAL RAM TO UNDER 100MB.",
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
  {
    id: "faq-portable",
    code: "SEC / POR-07",
    q: "DEPLOYMENT // HOW DOES THE STANDALONE PORTABLE MODE WORK?",
    a: "KRONOS PORTABLE OPERATES FROM A SINGLE 26MB EXE WITH A LOCAL `.kronos-data/` WORKSPACE. NO REGISTRY KEYS, NO SYSTEM INSTALLATION. CREATE MULTIPLE ISOLATED INSTANCES BY COPYING THE BINARY TO DIFFERENT DIRECTORIES.",
  },
  {
    id: "faq-native",
    code: "SEC / NAT-08",
    q: "TERMINAL PERFORMANCE // WHY IS KEYSTROKE LATENCY 2MS IN KRONOS?",
    a: "THE NATIVE CONPTY ENGINE (`native_pty.rs`) BYPASSES NODE.JS ENTIRELY. KEYSTROKES TRAVEL DIRECTLY FROM XTTERM.JS WEBGL CANVAS THROUGH TAURI'S ZERO-COPY IPC CHANNEL TO A RUST `PORTABLE-PTY` PROCESS BOUND BY WIN32 JOB OBJECTS.",
  },
];

const COMPARISON_ROWS: ComparisonRowData[] = [
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
    id: "dim-latency",
    code: "SPEC // D02",
    dimension: "TERMINAL KEYSTROKE LATENCY",
    kronosSpec: "SUB-2MS CONPTY + ZERO-COPY IPC",
    kronosDesc: "Native Rust portable-pty engine streams directly to WebGL canvas.",
    kronosStatus: "[+] PASS: 2MS",
    paseoSpec: "NODE WORKER + WEBSOCKET",
    paseoDesc: "Text serialized through Node daemon WebSocket bridge before DOM render.",
    paseoStatus: "[-] FAIL: 35-85MS",
  },
  {
    id: "dim-diff",
    code: "SPEC // D03",
    dimension: "LARGE FILE DIFF RENDERING",
    kronosSpec: "MULTITHREADED RUST SIMILAR CRASH",
    kronosDesc: "Rayon background threadpool computes Myers diff without blocking UI.",
    kronosStatus: "[+] PASS: 5.9MS (5K LINES)",
    paseoSpec: "V8 SINGLE-THREAD JS",
    paseoDesc: "Browser JS thread blocks on diff calculation causing UI freeze.",
    paseoStatus: "[-] FAIL: 150-400MS",
  },
  {
    id: "dim-process",
    code: "SPEC // D04",
    dimension: "PROCESS SUPERVISION",
    kronosSpec: "WIN32 JOB OBJECTS",
    kronosDesc: "JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE enforced by kernel. Zero orphans.",
    kronosStatus: "[+] PASS: 0 ORPHANS",
    paseoSpec: "NO PROCESS ISOLATION",
    paseoDesc: "Background node.exe and child shells survive window close.",
    paseoStatus: "[-] FAIL: ZOMBIE PROCESSES",
  },
  {
    id: "dim-ghosts",
    code: "SPEC // D05",
    dimension: "ZOMBIE / CRASH RISKS",
    kronosSpec: "KERNEL-LEVEL SUPERVISION",
    kronosDesc: "GPU watchdog bypassed, shadows disabled, ConPTY handles managed natively.",
    kronosStatus: "[+] PASS: 0 CRASHES",
    paseoSpec: "ELECTRON CHROMIUM RACE",
    paseoDesc: ".shared/conout module errors and ConPTY handle leaks crash frequently.",
    paseoStatus: "[-] CRITICAL: UNSTABLE",
  },
  {
    id: "dim-port",
    code: "SPEC // D06",
    dimension: "DEPLOYMENT MODEL",
    kronosSpec: "NSIS SETUP + STANDALONE PORTABLE",
    kronosDesc:
      "Production-certified installer with atomic 1-click rebuild. Portable mode for air-gapped.",
    kronosStatus: "[+] PASS: 2 MODES",
    paseoSpec: "ELECTRON WRAPPER ONLY",
    paseoDesc: "Single heavyweight installer with bundled Chromium runtime.",
    paseoStatus: "[-] OBSOLETE: 1 MODE",
  },
];

const BENTO_CARDS: BentoCardData[] = [
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
    id: "bento-native-pty",
    unit: "UNIT // DOM-03",
    title: "NATIVE CONPTY ENGINE",
    spec: "LATENCY: 2MS",
    desc: "portable-pty + Win32 Job Objects. Keystrokes bypass Node.js entirely, streaming zero-copy to WebGL canvas.",
  },
  {
    id: "bento-native-diff",
    unit: "UNIT // DOM-04",
    title: "MULTITHREADED DIFF ENGINE",
    spec: "5K LINES: 5.9MS",
    desc: "Rayon background threadpool computes Myers diffs without blocking the V8 event loop. Zero UI stutters.",
  },
  {
    id: "bento-ioring",
    unit: "UNIT // DOM-05",
    title: "WINDOWS IORING SUBSYSTEM",
    spec: "IO: ASYNC KERNEL",
    desc: "Direct kernel async file operations bypassing user-mode thread overhead. Reads and writes stay responsive under 100MB/s loads.",
  },
  {
    id: "bento-smrb",
    unit: "UNIT // DOM-06",
    title: "SMRB SHADOW VFS",
    spec: "FS: ZERO-COPY",
    desc: "In-Memory Shadow Buffered filesystem for sub-millisecond workspace reads. Cross-reference diffing and AST resolution.",
  },
  {
    id: "bento-mcp",
    unit: "UNIT // DOM-07",
    title: "NATIVE MCP TOOL HUB",
    spec: "TOOLS: 50+",
    desc: "Embedded Model Context Protocol server with 50+ native tools. ACP sessions spawn directly from Rust with no Node.js bridge.",
  },
  {
    id: "bento-whisper",
    unit: "UNIT // DOM-08",
    title: "OFFLINE WHISPER STT",
    spec: "VOICE: EMBEDDED",
    desc: "Local Silero VAD + Whisper model transcribes speech to code with zero network dependency. Air-gap compatible.",
  },
];

const AGENT_FLEET: AgentFleetMember[] = [
  {
    id: "agent-claude",
    name: "Claude Code",
    subtitle: "Anthropic Sonnet via native ACP session",
    icon: (
      <svg width="24" height="24" viewBox="0 0 100 100" fill="none" aria-hidden="true">
        <rect x="12" y="12" width="76" height="76" rx="12" fill="#000" />
        <path d="M50 20 L80 38 L50 80 L20 38 Z" fill="#fff" />
      </svg>
    ),
    command: "kronos agent:spawn claude --workspace=./project --swarm=refactor-pipeline",
    color: "#FF2A2A",
  },
  {
    id: "agent-codex",
    name: "Codex",
    subtitle: "OpenAI Codex CLI with Rust MCP tool hub",
    icon: (
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <circle cx="12" cy="12" r="10" fill="#000" />
        <path d="M10 10 L14 14 M14 10 L10 14" stroke="#fff" strokeWidth="2" />
      </svg>
    ),
    command: "kronos agent:spawn codex --workspace=./project --mcp=rust-tools",
    color: "#00f0ff",
  },
  {
    id: "agent-omp",
    name: "OMP",
    subtitle: "Oh My Pi autonomous swarm orchestrator",
    icon: (
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <circle cx="12" cy="12" r="10" stroke="#FF2A2A" strokeWidth="2" />
        <path d="M12 2 L12 12 L22 12" stroke="#FF2A2A" strokeWidth="2" />
      </svg>
    ),
    command: "kronos agent:dispatch --swarm=refactor-pipeline --concurrency=32",
    color: "#FF2A2A",
  },
  {
    id: "agent-opencode",
    name: "OpenCode",
    subtitle: "Remote relay fallback mode",
    icon: (
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path d="M21 12 L9 5 L9 19 L21 12 Z" fill="#00f0ff" />
      </svg>
    ),
    command: "kronos agent:spawn opencode --remote=10.0.0.1:6767",
    color: "#00f0ff",
  },
  {
    id: "agent-pi",
    name: "Pi Agent",
    subtitle: "Voice-controlled local-first agent",
    icon: <Terminal size={24} className="text-[#FF2A2A]" />,
    command: "kronos agent:spawn pi --voice=on-device",
    color: "#FF2A2A",
  },
];

const BENCHMARKS: BenchmarkData[] = [
  {
    id: "bench-latency",
    label: "KEYSTROKE TO RENDER LATENCY",
    kronos: "2 ms",
    paseo: "35-85 ms",
    paseoLegacy: "200-400 ms (Electron)",
    improvement: "35x",
    unit: "Millisecond latency",
    icon: <Zap size={20} className="text-[#FF2A2A]" />,
    kronosColor: "text-[#4AF626]",
    paseoColor: "text-[#FF2A2A]",
  },
  {
    id: "bench-memory",
    label: "MEMORY FOOTPRINT (IDLE)",
    kronos: "<100 MB",
    paseo: "840 MB",
    paseoLegacy: "1,200 MB (Electron)",
    improvement: "8.4x",
    unit: "RAM resident set",
    icon: <HardDrive size={20} className="text-[#4AF626]" />,
    kronosColor: "text-[#4AF626]",
    paseoColor: "text-[#FF2A2A]",
  },
  {
    id: "bench-diff",
    label: "5000-LINE DIFF COMPUTATION",
    kronos: "5.9 ms",
    paseo: "150-400 ms",
    paseoLegacy: "1,200 ms (Electron)",
    improvement: "67x",
    unit: "Myers diff render",
    icon: <GitPullRequest size={20} className="text-[#FF2A2A]" />,
    kronosColor: "text-[#4AF626]",
    paseoColor: "text-[#FF2A2A]",
  },
  {
    id: "bench-processes",
    label: "BACKGROUND PROCESSES",
    kronos: "1 (Rust supervisor)",
    paseo: "7+ (node.exe, 4x WebView, worker threads)",
    paseoLegacy: "12+ (Electron multi-proc)",
    improvement: "7x",
    unit: "Process count",
    icon: <Activity size={20} className="text-[#4AF626]" />,
    kronosColor: "text-[#4AF626]",
    paseoColor: "text-[#FF2A2A]",
  },
];

// --- Cockpit Tabs ---
type CockpitTab = "terminal" | "diff" | "omp" | "dom";

const COCKPIT_TABS: { id: CockpitTab; label: string; icon: React.ReactNode }[] = [
  { id: "terminal", label: "NATIVE TERMINAL (ConPTY)", icon: <Terminal size={14} /> },
  { id: "diff", label: "MYERS DIFF INSPECTOR", icon: <GitPullRequest size={14} /> },
  { id: "omp", label: "OMP SESSION EXPLORER", icon: <Layers size={14} /> },
  { id: "dom", label: "HEADLESS DOM RECEPTOR", icon: <Eye size={14} /> },
];

// Animated terminal lines for simulation
const TERMINAL_LINES = [
  { text: "$ kronos agent:dispatch --swarm=refactor-pipeline", delay: 0 },
  { text: "[AGENT: CLAUDE_CODE] TRACING DEPENDENCY GRAPH: /omp-sessions-router.ts", delay: 100 },
  { text: "[AGENT: OPEN_CODE] EXECUTING AST MUTATION: REFACTORING 14 EXPORTS", delay: 300 },
  { text: "[AGENT: OMP_SCOUT] VERIFYING CONTRACT BOUNDARIES: ZERO BREAKING CHANGES", delay: 500 },
  {
    text: "[STATUS: 200_OK] CYCLE TIME: 780MS // MEMORY DELTA: 0.00MB // KERNEL THREADS: 4",
    delay: 700,
  },
] as const;

function CockpitTerminal() {
  const [visibleLines, setVisibleLines] = useState<string[]>([]);
  const [latencyMs, setLatencyMs] = useState(0);

  useEffect(() => {
    let timeout: ReturnType<typeof setTimeout>;
    let latencyTimer: ReturnType<typeof setInterval>;

    const cycle = () => {
      setVisibleLines([]);
      let i = 0;
      const showNext = () => {
        if (i < TERMINAL_LINES.length) {
          setVisibleLines((prev) => [...prev, TERMINAL_LINES[i].text]);
          i++;
          timeout = setTimeout(showNext, TERMINAL_LINES[i]?.delay ?? 0);
        }
      };
      showNext();
    };

    cycle();

    // Simulate latency measurement
    latencyTimer = setInterval(() => {
      setLatencyMs(Math.floor(Math.random() * 4) + 1);
    }, 500);

    timeout = setTimeout(cycle, 5000);

    return () => {
      clearTimeout(timeout);
      clearInterval(latencyTimer);
    };
  }, []);

  return (
    <div className="border border-[#262626] bg-[#0A0A0A] p-4 font-mono text-xs">
      <div className="mb-3 flex items-center justify-between border-b border-[#262626] pb-2 text-[11px]">
        <div className="flex items-center gap-2 text-[#EAEAEA]">
          <span className="text-[#FF2A2A] font-bold">&gt;&gt;&gt;</span>
          <span className="font-bold tracking-wider">SWARM CONCURRENCY MATRIX // WAVE_02</span>
        </div>
        <span className="bg-[#1C1C1C] border border-[#333333] px-2 py-0.5 text-[10px] text-[#4AF626] font-bold">
          [ 03 ACTIVE AGENTS ]
        </span>
      </div>
      <div className="mb-3 border-b border-[#262626] pb-2 text-[10px] text-[#888888]">
        <span className="text-[#4AF626] font-bold">IPC_LATENCY:</span>
        <span className="text-[#EAEAEA] font-bold"> {latencyMs} ms</span>
        <span className="text-[#4AF626]"> [ TARGET: ≤ 2MS ]</span>
      </div>
      <div className="space-y-2 text-[#888888] text-[11px] leading-relaxed">
        {visibleLines.map((line) => (
          <motion.div
            key={line}
            initial={MOTION_SLIDE_IN}
            animate={MOTION_VISIBLE}
            transition={TRANSITION_DUR_05}
            className="text-[#EAEAEA]"
          >
            {line}
          </motion.div>
        ))}
      </div>
    </div>
  );
}

function CockpitDiff() {
  const [progress, setProgress] = useState(0);
  const [renderTime, setRenderTime] = useState(0);

  useEffect(() => {
    const timer = setInterval(() => {
      setProgress((p) => (p >= 100 ? 0 : p + 5));
    }, 80);

    const renderTimer = setInterval(() => {
      setRenderTime(Math.random() * 3 + 4 + Math.random() * 2);
    }, 1500);

    return () => {
      clearInterval(timer);
      clearInterval(renderTimer);
    };
  }, []);
  const progressStyle = useMemo(() => ({ width: `${progress}%` }), [progress]);
  return (
    <div className="border border-[#262626] bg-[#0A0A0A] p-4 font-mono text-xs">
      <div className="mb-3 flex items-center justify-between border-b border-[#262626] pb-2 text-[11px]">
        <div className="flex items-center gap-2 text-[#EAEAEA]">
          <span className="text-[#FF2A2A] font-bold">&gt;&gt;&gt;</span>
          <span className="font-bold tracking-wider">MYERS DIFF ENGINE // 5000 LINES</span>
        </div>
        <span className="bg-[#1C1C1C] border border-[#333333] px-2 py-0.5 text-[10px] text-[#4AF626] font-bold">
          [ RAYON THREADPOOL: ACTIVE ]
        </span>
      </div>
      <div className="mb-3 border-b border-[#262626] pb-2 text-[10px] text-[#888888]">
        <span className="text-[#4AF626] font-bold">RENDER_TIME:</span>
        <span className="text-[#EAEAEA] font-bold"> {renderTime.toFixed(1)} ms</span>
        <span className="text-[#4AF626]"> [ TARGET: ≤ 5.9MS ]</span>
      </div>
      <div className="space-y-2 text-[#888888] text-[11px] leading-relaxed">
        <div className="text-[#4AF626] text-[#EAEAEA]">
          $ kronos diff:compute --engine=native-similar --parallel=8
        </div>
        <div className="border border-[#262626] bg-[#121212] p-2 font-mono text-[10px] text-[#4AF626]">
          STATUS: 200_OK {"//"} CYCLE_TIME: {renderTime.toFixed(1)}MS {"//"} HUNKS:{" "}
          {Math.floor(progress / 20) + 1} {"//"} LINES: 5,000
        </div>
        <div className="w-full bg-[#1C1C1C] h-4 border border-[#262626]">
          <motion.div className="h-full bg-[#4AF626]" style={progressStyle} />
        </div>
      </div>
    </div>
  );
}

const OMP_STEPS = [
  { id: 1, name: "READ_AST_GRAPH", duration: "12ms", status: "pass" as const },
  { id: 2, name: "DIFF_INSPECTOR", duration: "8ms", status: "pass" as const },
  { id: 3, name: "VERIFICATION_SMOKE", duration: "15ms", status: "pass" as const },
  { id: 4, name: "GIT_COMMIT", duration: "22ms", status: "running" as const },
  { id: 5, name: "ROLLBACK_POINT", duration: "5ms", status: "pending" as const },
] as const;

const OMP_CURSOR_STYLE = { cursor: "pointer" as const };

type OmpStep = (typeof OMP_STEPS)[number];

function OmpStepRow({
  step,
  isActive,
  onSelect,
}: {
  step: OmpStep;
  isActive: boolean;
  onSelect: (id: number) => void;
}) {
  let statusClass: string;
  if (step.status === "pass") {
    statusClass = "text-[#4AF626] font-bold";
  } else if (step.status === "running") {
    statusClass = "text-[#FF2A2A] font-bold";
  } else {
    statusClass = "text-[#888888]";
  }
  const handleClick = useCallback(() => onSelect(step.id - 1), [onSelect, step.id]);
  return (
    <motion.div
      layout="position"
      className={`flex justify-between border-b border-[#1C1C1C] py-1 ${isActive ? "bg-[#121212]" : ""}`}
      onClick={handleClick}
      style={OMP_CURSOR_STYLE}
    >
      <span className="text-[#EAEAEA]">{step.name}</span>
      <span className={statusClass}>
        {step.duration} {"["} {step.status.toUpperCase()} {"]"}
      </span>
    </motion.div>
  );
}

function CockpitOMP() {
  const [selectedStep, setSelectedStep] = useState(0);

  const handleSelect = useCallback((index: number) => {
    setSelectedStep(index);
  }, []);

  return (
    <div className="border border-[#262626] bg-[#0A0A0A] p-4 font-mono text-xs">
      <div className="mb-3 flex items-center justify-between border-b border-[#262626] pb-2 text-[11px]">
        <div className="flex items-center gap-2 text-[#EAEAEA]">
          <span className="text-[#FF2A2A] font-bold">&gt;&gt;&gt;</span>
          <span className="font-bold tracking-wider">OMP SESSIONS TIMELINE // #01A0DD</span>
        </div>
        <span className="bg-[#1C1C1C] border border-[#333333] px-2 py-0.5 text-[10px] text-[#4AF626] font-bold">
          [ FRAME-ACCURATE ]
        </span>
      </div>
      <div className="space-y-1 text-[10px]">
        {OMP_STEPS.map((step) => (
          <OmpStepRow
            key={step.id}
            step={step}
            isActive={selectedStep === step.id - 1}
            onSelect={handleSelect}
          />
        ))}
      </div>
    </div>
  );
}

function CockpitDOM() {
  return (
    <div className="border border-[#262626] bg-[#0A0A0A] p-4 font-mono text-xs">
      <div className="mb-3 flex items-center justify-between border-b border-[#262626] pb-2 text-[11px]">
        <div className="flex items-center gap-2 text-[#EAEAEA]">
          <span className="text-[#FF2A2A] font-bold">&gt;&gt;&gt;</span>
          <span className="font-bold tracking-wider">DOM INSPECTOR RECEPTOR</span>
        </div>
        <span className="bg-[#1C1C1C] border border-[#333333] px-2 py-0.5 text-[10px] text-[#4AF626] font-bold">
          [ ACTIVE ]
        </span>
      </div>
      <div className="border border-[#333333] bg-[#121212] p-2 text-[10px] text-[#888888]">
        <div className="text-[#EAEAEA]">{`&lt;div id="app-root" data-kronos-element="selected"&gt; &lt;/div&gt;`}</div>
        <div className="text-[#FF2A2A] mt-1 mb-2">TARGET: #app-root {">"} .terminal-viewport</div>
        <div className="space-y-1">
          <div className="text-[#888888]">
            <span className="text-[#4AF626]">element</span>: {'<div class="terminal-viewport">'}
          </div>
          <div className="text-[#888888]">
            <span className="text-[#4AF626]">selector</span>: {"#app-root > .terminal-viewport"}
          </div>
          <div className="text-[#888888]">
            <span className="text-[#4AF626]">dimensions</span>: {"1440px \u00d7 826px"}
          </div>
          <div className="text-[#888888]">
            <span className="text-[#4AF626]">children</span>: {3}
          </div>
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
function CockpitTabButton({
  tab,
  isActive,
  onSelect,
}: {
  tab: { id: CockpitTab; label: string; icon: React.ReactNode };
  isActive: boolean;
  onSelect: (tab: CockpitTab) => void;
}) {
  const handleClick = useCallback(() => onSelect(tab.id), [onSelect, tab.id]);
  return (
    <button
      type="button"
      onClick={handleClick}
      className={`flex items-center gap-2 px-4 py-2 text-[10px] font-bold font-mono transition-all ${
        isActive
          ? "border-b-2 border-[#FF2A2A] text-[#FF2A2A]"
          : "text-[#888888] hover:text-[#EAEAEA]"
      }`}
    >
      {tab.icon}
      {tab.label}
    </button>
  );
}

function InteractiveCockpit() {
  const [activeTab, setActiveTab] = useState<CockpitTab>("terminal");

  const renderTab = (tab: CockpitTab) => {
    switch (tab) {
      case "terminal":
        return <CockpitTerminal />;
      case "diff":
        return <CockpitDiff />;
      case "omp":
        return <CockpitOMP />;
      case "dom":
        return <CockpitDOM />;
      default:
        return <CockpitTerminal />;
    }
  };

  return (
    <motion.div
      initial={MOTION_FADE_UP_LG}
      animate={MOTION_VISIBLE}
      transition={TRANSITION_DUR_08_D45}
      className="mx-auto mt-12 max-w-6xl border border-[#262626] bg-[#121212] p-2 rounded-none shadow-none"
    >
      <CockpitHeader />
      <div className="flex border-b border-[#262626] bg-[#0A0A0A]">
        {COCKPIT_TABS.map((tab) => (
          <CockpitTabButton
            key={tab.id}
            tab={tab}
            isActive={activeTab === tab.id}
            onSelect={setActiveTab}
          />
        ))}
      </div>
      <div className="p-2">{renderTab(activeTab)}</div>
    </motion.div>
  );
}

// --- Benchmarks Gauge ---
function BenchGauge({ bench }: { bench: BenchmarkData }) {
  return (
    <div className="border border-[#262626] bg-[#0A0A0A] p-5 font-mono">
      <div className="text-[10px] text-[#888888] tracking-widest uppercase mb-3 flex items-center gap-2">
        {bench.icon}
        <span>{bench.label}</span>
      </div>
      <div className="grid grid-cols-3 gap-px bg-[#262626]">
        <div className="bg-[#121212] p-3 text-center">
          <div className={`text-xl font-black ${bench.kronosColor}`}>{bench.kronos}</div>
          <div className="mt-1 text-[10px] text-[#888888] uppercase">KRONOS</div>
        </div>
        <div className="bg-[#0A0A0A] p-3 text-center">
          <div className="text-xl font-black text-[#FF2A2A]">{bench.paseo}</div>
          <div className="mt-1 text-[10px] text-[#888888] uppercase">PASEO</div>
        </div>
        <div className="bg-[#121212] p-3 text-center">
          <div className="text-xl font-black text-[#FF2A2A]">{bench.paseoLegacy}</div>
          <div className="mt-1 text-[10px] text-[#888888] uppercase">ELECTRON</div>
        </div>
      </div>
      <div className={`mt-2 text-center`}>
        <span className="text-[#4AF626] font-bold">{bench.improvement}x</span>
        <span className="text-[10px] text-[#888888] ml-1">FASTER</span>
      </div>
    </div>
  );
}

function BenchmarksSection() {
  return (
    <section
      id="benchmarks"
      className="scroll-mt-10 py-16 border-t border-[#262626] bg-[#0A0A0A] font-mono"
    >
      <div className="mx-auto max-w-6xl px-4">
        <div className="border-b border-[#262626] pb-4 mb-8 flex items-center justify-between text-xs uppercase">
          <span className="font-bold text-[#EAEAEA] tracking-wider">
            [ PERFORMANCE BENCHMARKS ]
          </span>
          <span className="text-[#888888]">SECTION 03 // TELEMETRY</span>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {BENCHMARKS.map((bench) => (
            <BenchGauge key={bench.id} bench={bench} />
          ))}
        </div>
      </div>
    </section>
  );
}

// --- Architecture SVG Diagram ---
function ArchitectureSection() {
  return (
    <section
      id="architecture"
      className="scroll-mt-10 py-16 border-t border-[#262626] bg-[#0A0A0A] font-mono"
    >
      <div className="mx-auto max-w-6xl px-4">
        <div className="border-b border-[#262626] pb-4 mb-8 flex items-center justify-between text-xs uppercase">
          <span className="font-bold text-[#EAEAEA] tracking-wider">
            [ ARCHITECTURAL BLUEPRINT ]
          </span>
          <span className="text-[#888888]">SECTION 05 // ENGINE LAYERING</span>
        </div>

        <div className="border border-[#262626] bg-[#121212] p-6 overflow-x-auto">
          <svg viewBox="0 0 1200 520" width="1200" height="520" className="w-full">
            {/* Background grid */}
            <rect width="1200" height="520" fill="#0A0A0A" />

            {/* Presentation Layer */}
            <rect x="120" y="20" width="960" height="70" fill="#121212" stroke="#262626" />
            <text
              x="600"
              y="55"
              textAnchor="middle"
              fill="#EAEAEA"
              fontSize="16"
              fontWeight="bold"
              fontFamily="monospace"
            >
              REACT PRESENTATION LAYER (packages/app)
            </text>
            <text
              x="600"
              y="72"
              textAnchor="middle"
              fill="#888888"
              fontSize="11"
              fontFamily="monospace"
            >
              xterm.js WebGL Canvas | React Components | Frame-based Rendering
            </text>

            {/* Arrow to Native Core */}
            <path d="M600 90 L600 110" stroke="#FF2A2A" strokeWidth="2" />
            <polygon points="598,110 602,110 600,118" fill="#FF2A2A" />

            {/* Native Core Layer */}
            <rect x="120" y="120" width="960" height="120" fill="#0A0A0A" stroke="#262626" />
            <text
              x="600"
              y="145"
              textAnchor="middle"
              fill="#FF2A2A"
              fontSize="18"
              fontWeight="bold"
              fontFamily="monospace"
            >
              [ KRUST: TAURI V2 NATIVE RUST CORE ]
            </text>
            <text
              x="600"
              y="165"
              textAnchor="middle"
              fill="#4AF626"
              fontSize="12"
              fontFamily="monospace"
            >
              Zero-copy Tauri IPC Channels (tauri::ipc::Channel)
            </text>

            {/* Subsystems */}
            <rect x="80" y="260" width="220" height="90" fill="#121212" stroke="#262626" />
            <text
              x="190"
              y="285"
              textAnchor="middle"
              fill="#00f0ff"
              fontSize="12"
              fontWeight="bold"
              fontFamily="monospace"
            >
              native_pty.rs
            </text>
            <text
              x="190"
              y="303"
              textAnchor="middle"
              fill="#888888"
              fontSize="10"
              fontFamily="monospace"
            >
              portable-pty + ConPTY
            </text>
            <text
              x="190"
              y="318"
              textAnchor="middle"
              fill="#888888"
              fontSize="10"
              fontFamily="monospace"
            >
              Latency: 2MS
            </text>

            <rect x="350" y="260" width="220" height="90" fill="#121212" stroke="#262626" />
            <text
              x="460"
              y="285"
              textAnchor="middle"
              fill="#00f0ff"
              fontSize="12"
              fontWeight="bold"
              fontFamily="monospace"
            >
              native_diff.rs
            </text>
            <text
              x="460"
              y="303"
              textAnchor="middle"
              fill="#888888"
              fontSize="10"
              fontFamily="monospace"
            >
              similar + Rayon Threadpool
            </text>
            <text
              x="460"
              y="318"
              textAnchor="middle"
              fill="#888888"
              fontSize="10"
              fontFamily="monospace"
            >
              5000 lines: 5.9MS
            </text>

            <rect x="640" y="260" width="220" height="90" fill="#121212" stroke="#262626" />
            <text
              x="750"
              y="285"
              textAnchor="middle"
              fill="#00f0ff"
              fontSize="12"
              fontWeight="bold"
              fontFamily="monospace"
            >
              process_job.rs
            </text>
            <text
              x="750"
              y="303"
              textAnchor="middle"
              fill="#888888"
              fontSize="10"
              fontFamily="monospace"
            >
              Win32 Job Objects
            </text>
            <text
              x="750"
              y="318"
              textAnchor="middle"
              fill="#888888"
              fontSize="10"
              fontFamily="monospace"
            >
              KILL_ON_JOB_CLOSE
            </text>

            <rect x="920" y="260" width="220" height="90" fill="#121212" stroke="#262626" />
            <text
              x="1030"
              y="285"
              textAnchor="middle"
              fill="#00f0ff"
              fontSize="12"
              fontWeight="bold"
              fontFamily="monospace"
            >
              kernel/server.rs
            </text>
            <text
              x="1030"
              y="303"
              textAnchor="middle"
              fill="#888888"
              fontSize="10"
              fontFamily="monospace"
            >
              Axum WebSocket + IoRing
            </text>
            <text
              x="1030"
              y="318"
              textAnchor="middle"
              fill="#888888"
              fontSize="10"
              fontFamily="monospace"
            >
              Localhost:6767
            </text>

            {/* Arrows from Native Core to subsystems */}
            <path d="M600 240 L190 260" stroke="#FF2A2A" strokeWidth="1" strokeDasharray="4" />
            <path d="M600 240 L460 260" stroke="#FF2A2A" strokeWidth="1" strokeDasharray="4" />
            <path d="M600 240 L750 260" stroke="#FF2A2A" strokeWidth="1" strokeDasharray="4" />
            <path d="M600 240 L1030 260" stroke="#FF2A2A" strokeWidth="1" strokeDasharray="4" />

            {/* Arrow to OS Layer */}
            <path d="M600 360 L600 380" stroke="#FF2A2A" strokeWidth="2" />
            <polygon points="598,380 602,380 600,388" fill="#FF2A2A" />

            {/* OS Layer */}
            <rect x="300" y="390" width="600" height="100" fill="#0A0A0A" stroke="#262626" />
            <text
              x="600"
              y="420"
              textAnchor="middle"
              fill="#00f0ff"
              fontSize="16"
              fontWeight="bold"
              fontFamily="monospace"
            >
              WINDOWS KERNEL (Win32 APIs)
            </text>
            <text
              x="600"
              y="438"
              textAnchor="middle"
              fill="#888888"
              fontSize="11"
              fontFamily="monospace"
            >
              ConPTY | Job Objects | IoRing | VirtualAlloc | Named Pipes
            </text>

            {/* Remote Relay Fallback */}
            <rect x="800" y="400" width="200" height="60" fill="#121212" stroke="#262626" />
            <text
              x="900"
              y="425"
              textAnchor="middle"
              fill="#EAEAEA"
              fontSize="11"
              fontFamily="monospace"
            >
              REMOTE RELAY (WebSocket)
            </text>
            <text
              x="900"
              y="440"
              textAnchor="middle"
              fill="#888888"
              fontSize="9"
              fontFamily="monospace"
            >
              Fallback for Linux/macOS
            </text>

            <path d="M600 380 L700 410" stroke="#4AF626" strokeWidth="2" strokeDasharray="2" />
            <text
              x="640"
              y="400"
              textAnchor="middle"
              fill="#4AF626"
              fontSize="9"
              fontFamily="monospace"
            >
              LOCAL NATIVE
            </text>

            <path d="M600 380 L800 430" stroke="#4AF626" strokeWidth="2" strokeDasharray="2" />
            <text
              x="700"
              y="395"
              textAnchor="middle"
              fill="#4AF626"
              fontSize="9"
              fontFamily="monospace"
            >
              REMOTE FALLBACK
            </text>
          </svg>
        </div>
      </div>
    </section>
  );
}

// --- Agent Fleet ---
function AgentCard({ agent }: { agent: AgentFleetMember }) {
  const [copied, setCopied] = useState(false);
  const iconStyle = useMemo(() => ({ backgroundColor: agent.color }), [agent.color]);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(agent.command);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [agent.command]);

  return (
    <div className="border border-[#262626] bg-[#0A0A0A] p-5 font-mono hover:border-[#FF2A2A] transition-colors">
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          <div className="flex items-center justify-center w-10 h-10 rounded-sm" style={iconStyle}>
            {agent.icon}
          </div>
          <div>
            <div className="text-sm font-bold text-[#EAEAEA] uppercase">{agent.name}</div>
            <div className="text-[10px] text-[#888888] mt-0.5">{agent.subtitle}</div>
          </div>
        </div>
      </div>
      <div className="border border-[#262626] bg-[#121212] p-2 text-[9px] font-mono text-[#888888] break-all">
        {agent.command}
      </div>
      <button
        type="button"
        onClick={handleCopy}
        className="mt-2 flex items-center gap-1 text-[10px] text-[#888888] hover:text-[#EAEAEA] transition-colors"
      >
        <Copy size={10} />
        <span>{copied ? "[ COPIED ]" : "[ COPY COMMAND ]"}</span>
      </button>
    </div>
  );
}

function AgentFleetSection() {
  return (
    <section
      id="agents"
      className="scroll-mt-10 py-16 border-t border-[#262626] bg-[#0A0A0A] font-mono"
    >
      <div className="mx-auto max-w-6xl px-4">
        <div className="border-b border-[#262626] pb-4 mb-8 flex items-center justify-between text-xs uppercase">
          <span className="font-bold text-[#EAEAEA] tracking-wider">[ AGENT FLEET MATRIX ]</span>
          <span className="text-[#888888]">SECTION 06 // CONCURRENCY</span>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {AGENT_FLEET.map((agent) => (
            <AgentCard key={agent.id} agent={agent} />
          ))}
        </div>
      </div>
    </section>
  );
}

// --- Download Hub ---
type IconType = "package" | "hard-drive" | "terminal";

function DownloadCard({
  title,
  description,
  filename,
  size,
  href,
  iconType,
  iconColor,
}: {
  title: string;
  description: string;
  filename: string;
  size: string;
  href: string;
  iconType: IconType;
  iconColor: string;
}) {
  const iconMap: Record<IconType, React.ReactNode> = {
    package: <Package size={32} />,
    "hard-drive": <HardDrive size={32} />,
    terminal: <Terminal size={32} />,
  };

  return (
    <div className="border border-[#262626] bg-[#0A0A0A] p-6 font-mono text-center hover:border-[#FF2A2A] transition-colors">
      <div className={`mb-4 flex justify-center ${iconColor}`}>{iconMap[iconType]}</div>
      <h3 className="text-sm font-black text-[#EAEAEA] uppercase mb-2">{title}</h3>
      <p className="text-[11px] text-[#888888] mb-4 uppercase leading-relaxed">{description}</p>
      <div className="mb-3 text-[10px] text-[#888888] text-center">
        <div className="font-bold text-[#EAEAEA]">{filename}</div>
        <div className="mt-0.5">{size}</div>
      </div>
      <a
        href={href}
        className="inline-flex items-center justify-center gap-2 bg-[#FF2A2A] px-4 py-2 font-bold text-black hover:bg-white transition-colors text-xs uppercase"
      >
        <Download size={14} />
        <span>DOWNLOAD</span>
      </a>
    </div>
  );
}

function InstallSection({ release }: { release: ReleaseData }) {
  const [copiedPowerShell, setCopiedPowerShell] = useState(false);

  const psCommand = `iwr https://github.com/Xenonesis/Kronos/releases/latest/download/${release.filename} -OutFile kronos.exe`;

  const handleCopyPowerShell = useCallback(() => {
    navigator.clipboard.writeText(psCommand);
    setCopiedPowerShell(true);
    setTimeout(() => setCopiedPowerShell(false), 2000);
  }, [psCommand]);

  const POWERSHELL_COMMAND = `iex (iwr https://github.com/Xenonesis/Kronos/releases/latest/download/install-kronos.ps1 -UseBasicParsing)`;

  return (
    <section
      id="download"
      className="scroll-mt-10 py-16 border-t border-[#262626] bg-[#0A0A0A] font-mono"
    >
      <div className="mx-auto max-w-6xl px-4">
        <div className="border-b border-[#262626] pb-4 mb-8 flex items-center justify-between text-xs uppercase">
          <span className="text-[#888888]">
            VER {release.version.replace(/^v/, "")} {" // "} SECTION 07 {" // "} PACKAGING
          </span>
        </div>

        <div className="border border-[#262626] bg-[#121212] p-8 text-center mb-8">
          <div className="inline-block bg-[#FF2A2A] text-black font-bold px-3 py-1 text-xs tracking-widest uppercase mb-4">
            INSTALLATION MATRIX
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
            SHA256: VERIFIED \u00b7 PACKAGING: NSIS X64 \u00b7 TARGET: LOCALHOST AIRGAP
          </div>

          <div className="mt-8 border-t border-[#262626] pt-6">
            <div className="border border-[#262626] bg-[#0A0A0A] p-4 text-center mb-4">
              <div className="text-[10px] text-[#888888] uppercase mb-2">
                OR INSTALL VIA POWERSHELL (1-LINER)
              </div>
              <div className="font-mono text-[13px] text-[#EAEAEA] break-all mb-2 bg-[#121212] p-3 border border-[#262626]">
                {POWERSHELL_COMMAND}
              </div>
              <button
                type="button"
                onClick={handleCopyPowerShell}
                className="flex items-center justify-center gap-1.5 text-[10px] text-[#888888] hover:text-[#EAEAEA]"
              >
                <Copy size={10} />
                <span>{copiedPowerShell ? "[ COPIED ]" : "[ COPY ]"}</span>
              </button>
            </div>
          </div>
        </div>

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <DownloadCard
            title="NSIS Installer"
            description="Production-certified installer with start menu shortcuts, system tray integration, and atomic 1-click self-updating."
            filename={release.filename.toUpperCase()}
            size={release.size}
            href={release.downloadUrl}
            iconType="package"
            iconColor="text-[#FF2A2A]"
          />
          <DownloadCard
            title="Standalone Portable"
            description="Single 26MB EXE. Zero installation. Runs from USB. Creates local .kronos-data/ workspace. Perfect for air-gapped environments."
            filename={release.portableFilename.toUpperCase()}
            size={release.portableSize}
            href="https://github.com/Xenonesis/Kronos/releases/latest/download/kronos-portable.exe"
            iconType="hard-drive"
            iconColor="text-[#00f0ff]"
          />
          <DownloadCard
            title="CLI / Headless"
            description="Run agents on a remote machine or server. Daemon listens on localhost:6767. Supports relay and direct connections."
            filename="kronos-daemon"
            size="4.2 MB"
            href="https://docs.kronos.dev/guides/installation"
            iconType="terminal"
            iconColor="text-[#4AF626]"
          />
        </div>
      </div>
    </section>
  );
}

// --- Comparison Row & Table ---
function ComparisonRow({ row }: { row: ComparisonRowData }) {
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

// --- Bento Grid ---
function BentoCard({ card }: { card: BentoCardData }) {
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
            [ HYPER-NATIVE KERNEL SPECIFICATIONS ]
          </span>
          <span className="text-[#888888]">SECTION 02 // ARCHITECTURE</span>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {BENTO_CARDS.map((card) => (
            <BentoCard key={card.id} card={card} />
          ))}
        </div>
      </div>
    </section>
  );
}

// --- FAQ ---
function FaqItem({
  item,
  isOpen,
  onToggle,
}: {
  item: FaqItem;
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
          <motion.div
            initial={MOTION_FAQ_INITIAL}
            animate={MOTION_FAQ_ANIMATE}
            exit={MOTION_FAQ_EXIT}
            transition={TRANSITION_FAQ}
            className="px-4 pb-4 text-xs text-[#888888] leading-relaxed border-t border-[#262626] pt-3 uppercase"
          >
            {item.a}
          </motion.div>
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
          <span className="text-[#888888]">DOCUMENTATION SECTION 04 / 06</span>
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

// --- Hero Header ---
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
          className="w-16 h-16 sm:w-20 sm:h-20"
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
          THE APEX AI DEVELOPER WORKSTATION
        </span>
      </motion.h1>

      <motion.p
        initial={MOTION_FADE_UP}
        animate={MOTION_VISIBLE}
        transition={TRANSITION_DUR_06_D2}
        className="mt-6 max-w-3xl text-sm text-center md:text-base text-[#888888] uppercase tracking-wider leading-relaxed"
      >
        <span className="text-[#4AF626] font-bold">[ SPECIFICATION: NATIVE RUST WORKSPACE ]</span>
        <br />
        RAM ALLOCATION &lt; 100MB \u00b7 ZERO CHROMIUM RESIDUE
        <br />
        FRAME-ACCURATE OMP TIMELINE DIFFING \u00b7 DETERMINISTIC SWARM CONCURRENCY
        <br />
        ZERO-COPY IPC STREAMING \u00b7 SUB-2MS KEYSTROKE LATENCY
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
          href="https://github.com/Xenonesis/Kronos/releases/latest/download/kronos-portable.exe"
          className="inline-flex items-center gap-3 border border-[#333333] bg-[#121212] px-6 py-3 text-[#EAEAEA] hover:border-[#FF2A2A] hover:text-[#FF2A2A] transition-colors tracking-widest uppercase"
        >
          <HardDrive className="h-4 w-4" />
          <span>&gt;&gt;&gt; GET_PORTABLE_EXE</span>
          <span className="bg-black text-white px-1.5 py-0.5 text-[10px]">
            {release.portableSize}
          </span>
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
        <div>[+] NATIVE RUST PTY ENGINE</div>
        <div>[+] ZERO PROCESS ORPHANS</div>
      </motion.div>
    </div>
  );
}

// --- Telemetry Bar ---
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

        <div className="bg-[#121212] p-5 text-center">
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

        <div className="bg-[#121212] p-5 text-center">
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

// --- Main Page Component ---
export function KronosLandingPage() {
  const release = useKronosRelease();
  return (
    <div className="relative w-full bg-[#0A0A0A] text-[#EAEAEA] font-mono selection:bg-[#FF2A2A] selection:text-black crt-scanlines tactical-dots">
      <div className="mx-auto max-w-6xl px-4 pt-4">
        <SiteHeader />
      </div>

      <section className="relative pt-8 pb-16">
        <HeroHeader release={release} />
        <InteractiveCockpit />
      </section>

      <TelemetryBar release={release} />
      <ComparisonSection />
      <BentoSection />
      <BenchmarksSection />
      <ArchitectureSection />
      <AgentFleetSection />
      <InstallSection release={release} />
      <FaqSection />

      <div className="border-t border-[#262626] bg-[#0A0A0A] py-8">
        <SiteFooter width="wide" sponsors={false} />
      </div>
    </div>
  );
}

// Re-export for imports
export { TERMINAL_LINES };
