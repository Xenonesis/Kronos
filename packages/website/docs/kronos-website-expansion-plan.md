# Kronos Website Expansion Design Specification

## Current State

- `packages/website` uses TanStack Router (`@tanstack/react-start`) + Vite + Tailwind CSS v3
- Landing page: `src/components/kronos-landing-page.tsx` — single scroll page with Hero, CockpitWindow (static text mockup), TelemetryBar, ComparisonTable, BentoGrid, InstallSection, FAQ
- Mockup system: `src/components/mockup/` — `window.tsx`, `sidebar.tsx`, `chat.tsx`, `right-panel.tsx`, `diff.tsx`, mobile, with real-app pixel dimensions (1440×826) and CSS color tokens (`--color-mock-*`)
- Theme: Industrial Brutalism / CRT Telemetry (`#0A0A0A`, `#EAEAEA`, `#FF2A2A`, monospaced, `crt-scanlines`, `tactical-dots`)
- Paseo (`paseo.sh`) reference: comprehensive multi-section landing with interactive app mockup, agent grid, benchmarks, run-anywhere, social proof, FAQ

## Target: Expand to Paseo.sh-scale

### Sections to Add/Refactor

1. **Header Nav** — Full nav bar with section links + download type selector
2. **Hero** — Refined copy + dual download buttons (Setup / Portable) + PowerShell 1-liner
3. **Interactive Cockpit** — Tabbed mockup with live simulation:
   - Tab 1: Native Terminal (PTY) — animated shell with 2ms latency indicator
   - Tab 2: Myers Diff Inspector — animated diff rendering with 5.9ms timer
   - Tab 3: OMP Session Explorer — interactive AST diff tree + rollback points
   - Tab 4: DOM Inspector — live CDP receptor showing element selection
4. **Telemetry Benchmarks** — 3 gauges: Keystroke Latency (2ms vs 35-85ms), RAM (74MB vs 840MB), Diff Speed (5.9ms vs 380ms)
5. **Agent Fleet** — Interactive grid (Claude Code, Codex, OMP, OpenCode, Pi + copyable dispatch commands)
6. **Architecture Deep-Dive** — SVG diagram of progressive native engine
7. **Bento Grid** — Extended with Hyper-Native Kernel features (IoRing, SMRB, Rust MCP, Whisper STT)
8. **Download Hub** — Card layout: NSIS Installer, Portable EXE, CLI/Headless, PowerShell installer
9. **FAQ** — Expanded to 8 engineering Q&As
10. **Footer** — Kronos-specific links

### Implementation Strategy

- Extend existing `kronos-landing-page.tsx` — preserve working structure, add sections
- Reuse mockup color tokens & geometry from `src/components/mockup/`
- Add interactive simulation using `@xterm/xterm`-style ANSI rendering in CSS
- Use Framer Motion tabs for cockpit mockup (consistent with existing `HeroMockup` tab pattern)

### File Changes

- `src/components/kronos-landing-page.tsx` — main refactor
- Add new subcomponents in `src/components/` or within the landing page file
- `src/i18n/resources/en.json` + `zh-CN.json` — add translation keys

### Verification

1. `npx tsc --noEmit` passes
2. `npm run build` succeeds
3. Manual review of expanded sections
