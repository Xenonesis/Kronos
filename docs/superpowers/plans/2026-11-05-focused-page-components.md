# Focused Page Components Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This document is a plan, not authorization to change application code or publish a release.

**Goal:** Decompose Kronos application pages into focused components without changing functionality, then remove the imports, definitions, styles and files made unused by those extractions.

**Architecture:** Keep filesystem routes, screen identities, stateful owners and existing feature models intact. Extract cohesive presentation subtrees and already-independent stateful cards into their existing feature directories; migrate every caller directly, without compatibility re-exports or a new controller layer. Land one independently verifiable slice at a time, with Workspace last because it owns retained panes and high-frequency subscriptions.

**Tech Stack:** TypeScript, React, Expo Router, React Native/Web, Unistyles, existing Zustand/form models and data-query adapters; Vitest and Playwright. Windows desktop uses Tauri/WebView2; Electron-only guest behavior remains a separate verification target when available.

**Spec:** The user's request in this conversation: “Refactor pages into smaller focused components without breaking any functionality” and “delete any unused imports or files after the operation is done.” The scope, boundaries and acceptance criteria below make that request executable. Existing contracts remain governed by `docs/coding-standards.md`, `docs/expo-router.md`, `docs/forms.md`, `docs/unistyles.md`, `docs/mobile-panels.md`, `docs/timeline-sync.md` and `docs/qa.md`.

## Global constraints

- This is a behavior-preserving extraction, not a redesign, state-management migration, backend refactor or provider change. Preserve copy, test IDs, accessibility labels, keyboard shortcuts, errors, pending states, network payloads and theme behavior.
- Preserve routes under `packages/app/src/app`, route registrations, matched local parameters, Back behavior, deep links, bootstrap boundaries and compatibility redirects. New non-route modules go outside `src/app`.
- Preserve provider nesting, keys, component lifetime, refs, focus gating, effect dependencies and cleanup order. No appearance-dependent keys or newly defined components inside render functions.
- Keep each query/subscription/mutation with its present owner unless an entire already-independent card moves intact. Do not add row-level subscriptions, duplicate fetching or a giant screen-model hook.
- New modules cannot add `useUnistyles`, raw React Query read hooks or raw React Native `TextInput`; do not expand lint grandfathering. Keep legacy hooks at existing owners for view extractions; use existing query adapters and `StyleSheet.create`/`withUnistyles` where a moved independent card needs them.
- Use named props interfaces and existing domain types. No `any`, error-suppressing casts or exported `ReturnType<typeof fn>` contracts. Direct source imports; no re-export-only barrels.
- Keep platform-resolved `.web`, `.native`, `.electron`, Android and iOS implementations and runtime plugin entrypoints. Static import absence does not prove these unused.
- Do not restart the installed daemon on port 6767 or the developer daemon on 6768. Use the existing isolated browser fixtures and ephemeral ports/state. Never uninstall the application or touch production agent data for this refactor.
- Do not run the full local test suite. Run the explicitly selected files serially; final broad verification belongs in existing CI.
- Existing unrelated edits are not cleanup candidates. Remove only code proven unused in the affected page/module graph. No bulk `knip --fix --allow-remove-files` or repository-wide import autofix.

## Inventory and scope

Measured source sizes are approximate, including a trailing empty line. They prioritize inspection; they are not success thresholds.

| Current file under `packages/app/src`    | Approximate lines | Decision                                                                                                 |
| ---------------------------------------- | ----------------: | -------------------------------------------------------------------------------------------------------- |
| `screens/workspace/workspace-screen.tsx` |             4,020 | Extract header and focused-pane views; retain workspace orchestration.                                   |
| `screens/settings/host-page.tsx`         |             1,820 | Extract cohesive host connectivity, lifecycle and configuration cards.                                   |
| `screens/new-workspace-screen.tsx`       |             1,705 | Extract layout and launch-specific composers; retain draft/submission ownership.                         |
| `screens/settings-screen.tsx`            |             1,645 | Extract settings sections and sidebar content; retain navigation/layout ownership.                       |
| `screens/project-settings-screen.tsx`    |             1,277 | Extract controlled config sections, script editing and feedback; retain revision-keyed draft/save owner. |
| `screens/schedules-screen.tsx`           |               460 | Separate existing screen-body presentation from aggregation/form ownership.                              |
| `screens/sessions-screen.tsx`            |               378 | Extract view-mode tabs and list feedback; preserve history/search/import owner.                          |
| `screens/open-project-screen.tsx`        |               261 | Retain: already a focused landing surface.                                                               |
| `screens/projects-screen.tsx`            |               227 | Retain: focused project list; verify it through project-settings flows.                                  |
| `app/_layout.tsx`                        |               939 | Retain: bootstrap/provider/route ownership is not a page-size cleanup.                                   |

