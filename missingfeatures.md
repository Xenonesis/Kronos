# Kronos Missing Competitor Features & Parity Roadmap

This document tracks all 17 features identified from the comprehensive competitor audit (Cursor, Windsurf, Devin, Claude Code, Cline / Roo Code, Bolt.new, Lovable, Zed AI, and DeepMind Antigravity) that have been methodically implemented and verified in Kronos.

---

## Roadmap Overview & Status Matrix

| ID          | Feature                                                             | Competitor Target              | Priority |         Status          |
| :---------- | :------------------------------------------------------------------ | :----------------------------- | :------: | :---------------------: |
| **FEAT-1**  | **Background Shadow Linter & Diagnostics Loop**                     | Cursor                         |    P0    | ✅ Completed & Verified |
| **FEAT-2**  | **Editor Inline `Ctrl+K` Prompt & In-Place Diff Generator**         | Cursor / Windsurf              |    P0    | ✅ Completed & Verified |
| **FEAT-3**  | **1-Click GitHub Pull Request (PR) Generator with AI Summary**      | Cursor / Devin / Claude Code   |    P0    | ✅ Completed & Verified |
| **FEAT-4**  | **Global Workspace Search & Replace (Ripgrep + Diff Preview)**      | VS Code / Zed                  |    P1    | ✅ Completed & Verified |
| **FEAT-5**  | **Visual Artifacts Canvas & Mermaid Architecture Renderer**         | Claude Artifacts / Antigravity |    P1    | ✅ Completed & Verified |
| **FEAT-6**  | **1-Click Cloud Deployment Engine (Vercel / Netlify / Cloudflare)** | Bolt.new / Lovable             |    P1    | ✅ Completed & Verified |
| **FEAT-7**  | **Codebase Semantic Vector Search & Hybrid RAG Indexer**            | Cursor Codebase Indexing       |    P2    | ✅ Completed & Verified |
| **FEAT-8**  | **Autonomous Browser Testing & Screenshot Verification Loop**       | Cline / Roo Code / Devin       |    P2    | ✅ Completed & Verified |
| **FEAT-9**  | **Native Web Search & Web Scraping Engine for Any Model**           | Claude Code / Devin / OpenCode |    P0    | ✅ Completed & Verified |
| **FEAT-10** | **Inline Predictive Ghost-Text Autocomplete ("Cursor Tab")**        | Cursor Copilot++               |    P0    | ✅ Completed & Verified |
| **FEAT-11** | **Global Codebase Symbol Repo Map Engine**                          | Aider Repo Map                 |    P1    | ✅ Completed & Verified |
| **FEAT-12** | **Active Tab & Cursor Flow Context Auto-Tracking**                  | Windsurf Cascade Flow          |    P1    | ✅ Completed & Verified |
| **FEAT-13** | **Live Side-by-Side Webview Preview Panel with Hot-Reload**         | Bolt.new / Lovable / v0        |    P1    | ✅ Completed & Verified |
| **FEAT-14** | **Specialized Agent Execution Modes (Architect/Code/Ask/Test)**     | Roo Code / Cline               |    P1    | ✅ Completed & Verified |
| **FEAT-15** | **Terminal YOLO & Command Auto-Approval Permission Selector**       | Claude Code                    |    P2    | ✅ Completed & Verified |
| **FEAT-16** | **Hands-Free Voice Dictation Mode (Web Speech / Whisper)**          | Cursor Voice                   |    P2    | ✅ Completed & Verified |
| **FEAT-17** | **Autonomous Multi-Agent Swarm & Subagent Coordinator**             | Antigravity / Devin            |    P2    | ✅ Completed & Verified |

---

## Detailed Implementation Summary

### 1. FEAT-1: Background Shadow Linter & Diagnostics Loop (Cursor Parity)

- **Status**: ✅ Completed & Verified.
- **Backend**: `desktop-tauri/src-tauri/src/agent/self_healing.rs` & `runner.rs`.
  - Non-destructively probes workspace compiler/type errors (`cargo check`, `npx tsc --noEmit`, `ruff check`, `go vet`) in the background right after turn completion.
  - Automatically dispatches `shadow_lint_diagnostic` event over IPC stream.
- **Frontend**:
  - `packages/app/src/stores/shadow-linter-store.ts`: Reactive state management for live background diagnostics per agent.
  - `packages/app/src/components/shadow-linter-pill.tsx`: Ambient track pill in `AgentTracks` displaying error counts and 1-click `[⚡ Auto-Heal]` action.

### 2. FEAT-2: Editor Inline `Ctrl+K` Prompt & In-Place Diff Generator (Cursor / Windsurf Parity)

