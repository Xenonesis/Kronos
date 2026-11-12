# Kronos Missing Competitor Features & Parity Roadmap

This document tracks all features identified from the comprehensive competitor audit (Cursor, Windsurf, Devin, Claude Code, Cline / Roo Code, Bolt.new, Lovable, Zed AI, and DeepMind Antigravity) that have been methodically implemented into Kronos.

---

## Roadmap Overview & Status Matrix

| ID | Feature | Competitor Target | Priority | Status |
|:---|:---|:---|:---:|:---:|
| **FEAT-1** | **Background Shadow Linter & Diagnostics Loop** | Cursor | P0 | ✅ Completed |
| **FEAT-2** | **Editor Inline `Ctrl+K` Prompt & In-Place Diff Generator** | Cursor / Windsurf | P0 | ✅ Completed |
| **FEAT-3** | **1-Click GitHub Pull Request (PR) Generator with AI Summary** | Cursor / Devin / Claude Code | P0 | ✅ Completed |
| **FEAT-4** | **Global Workspace Search & Replace (Ripgrep + Diff Preview)** | VS Code / Zed | P1 | ✅ Completed |
| **FEAT-5** | **Visual Artifacts Canvas & Mermaid Architecture Renderer** | Claude Artifacts / Antigravity | P1 | ✅ Completed |
| **FEAT-6** | **1-Click Cloud Deployment Engine (Vercel / Netlify / Cloudflare)** | Bolt.new / Lovable | P1 | ✅ Completed |
| **FEAT-7** | **Codebase Semantic Vector Search & Hybrid RAG Indexer** | Cursor Codebase Indexing | P2 | ✅ Completed |
| **FEAT-8** | **Autonomous Browser Testing & Screenshot Verification Loop** | Cline / Roo Code / Devin | P2 | ✅ Completed |

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

---

## Verification & Test Results

- **Rust Backend**: All test suites passed (`self_healing`, `github_ops`, `ripgrep_search`, `cloud_deploy`, `semantic_index`, `browser_runner`).
- **Frontend Workspaces**: 100% typecheck pass across all 12 monorepo workspaces via `npm run typecheck`.
- **Vitest Suites**: 100% pass across slash commands, editor prompts, and linter stores.