Existing provider, plugin, voice, appearance, composer, explorer, pane, terminal and form modules are reused, not replaced. `screens/new-workspace/form/index.tsx` contains implementation and is not an unused barrel.

### Chosen approach

**Incremental extraction** has the smallest behavioral surface and keeps each change reversible. A one-shot directory rewrite would combine route, import and lifetime failures. Moving everything into `useWorkspaceScreenModel` or `useSettingsScreenModel` would move the complexity without narrowing ownership. Neither alternative is part of this plan.

Paths marked **Create** below are proposed files, not claims that these files already exist. Source line ranges are discovery anchors; locate the named symbols again before editing because lines can shift.

## Task 1: Establish reference, verification and deletion baselines

**Existing files:** `knip.json`, `package.json`, `packages/app/package.json`, `packages/app/playwright.config.ts`, `packages/app/e2e/support/global-setup.ts`, the page owners above and their route importers.

- [ ] Record existing local edits and preserve them. Create an isolated implementation branch/workspace without resetting or overwriting the earlier native reasoning repair.
- [ ] Map imports of each extracted symbol, including tests, direct imports, dynamic registrations and platform siblings. Use LSP references when available; otherwise inspect imports/exports explicitly. Planning-time LSP reported no configured server and no local `.gitnexus` index was found; no graph completeness is claimed.
- [ ] Inspect Knip's resolved entrypoints before using its report for deletion. `knip.json` explicitly lists `app/**/*.{ts,tsx}` for the app, while this checkout's routes live under `src/app`. If automatic plugin discovery has not already included these routes, add `src/app/**/*.{ts,tsx}` to the existing app entry list, preserving all other entries. Do not suppress warnings by making every source file an entrypoint.
- [ ] Save the scoped unused-file/export report as a baseline. A nonzero exit due to existing findings is a baseline to compare, not permission to delete every reported file.

```sh
npm run knip -- --workspace packages/app --include files,exports,types --no-progress --debug
```

- [ ] Before each slice, run that slice's listed existing regressions once, and capture its actual screen at a wide and compact viewport. Save console errors and observable state: input text, focus, scroll, selected host/model/thinking, pending/error views and request count where relevant. Do not replace existing behavior expectations with the refactored implementation's output.
- [ ] Establish typecheck/lint baselines without trying to reconfirm already reported unrelated failures. Resolve genuinely stale workspace declarations through the existing owning build script, not local type casts. Previously reported global lint failures remain outside this refactor.

## Task 2: Separate Schedules rendering from schedule ownership

**Modify:** `packages/app/src/screens/schedules-screen.tsx`.

**Create:** `packages/app/src/screens/schedules/body.tsx`.

**Reuse unchanged:** `packages/app/src/screens/schedules-screen-state.ts`, `packages/app/src/components/schedules/schedules-table.tsx`, `packages/app/src/components/schedules/schedule-form-sheet.tsx` and the existing schedule derivation/model modules.

**Produces:** `SchedulesScreenBody` and `SchedulesScreenBodyProps`; private loading/empty/error views and their owned styles remain in the body module.

- [ ] Move the existing `SchedulesScreenBody`, `SchedulesEmptyState`, `SchedulesEndedEmptyState` and `ScheduleHostErrorsBanner` together. Preserve branch order, filters, retry/create/edit callbacks, table rows, scroll container and every test ID.
- [ ] Replace the current `hosts: ReturnType<typeof useHosts>` annotation with `hosts: HostProfile[]`, importing the existing type from `@/types/host-connection`. Use existing `ScheduleRowView`, `AggregateLoadState`, `AggregatedSchedule`, `ScheduleHostError` and `ScheduleBucket`; do not invent parallel row types.
- [ ] Keep focus-gated mounting, `useSchedules`, aggregated agents/projects, per-host agent-directory readiness, runtime subscription, row derivation and `FormState` in `SchedulesScreenContent`. Keep `ScheduleFormSheet` under its present owner.
- [ ] Move styles with their consumers, retaining only root container styles in the owner. Remove source imports/definitions now unused; do not export an old-name shim.
- [ ] Verify connecting/loading versus loaded-empty, partial host errors, Active/Ended and host filters, create/edit form reseeding, late model hydration, project targets and relative times.