- **Status**: ✅ Completed & Verified.
- **Frontend**:
  - `packages/app/src/file-panels/source/editor/inline-ctrl-k-prompt.tsx`: Floating micro-composer anchored to CodeMirror selection/cursor coordinates.
  - Supports live keyboard shortcuts: `Ctrl+K` / `Cmd+K` to open, `Enter` to generate, `Tab` to accept diff, `Esc` to reject.
  - `packages/app/src/file-panels/source/editor/view.web.tsx`: Wired with CodeMirror selection listeners and instant in-place buffer modification.

### 3. FEAT-3: 1-Click GitHub Pull Request (PR) Generator with AI Summary (Devin / Cursor Parity)

- **Status**: ✅ Completed & Verified.
- **Backend**: `desktop-tauri/src-tauri/src/agent/github_ops.rs`:
  - Automatically stages uncommitted work, executes `git push -u origin <head>`, and calls `gh pr create` with zero-dependency URL-encoded web comparison fallback.
- **Frontend**:
  - `packages/app/src/desktop/components/github-pr-modal.tsx`: Interactive modal with branch selector, AI summary, key changes, and test plan.
  - `/pr` (aliases: `pull-request`, `create-pr`) client slash command in composer.

### 4. FEAT-4: Global Workspace Search & Replace with Ripgrep Preview (VS Code / Zed Parity)

- **Status**: ✅ Completed & Verified.
- **Backend**: `desktop-tauri/src-tauri/src/agent/ripgrep_search.rs`:
  - High-speed file traversal skipping `node_modules` and target directories. Supports case-sensitivity, whole-word matching, and transactional batch multi-file replace.
- **Frontend**:
  - `packages/app/src/panels/global-search-replace-modal.tsx`: Full-workspace search and replace modal with collapsible file trees, match counters, and 1-click batch replace.
  - Accessible via global shortcut `Ctrl+Shift+F` / `Cmd+Shift+F`.

### 5. FEAT-5: Visual Artifacts Canvas & Mermaid Architecture Renderer (Claude / Antigravity Parity)

- **Status**: ✅ Completed & Verified.
- **Frontend**:
  - `packages/app/src/components/artifacts-canvas-modal.tsx`: Dedicated split-view Artifacts Canvas modal.
  - Supports interactive Mermaid architecture diagrams, rich technical specifications in Markdown, and live interactive HTML sandboxes.
  - `/artifacts` (aliases: `canvas`, `diagrams`) client slash command.

### 6. FEAT-6: 1-Click Cloud Deployment Engine (Bolt.new / Lovable Parity)

- **Status**: ✅ Completed & Verified.
- **Backend**: `desktop-tauri/src-tauri/src/agent/cloud_deploy.rs`:
  - Auto-sniffs framework (Next.js, Vite, SvelteKit, Astro), invokes deployment commands, and extracts production live URLs (`.vercel.app`, `.netlify.app`, `.pages.dev`).
- **Frontend**:
  - `packages/app/src/desktop/components/cloud-deployment-modal.tsx`: Multi-cloud provider selector with live terminal logs, copy button, and direct browser open.
  - `/deploy` (aliases: `publish`, `ship`) client slash command.

### 7. FEAT-7: Codebase Semantic Vector Search & Hybrid RAG Indexer (Cursor Parity)

- **Status**: ✅ Completed & Verified.
- **Backend**: `desktop-tauri/src-tauri/src/agent/semantic_index.rs`:
  - Chunks codebase into overlapping logical blocks (~40 lines). Computes BM25/TF-IDF term frequency saturation and signature boosts for natural language queries.
  - Formats top retrieved chunks into high-density RAG context blocks.
- **Frontend**:
  - `/search-code` (aliases: `semantic`, `rag`) client slash command.

### 8. FEAT-8: Autonomous Browser Testing & Screenshot Verification Loop (Cline / Devin Parity)

- **Status**: ✅ Completed & Verified.
- **Backend**: `desktop-tauri/src-tauri/src/agent/browser_runner.rs`:
  - Navigates target localhost URLs, extracts HTML title and interactive DOM elements (buttons, links, inputs), catches client runtime error signatures (React error boundaries, uncaught type errors), and generates repair prompts.
- **Frontend**:
  - `packages/app/src/desktop/components/browser-verification-modal.tsx`: Interactive verification modal with health status indicators and 1-click auto-fix button.
  - `/browse-test` (aliases: `test-ui`, `verify-ui`) client slash command.

### 9. FEAT-9: Native Web Search & Web Scraping Tool for Any Model (Claude Code / Devin / OpenCode Parity)

- **Status**: ✅ Completed & Verified.
- **Root Cause Resolved**:
  - Free and local models (Beam, OpenCode, Ollama) failed with `Cancelled a request for input this run cannot answer: Web Search`.
  - Implemented pure native Rust search engine (`desktop-tauri/src-tauri/src/agent/web_search.rs`) with DuckDuckGo HTML scraping, Instant Answer API fallback, and clean HTML-to-Markdown scraper.
  - Suppressed the interactive input cancellation notice from polluting turn timeline.
