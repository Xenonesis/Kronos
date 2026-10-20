# Tauri v2 Desktop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and package `packages/desktop-tauri` using Tauri v2, providing a lightweight desktop shell with <100MB idle RAM.

**Architecture:** Tauri v2 Rust backend manages window state, editor openers, and sidecar Node server daemon; OS WebView2 loads precompiled React web UI from `packages/app/dist`.

**Tech Stack:** Rust 2021, Tauri v2 CLI & Core, Node.js, TypeScript.

**Spec:** `docs/superpowers/specs/2026-09-26-tauri-v2-migration-design.md`

## Global Constraints

- Must not modify or break `packages/desktop` (Electron fallback stays fully functional).
- Memory footprint must remain under 120MB total at idle.
- Retain full compatibility with `@getpaseo/protocol` WebSocket endpoints.

---

### Task 1: Initialize `packages/desktop-tauri` Scaffolding

**Files:**

- Create: `packages/desktop-tauri/package.json`
- Create: `packages/desktop-tauri/src-tauri/Cargo.toml`
- Create: `packages/desktop-tauri/src-tauri/tauri.conf.json`
- Create: `packages/desktop-tauri/src-tauri/src/main.rs`
- Modify: `package.json` (add workspace)

- [ ] **Step 1: Create package.json and add workspace**
- [ ] **Step 2: Create Cargo.toml with Tauri v2 dependencies**
- [ ] **Step 3: Configure tauri.conf.json**
- [ ] **Step 4: Implement minimal main.rs and verify compilation**

---

### Task 2: Implement Rust Sidecar Daemon Manager

**Files:**

- Create: `packages/desktop-tauri/src-tauri/src/daemon.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`

- [ ] **Step 1: Implement process spawning with Command**
- [ ] **Step 2: Add graceful shutdown on Tauri app exit**
- [ ] **Step 3: Wire into App setup builder**

---

### Task 3: Implement Native Editor Opener Commands

**Files:**

- Create: `packages/desktop-tauri/src-tauri/src/editor.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`

- [ ] **Step 1: Write editor launch logic (VS Code, Cursor, Zed)**
- [ ] **Step 2: Register Tauri command handler**

---

### Task 4: Build & Memory Verification

**Files:**

- Build artifacts: `packages/desktop-tauri/src-tauri/target/release/`

- [ ] **Step 1: Run `cargo tauri build`**
- [ ] **Step 2: Launch built exe and measure RAM using `tasklist`**