```sh
npm run test --workspace=@getpaseo/app -- --project unit src/screens/schedules-screen-state.test.ts --bail=1 --maxWorkers=1
npm run test:e2e --workspace=@getpaseo/app -- e2e/browser/schedules-edit-model-hydration.spec.ts e2e/browser/schedules-project-target.spec.ts e2e/browser/schedules-relative-time.spec.ts --workers=1 --max-failures=1
```

## Task 3: Extract Sessions tabs and feedback without changing search semantics

**Modify:** `packages/app/src/screens/sessions-screen.tsx`.

**Create:** `packages/app/src/screens/sessions/view-mode-tabs.tsx`, `packages/app/src/screens/sessions/list-feedback.tsx`.

**Produces:** `SessionViewModeTabs`, `SessionViewMode`, `SessionHostErrorsBanner`, `SessionsEmptyState` and `SessionsListFooter`, each with named props where exported.

- [ ] Move the existing module-scope view-mode tabs; preserve the `paseo | omp` union, button behavior and labels.
- [ ] Move host-error/empty presentation and the existing pagination/truncation footer into the feedback module. Preserve the existing compatibility annotation and truncation-before-load-more precedence. Keep the small `resolveEmptyText` derivation with the screen unless moving it is necessary for the same presentation responsibility.
- [ ] Keep focus-gated mounting, `useAgentHistory`, the 200 ms debounce, raw search input, selected-host normalization, refresh/pagination operations, mode selection, `useImportSession` and its sheet lifetime at the screen owner. In particular, switching to OMP must not silently change when the history hook or import sheet mounts.
- [ ] Remove only source imports and moved definitions made obsolete. Do not replace `AgentList` or `OmpSessionsView`, or create one subscription per history row.
- [ ] Verify chronology/date groups, exact/typo highlights, host isolation and host failure reporting, empty history versus no matches, clear/retry/load-more, OMP switching and Import Session.

```sh
npm run test:e2e --workspace=@getpaseo/app -- e2e/browser/sessions-search.spec.ts e2e/browser/sessions-search-hosts.spec.ts e2e/browser/sessions-empty.spec.ts e2e/browser/import-session-flow.spec.ts --workers=1 --max-failures=1
```

## Task 4: Split controlled Project Settings sections

**Modify:** `packages/app/src/screens/project-settings-screen.tsx`.

**Create under `packages/app/src/screens/settings/project/`:**

| File                       | Export/responsibility                                                                                                 |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `worktree-section.tsx`     | `ProjectWorktreeSection`: setup/teardown inputs and existing uncommitted warning/docs links.                          |
| `metadata-section.tsx`     | `ProjectMetadataSection`: metadata descriptors and prompt fields.                                                     |
| `scripts-section.tsx`      | `ProjectScriptsSection`: list/header/empty state and private `ScriptRow`/hint.                                        |
| `script-edit-modal.tsx`    | Existing `ScriptEditModal` and modal-local touched-field state.                                                       |
| `script-validation.ts`     | Existing `validateScript` and named `ScriptValidation`, shared by form and modal without a parent-child import cycle. |
| `save-actions.tsx`         | `ProjectConfigSaveActions`: stale/write-failed callouts and Save/reload/retry controls.                               |
| `read-failure-callout.tsx` | Existing `ProjectConfigReadFailureCallout` and its copy/detail derivation.                                            |

**Consumes:** Existing `ProjectConfigDraft`, `ProjectScriptDraft`, `MetadataPromptKey`, `PaseoConfigRaw`, `PaseoConfigRevision` and typed RPC errors. Sections receive controlled values and callbacks, not query result objects or a duplicate draft.