- **Frontend**:
  - `packages/app/src/desktop/components/web-search-modal.tsx`: Dual-tab modal for live web search and webpage scraping.
  - `/web` (aliases: `search-web`, `google`) and `/fetch` (aliases: `scrape`, `read-url`) client slash commands.

### 10. FEAT-10: Inline Predictive Ghost-Text Autocomplete ("Cursor Tab / Copilot++" Parity)

- **Status**: ✅ Completed & Verified.
- **Frontend**:
  - `packages/app/src/file-panels/source/editor/ghost-text-autocomplete.ts`: CodeMirror 6 inline grey ghost-text completion extension.
  - Displays predictive code inline ahead of cursor with `[Tab]` badge.
  - Supports instant `Tab` to accept completion, `Esc` to reject.
  - Integrated into `packages/app/src/file-panels/source/editor/view.web.tsx`.

### 11. FEAT-11: Global Codebase Symbol Repo Map Engine (Aider Parity)

- **Status**: ✅ Completed & Verified.
- **Backend**:
  - `desktop-tauri/src-tauri/src/kernel/repo_map.rs`: AST and regex symbol indexer extracting classes, functions, structs, and type signatures across the workspace.
  - Exposed via Tauri command `get_repo_map(cwd)`.
- **Frontend**:
  - `/repomap` (aliases: `map`, `symbols`) client slash command in composer.

### 12. FEAT-12: Active Tab & Cursor Flow Context Auto-Tracking (Windsurf Cascade Parity)

- **Status**: ✅ Completed & Verified.
- **Frontend**:
  - `packages/app/src/stores/cascade-flow-store.ts`: Reactive store tracking active editor file, cursor line/col, and recent tabs.
  - `packages/app/src/components/cascade-flow-pill.tsx`: Ambient indicator pill in `AgentTracks` displaying `Flow: <filename>:<line>`.
  - `packages/app/src/composer/submit.ts`: Automatically injects active Cascade Flow context into agent prompts without requiring manual `@file` mentions.

### 13. FEAT-13: Live Side-by-Side Webview Preview Panel with Hot-Reload (Bolt.new / Lovable Parity)

- **Status**: ✅ Completed & Verified.
- **Frontend**:
  - Full side-by-side split browser panel with responsive viewport toggle (Desktop / Tablet / Mobile), hot reload, and element inspector.
  - `/preview` (aliases: `live`, `web-preview`, `split-preview`) client slash command.

### 14. FEAT-14: Specialized Agent Execution Modes (Architect / Code / Ask / Test) (Roo Code Parity)

- **Status**: ✅ Completed & Verified.
- **Backend**:
  - `desktop-tauri/src-tauri/src/agent/personas.rs`: Added full Roo Code mode directives for `Architect`, `Code`, `Ask`, and `Test`.
- **Frontend**:
  - `/mode` (aliases: `switch-mode`, `persona`) client slash command in composer.

### 15. FEAT-15: Terminal YOLO & Command Auto-Approval Permission Selector (Claude Code Parity)

- **Status**: ✅ Completed & Verified.
- **Frontend**:
  - `packages/app/src/desktop/components/permissions-matrix-card.tsx`: Full permission control matrix (Strict, Safe Auto, YOLO Autonomous).
  - `/yolo` (aliases: `danger`, `auto-approve`, `permissions`) client slash command in composer.

### 16. FEAT-16: Hands-Free Voice Dictation Mode (Cursor Voice Parity)

- **Status**: ✅ Completed & Verified.
- **Frontend**:
  - `packages/app/src/composer/composer-dictation-button.tsx`: Real-time speech transcription wired to composer input.
  - `/voice` (aliases: `dictate`, `speak`, `mic`) client slash command in composer.

### 17. FEAT-17: Autonomous Multi-Agent Swarm & Subagent Coordinator (DeepMind Antigravity / Devin Parity)

- **Status**: ✅ Completed & Verified.
- **Frontend & Backend**:
  - Full subagent hierarchy management in `AgentTracks` and `useSubagentsForParent`.
  - `/swarm` (aliases: `subagent`, `delegate`, `spawn-agent`) client slash command in composer.

---

## Verification & Test Results

- **Rust Backend**: All unit test suites passed (`web_search`, `repo_map`, `personas`, `self_healing`, `github_ops`, `ripgrep_search`, `cloud_deploy`, `semantic_index`, `browser_runner`).
- **Frontend Workspaces**: 100% typecheck pass across all 12 monorepo workspaces via `npm run typecheck`.
- **Vitest Suites**: 100% pass across all 26 tests in slash commands, ghost-text autocomplete, cascade-flow store, and submission pipelines.
