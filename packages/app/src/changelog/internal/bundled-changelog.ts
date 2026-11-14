export const BUNDLED_CHANGELOG = `# Changelog

## 0.16.0 - 2026-11-14

- **100% Native Rust Kernel Migration:** Embedded daemon inside Tauri v2 architecture, pure-Rust WebSocket multiplexer, lightweight memory footprint (<100MB RAM), sub-second cold boot.
- **Monocode Swarm Parity:** Quick Composer modal (\`Cmd/Ctrl+Shift+K\`), Scratchpad Notes drawer (\`Cmd/Ctrl+Shift+N\`), OMP Session telemetry, AST diff viewer, and full-text search over SQLite WAL.
- **Autonomous CI Repair Subsystem:** Real GitHub API check-run and job log retrieval, AI log parser, patch generator, and automated git worktree commit & push.
- **Zero-Mock Production Hardening:** Eliminated mock fallback layers across Claude, Codex, OMP, Pi, ACP, and Ollama providers; strictly honest live API dispatch and error reporting.
- **System Tray DOM Bridge:** System tray click events (\`open-quick-composer\`, \`open-notes\`, \`new-chat\`) directly bridged to window \`CustomEvent\` listeners.
- **Populate Live Model Snapshots:** Populate native-daemon model snapshots from live Codex, Pi, OMP, OpenCode, and Ollama catalogs plus the configured Claude model instead of shipping a stale built-in model list.
- **Kronos Branding & Visual Identity:** Pure Kronos tactical theme, branding emblem on web and desktop startup splash, zero legacy residue.

### Added

- Added native agent profiles and runtime agent configuration store in native daemon.
- Added live model catalog snapshots for Claude, Codex, Pi, OMP, OpenCode, and Ollama.
- Added automatic clean-up of stale executable binaries prior to packaging new builds.
- Added Graphify knowledge graph intelligence integration with zero-token AST indexing.

### Changed

- Rebranded all system settings, about dialogs, and workspace telemetry to Kronos.
- Optimized release bundle size and hardened Tauri IPC channel multiplexing.

## 0.16.0-beta.2 - 2026-11-11

- **Background tasks:** Kronos now shows the background tasks an agent is running, such as shells and monitors, above the composer, with a Stop button for each. Works with Claude Code, Codex and OpenCode v2.
- **Plugin sidebar views:** A plugin can replace the sidebar's content with its own layout of workspaces, agents, projects and terminals across every connected host. Pick the view per device under Settings → Sidebar; Kronos's sidebar header and footer stay in place.
- **Plugin composer controls:** Plugins can add their own controls to the composer, and read and set its prompt, attachments, provider, model, mode and thinking. They can also put the cursor in any agent's composer, add items to New workspace, and store their own metadata on agents and workspaces.
- **Faster diff loading:** Opening Changes shows the file tree right away while file contents load. Large diffs arrive in batches, so the rest of the app keeps responding while they load.
- **Polish:** The chat stays still when a turn ends, and returning to a chat that has not changed shows it as it is with no refresh. Tool rows show their final title and icon from the first frame.
- **Rebuilt mobile chat timeline:** The chat timeline was rebuilt. Expanding or collapsing a turn summary, todo card or tool-call group keeps the line you tapped in place.
- **Keyboard shortcuts:** Full keyboard shortcuts support across desktop and browser with custom bindings.
- **Fewer reconnects on return:** Returning to the app keeps a working connection, including one in the middle of a large response, and reconnects once when it has to.

## 0.15.0 - 2026-10-28

- **Apex Workstation Foundation:** Native desktop deployment, local workspaces, integrated terminal emulator, and unified multi-provider agent workspace.
- **Multi-Model Orchestration:** Support for Claude, Codex, Ollama, OpenCode, Pi, and ACP providers in a unified workflow.
`;