- [ ] Extract the sections from the existing `ProjectConfigForm` render subtree, then move the existing script row/modal/validation and read-failure view. Preserve `validateScript`'s current translated required-name/command checks; do not introduce new validation rules.
- [ ] Keep `ProjectSettingsScreen` host/project/client selection and `ProjectSettingsBody` query, focus refetch, icon reads, loading/read-error branch order and edit-sheet state in their current file. Preserve query key `["project-config", serverId, repoRoot]` and `retry: false`.
- [ ] Keep config draft, editing-script identity, immutable draft changes, mutation, expected revision, raw-field preservation, cache write, project invalidation and toast in `ProjectConfigForm`.
- [ ] Preserve config-form key `serverId::repoRoot::mtimeMs-size`; preserve project-edit key `${serverId}:${projectId}:${editSessionId}`. Keep script touched-field reset on `script.id`, initial-value input semantics, blur validation and add/cancel/remove behavior.
- [ ] Migrate direct callers, remove obsolete definitions/imports/styles and check parent/child modules do not import each other cyclically.
- [ ] Verify save/retry, external-config stale revision/reload, corrupt/read/transport errors, unknown raw-field preservation, script string-versus-array command preservation, rename/icon editing, Back navigation, and typing/focus during unrelated updates.

```sh
npm run test --workspace=@getpaseo/app -- --project unit src/utils/project-config-form.test.ts src/screens/projects-screen.test.tsx --bail=1 --maxWorkers=1
npm run test:e2e --workspace=@getpaseo/app -- e2e/browser/projects-settings.spec.ts --workers=1 --max-failures=1
```

If existing coverage does not exercise script blur/cancel or active typing during parent updates, add behavioral cases to this existing project-settings journey; do not add source-text or component-wiring tests.

## Task 5: Extract Host Settings by independent operation lifetime

**Modify:** `packages/app/src/screens/settings/host-page.tsx` and its direct importer `packages/app/src/screens/settings-screen.tsx`.

**Create under `packages/app/src/screens/settings/host/`:**

| File                            | Export and state owner                                                                                                                             |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `connections-section.tsx`       | `HostConnectionsSection({ host })`; move `ConnectionsSection`/rows with probes, pending removal and confirmation intact.                           |
| `daemon-restart-card.tsx`       | `RestartDaemonCard({ host })`; own mounted guard, pending/error, confirmation and terminal-survival gate. Reuse existing daemon lifecycle helpers. |
| `daemon-update-card.tsx`        | `UpdateDaemonCard({ host })`; own update state, request-ID filtered progress and unsubscribe lifetime.                                             |
| `remove-host-section.tsx`       | `RemoveHostSection`; retain host/local-daemon/completion props, desktop disable/stop, failure rollback and restart-card placement.                 |
| `agent-orchestration-cards.tsx` | `InjectPaseoToolsCard` and `AppendSystemPromptCard`; retain independent config subscriptions and prompt draft/reset/cancel/save ownership.         |
| `workspace-archival-card.tsx`   | `AutoArchiveMergedWorkspacesCard`; existing connected guard and patch/error behavior.                                                              |
| `terminal-profiles-section.tsx` | `TerminalProfilesSection` and `EnableTerminalAgentHooksCard`; persisted list edits, add/edit identity, row rendering and related helpers/styles.   |

- [ ] Move one independent card group at a time. Preserve current exported host page entry components, missing-host handling, local-versus-remote branches and card ordering.
- [ ] Keep the already-focused `HostStatusBadges`, `HostConnectionError`, host wrappers and pairing row at the current owner. Reuse existing appearance/provider/plugin/voice/recovery/browser-tools cards and `TerminalProfileEditModal`.
- [ ] Preserve update-card host key, progress cleanup and restart's fresh connectivity check. Do not introduce another reconnect implementation.
- [ ] Preserve terminal-profile persistence exactly: whole-list reorder/save uses configured profiles or `DEFAULT_TERMINAL_PROFILES`, not read-time adopted/resolved profiles. Keep unrelated daemon config fields intact.
- [ ] Use compliant theme boundaries in moved cards. Do not copy grandfathered `useUnistyles` into a new file or add lint exceptions.
- [ ] Migrate every direct importer, then remove unused imports/definitions/styles from `host-page.tsx`; do not retain re-export aliases. No pre-existing file has been proven safe to delete during planning.
- [ ] Verify connection removal cancel/pending/failure, offline/auth views, restart with terminal retain/close choices, update subscription completion/unmount, local removal rollback, prompt cancel/reset/rejected save, and terminal profile add/edit/remove/reorder/rejected persistence.

```sh
npm run test --workspace=@getpaseo/app -- --project unit src/screens/settings/daemon-lifecycle.test.ts src/screens/settings/daemon-reconnect.test.ts src/screens/settings/terminal-profile-edit-modal.test.tsx --bail=1 --maxWorkers=1
npm run test:e2e --workspace=@getpaseo/app -- e2e/browser/settings-host-page.spec.ts e2e/browser/daemon-lifecycle.spec.ts --workers=1 --max-failures=1
```

Extend `settings-host-page.spec.ts` only for missing operation failure/lifetime coverage, including profile reorder/save and local removal rollback. Browser fixtures must not restart or remove the installed host. POSIX-only lifecycle tests are not Windows native proof.

## Task 6: Split app Settings sections and sidebar content

**Modify:** `packages/app/src/screens/settings-screen.tsx`.

**Create:** `packages/app/src/screens/settings/general/language-section.tsx`, `packages/app/src/screens/settings/diagnostics-section.tsx`, `packages/app/src/screens/settings/about-section.tsx`, `packages/app/src/screens/settings/settings-sidebar-content.tsx`.

**New narrow contracts:** Reuse the existing `AppLanguage` and React return types from the owner; these replace broad settings/engine objects at the presentation boundary.

```ts
interface LanguageSectionProps {
  language: AppLanguage;
  onLanguageChange: (language: AppLanguage) => void;
}

interface DiagnosticsSectionProps {
  canTestPlayback: boolean;
  isPlaybackTestRunning: boolean;
  playbackTestResult: string | null;
  onPlaybackTest: () => Promise<void>;
}
```

- [ ] Move current language menu/helper/general language presentation into `LanguageSection`. Keep settings persistence/loading in `SettingsScreen` and reuse the existing sending/open-location sections where composed.
- [ ] Move diagnostics presentation using the contract above; keep voice engine, PCM initialization/playback, pending and result state at the owner so switching sections does not reset an in-flight operation.
- [ ] Move cohesive About/version/changelog/updater subtrees and their owned helpers/styles. Keep the updater hook with `DesktopAppUpdateRow`, focus-based silent checking, manual check/install confirmation and release-channel persistence. Host version rows retain their existing narrow selectors.
- [ ] Extract sidebar body/buttons/group/picker UI as `SettingsSidebarContent`. Keep current `SettingsSidebar` lifetime, safe-area/layout logic, host derivation and `useSettingsSidebarScroll(isDesktop)` at the owner. The picker keeps its open state and trigger ref together. Do not add a scroll container or a new sidebar store.
- [ ] Keep `SettingsView`, selected-host resolution, detail dispatch, all route callbacks, add-host visibility/intent handling and desktop-versus-compact layout at `SettingsScreen`. Preserve distinct project index/detail and plugin routes, cold host deep links and independent sidebar/detail scrolling.
- [ ] Remove moved declarations/imports/styles and decorative dead separators from the touched owner. Keep implementation-bearing existing index modules; do not create a new barrel.
- [ ] Verify locale labels/persistence, sidebar scroll under resize, compact list/detail/Back, selected host, add-host method Back/Escape/password flows, diagnostics pending/failure and About/updater feedback.

```sh
npm run test --workspace=@getpaseo/app -- --project unit src/navigation/settings-presentation/model.test.ts --bail=1 --maxWorkers=1
npm run test:e2e --workspace=@getpaseo/app -- e2e/browser/settings-navigation.spec.ts e2e/browser/settings-host-selection.spec.ts e2e/browser/settings-i18n.spec.ts e2e/browser/settings-toggle-tab-regression.spec.ts --workers=1 --max-failures=1
```

Add missing diagnostics/no-engine/pending/failure scenarios to the existing settings journey. Exercise actual native updater controls separately; mock/browser success does not prove install behavior, and installing an update is not required or authorized by this refactor.

## Task 7: Extract New Workspace layout and launch-specific composers

**Modify:** `packages/app/src/screens/new-workspace-screen.tsx`.

**Create:** `packages/app/src/screens/new-workspace/layout.tsx`, `packages/app/src/screens/new-workspace/chat-composer.tsx`, `packages/app/src/screens/new-workspace/terminal-composer.tsx`.

**Produces:** `NewWorkspaceLayout`, `NewWorkspaceChatComposer`, `NewWorkspaceTerminalComposer` and named props interfaces. The layout consumes `isCompact`, `fullscreen`, `title`, `formStack`, import callback and children. Composer contracts use existing draft/editing/composer types and only the values/callbacks currently passed to their respective JSX branches.

- [ ] Move current `NewWorkspaceLayout` and its owned content/title/compact-space styles first. Preserve `MenuHeader`, titlebar drag region, `ComposerDock`, compact import button, desktop import placement and fullscreen omissions. Keep `FileDropZone`, error and import sheet at the screen owner.
- [ ] Move the current chat and terminal composer render branches without moving the draft/editing-session hooks into children. Preserve explicit chat-versus-terminal identity, separate text sources, focus keys, `preserve-and-lock`, blur behavior, attachment scopes, Forge auto-attach and terminal no-attachment/no-draft seams.
- [ ] Pass live `composerState`/`agentControlsWithDisabled` values, not an initial snapshot. Do not hand-build model catalogs or thinking options. A model/provider change must still update available controls and submit the selected values.
- [ ] Keep creation identity, module-level submission promise deduplication, optimistic snapshot handoff, presence guard, retry/captured-draft cleanup, terminal launch/cache update, host/project choice and branch/Forge queries at the owner. Reuse existing `new-workspace/form` and policy helpers; do not introduce another form model.
- [ ] Keep `app/new.tsx` editing-session provider, reset key and `ComposerEditingScope` unchanged. Remove only newly unused owner imports/styles/definitions after migration.
- [ ] Verify launch mode switches preserve separate drafts, provider/model/thinking selection, late catalog recovery, worktree/local preferences, branch/PR input, import, attachment/dictation, delayed completion after navigation, failed submit and retry, and compact/fullscreen resize without draft loss.

```sh
npm run test --workspace=@getpaseo/app -- --project unit src/screens/new-workspace/background-handoff.test.ts src/screens/new-workspace/screen-presence.test.ts src/screens/new-workspace-terminal.test.ts --bail=1 --maxWorkers=1
npm run test:e2e --workspace=@getpaseo/app -- e2e/browser/new-workspace-launch-composer.spec.ts e2e/browser/new-workspace-launch-terminal.spec.ts e2e/browser/new-workspace-composer-draft.spec.ts e2e/browser/new-workspace-navigation-guard.spec.ts e2e/browser/new-workspace-model-recovery.spec.ts e2e/browser/workspace-draft-thinking-preferences.spec.ts --workers=1 --max-failures=1
```

Additional changed paths select their existing focused specs: `new-workspace-preselect.spec.ts`, `new-workspace-host-project-preservation.spec.ts`, `new-workspace-isolation-memory.spec.ts`, `new-workspace-live-pin.spec.ts`, `new-workspace-dictation-submit.spec.ts` and `new-workspace-codex-mode-preferences.spec.ts`. Do not run these merely because their names exist; run the ones whose owned path changed.

## Task 8: Extract Workspace views while retaining the workspace owner

**Modify:** `packages/app/src/screens/workspace/workspace-screen.tsx`.

**Create under `packages/app/src/screens/workspace/`:**

| File                                 | Responsibility and contract                                                                                                                                                          |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `workspace-header-title-bar.tsx`     | Existing `WorkspaceHeaderTitleBar` and named props; title/skeleton/project row/menu. Current interface is near lines 472–486.                                                        |
| `workspace-header-actions.tsx`       | `WorkspaceHeaderActions`; existing `headerRight` subtree. Consume IDs, scripts/live terminal IDs, directory/project/active file, explorer presentation and existing typed callbacks. |
| `workspace-compact-header.tsx`       | `WorkspaceCompactHeader`; existing compact header composition, tabs and menus. Reuse switcher/menu contracts with `Pick` rather than copying callback definitions.                   |
| `workspace-focused-pane-content.tsx` | `WorkspaceFocusedPaneContent`; move existing `MobileMountedTabSlot` and `renderWorkspaceContent` together, including private loading/missing/empty and retained-slot rendering.      |

- [ ] Extract title bar, actions and compact header in that order. Reuse existing `WorkspaceHeaderProjectRow`, menus, scripts/plugin/editor buttons and explorer toggle. Keep directory-null behavior, hidden-label flags, capability gates and header policy unchanged.
- [ ] Move focused-pane rendering without moving `useMountedTabSet`, stable descriptor-map ownership, ordered mounted-tab filtering or modified-tab subscriptions. Props retain the current pane ID, ordered mounted IDs, descriptor map, active descriptor, route focus, focus-suspension/focus callback and typed content factory.
- [ ] Preserve slot keys, `memo`, absolute fill, touch focus capture, `RetainedPanel` predicate and `WorkspacePaneContent` contexts. Do not key the new outer view by the active tab or recreate existing `WorkspacePaneContentModel` policy.
- [ ] Keep provider topology, persistence keys, hydration/pending-open recovery, setup seeding, visible-timeline subscriptions, focused agent/terminal synchronization, file-target routing, keyboard/command dispatch and close/confirmation/browser cleanup at the screen owner. Do not merge distinct terminal/agent/modified-file/subagent close paths into a generic action.
- [ ] Leave `SplitContainer`, explorer dock/frame, sheet hosts and platform geometry siblings intact. A pass-through wrapper around them is not an extraction benefit.
- [ ] Move owned styles and remove confirmed-unused style families from the old owner after rechecking references: `newTabActions`/`newTabActionButton`/tooltip family, old `tabsContainer`/scroll/content/actions family, old tab handle/icon/active/hover/label/close family and `contentPlaceholder`. Check computed `styles[...]`, spreads and enumeration before deletion.
- [ ] Do not remove `_insets`, `_pinnedAgentIds` or `_hiddenAgentIds` merely because their values are unused: their hooks may still subscribe. Changing those subscriptions is a separate behavioral decision, not import cleanup.
- [ ] Verify A→B→A workspace/tab retention, draft/caret/scroll, Settings round trips, cold/offline/reconnect/missing-workspace states, compact gestures/Back, focus mode, split/explorer placement, terminal stream/session retention, unsaved-close cancel and browser tab identity.

```sh
npm run test --workspace=@getpaseo/app -- --project unit src/screens/workspace/workspace-deck-retention.test.ts src/screens/workspace/use-mounted-tab-set.test.ts src/screens/workspace/visible-agent-ids.test.ts src/screens/workspace/workspace-draft-pane-focus.test.ts src/screens/workspace/workspace-pane-content.test.tsx --bail=1 --maxWorkers=1
npm run test:e2e --workspace=@getpaseo/app -- e2e/browser/workspace-pane-remount.spec.ts e2e/browser/workspace-navigation-regression.spec.ts e2e/browser/compact-workspace-header.spec.ts e2e/browser/compact-workspace-sheet.spec.ts e2e/browser/terminal-retained-tab-stream.spec.ts e2e/browser/explorer-pane-placement.spec.ts --workers=1 --max-failures=1
```

Select existing focused close/file/rename/script specs if those render paths change: `file-editing.spec.ts`, `file-change-conflicts.spec.ts`, `workspace-agent-tab-rename.spec.ts`, `workspace-scripts-menu-resize.spec.ts`. Preserve behavior assertions, not tests that search source text for the old file location; replace or remove such assertions rather than re-pinning implementation text.

## Per-slice completion gate: migrate, prove, then delete

Apply this gate to Tasks 2–8 before starting the next slice; do not defer removal until a final mass cleanup.

1. Existing user journeys pass before and after the extraction. Add tests only for a plausible user-visible boundary not already covered; avoid export/import wiring tests, shallow snapshots and mock echoes.
2. Direct callers import the new owner; old definitions and re-exports are gone. Remove imports and private helpers/styles made unused by that move. Use language-server organize-import/code actions when available and review their scope.
3. Confirm no cycles or new barrel-only modules. Keep each new file cohesive; do not split by a line quota. Do not claim runtime performance improvement from fewer lines alone.
4. Inspect any candidate whole-file deletion against the graph, filesystem routes, platform resolution, plugin/runtime registration, side-effect imports, tests and config. No existing whole file is preapproved for deletion by this plan. Delete files made redundant by completed migration only after all those roots are accounted for. Preserve unrelated pre-existing or user-owned files.
5. Run the affected source paths through existing formatting/lint scripts, and run workspace typecheck once after the slice is integrated. The shell executor substitutes the actual enumerated changed paths into the script invocation; never format or autofix unrelated files.
6. Run the selected regressions above serially and exercise the actual rendered surface with before/after screenshots and console inspection. Tests alone do not prove focus, popup placement, retained browser identity or native behavior.
7. Review the slice before advancing. If a failure appears, fix or revert that slice's extraction only; do not weaken assertions, remove features or suppress errors to proceed.

Common integrated commands:

```sh
npm run typecheck
npm run lint
npm run knip -- --workspace packages/app --include files,exports,types --no-progress
npm run build --workspace=@getpaseo/app
```

Use the existing `format:files`, `format:check:files` and targeted `lint` scripts with the actual changed file list. Run the root typecheck/lint above at final integration, not concurrently from every worker. Compare Knip findings with Task 1's baseline; resolve new affected-scope findings, and do not hide unrelated baseline findings with ignores or `--no-exit-code`.

## Task 9: End-to-end acceptance and delivery

- [ ] Use the existing browser fixture on isolated state for wide and compact navigation: Open Project → New Workspace → active conversation → History/Schedules → Settings → original workspace. Keep route names, Back behavior, selected host/tab, text, caret, scroll and pending feedback intact.
- [ ] On Windows Tauri, exercise the changed page controls and resize/hide/reveal paths. Verify typing, keyboard shortcuts, titlebar drag behavior, dialogs and terminal stream continuity. Use an isolated verification daemon; do not restart production. Web browser evidence is not WebView2 or native-mobile evidence.
- [ ] Verify one currently available reasoning model through the native daemon: live catalog → thinking selection → initial prompt and follow-up → reasoning/answer display → saved history → workspace/Settings round trip. Discover the model dynamically. A provider quota/auth/unavailable condition must remain visible and is a verification limitation, not a reason to fake success or change tools/config.
- [ ] When Electron guest-browser code paths are available, verify resident tab switching/new-tab/cleanup through their existing desktop harness. Do not treat ordinary app browser tests as proof of Electron guest lifetime. No guest-host implementation changes are planned.
- [ ] Obtain Android/iOS verification for cold host/project deep links, compact headers/sheets, keyboard/focus, Back/gesture and theme transitions. A compact browser viewport is not native proof. Explicitly report platforms not exercised; do not claim cross-platform equivalence without evidence.
- [ ] Repeat scoped reference/dead-code review after the final integration and remove refactor-created orphans, unused imports/exports and temporary verification scripts. Keep legitimate entrypoints and captured evidence. Record the exact deleted files and why each was unreachable; an empty deletion list is acceptable if the refactor leaves none.
- [ ] Update only existing subject documentation whose ownership rules actually changed; do not append a prose inventory of moved components. Mark this plan's completed checkboxes and attach exact commands/results, before/after screenshots, tested platforms and unresolved baseline failures.
- [ ] If a distributable installer is requested during execution, build it through the supported bundle pipeline with a freshly exported frontend and verify installed pages. This planning request does not authorize packaging, installation, publishing, pushing or committing unrelated changes.

## Definition of done

- All seven targeted page owners use the focused boundaries above; routes and intentionally retained focused screens remain unchanged.
- Every listed user-visible acceptance path is preserved, with actual evidence or an explicit environment limitation; no reduced functionality is accepted to achieve a smaller file.
- Drafts, form revisions, retained pane identity, focus/scroll, subscriptions, query/mutation ownership, native reasoning controls and error/pending behavior remain intact.
- No unused imports, stale moved definitions, compatibility re-exports or refactor-created orphan files remain in the affected graph. No unrelated file was deleted.
- Targeted regressions, typecheck/lint/build and runtime checks are recorded accurately. Existing unrelated failures are distinguished from new regressions.

## Planning evidence and limits

The plan is based on local screen/route/configuration inspection, source-size measurement, two read-only domain investigations and the installed Knip CLI help. No application source was changed, no baseline tests or runtime scenarios were run for this planning task, and no unused file was deleted. Knip's analysis has not been run; its explicit route-pattern discrepancy is a deletion-audit prerequisite, not proof of an orphan or of a broken scanner.
