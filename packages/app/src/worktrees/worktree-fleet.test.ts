/**
 * @vitest-environment jsdom
 */
import React, { type ReactNode } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  type ArchivePaseoWorktreeParams,
  type CreatePaseoWorktreeParams,
  type PaseoWorktreeRaw,
  type WorktreeClient,
  normalizePath,
  transformWorktrees,
} from "./types";
import { WorktreeFleetDrawer } from "./worktree-fleet-drawer";

// Theme hoist
const { theme } = vi.hoisted(() => ({
  theme: {
    spacing: { 1: 4, 1.5: 6, 2: 8, 2.5: 10, 3: 12, 3.5: 14, 4: 16, 6: 24, 8: 32 },
    borderWidth: { 1: 1 },
    borderRadius: { md: 6, lg: 8, full: 9999 },
    fontSize: { xs: 11, sm: 13, base: 15 },
    fontWeight: { normal: "400", medium: "500", semibold: "600" },
    iconSize: { sm: 14, md: 16, lg: 24 },
    colors: {
      foreground: "#ffffff",
      foregroundMuted: "#a0a0a0",
      surface0: "#050505",
      surface1: "#121212",
      surface2: "#1e1e1e",
      surface3: "#2a2a2a",
      border: "#333333",
      borderAccent: "#555555",
      palette: { red: { 300: "#f87171" } },
    },
  },
}));

vi.mock("react-native-unistyles", () => ({
  StyleSheet: {
    create: (factory: unknown) => (typeof factory === "function" ? factory(theme) : factory),
  },
  useUnistyles: () => ({ theme }),
  withUnistyles:
    (Component: React.ComponentType<Record<string, unknown>>) =>
    ({
      uniProps,
      ...rest
    }: {
      uniProps?: (theme: unknown) => Record<string, unknown>;
    } & Record<string, unknown>) => {
      const themed = uniProps ? uniProps(theme) : {};
      return React.createElement(Component, { ...rest, ...themed });
    },
}));

vi.mock("@/constants/layout", () => ({
  useIsCompactFormFactor: () => false,
}));

vi.mock("lucide-react-native", () => {
  const icon = (name: string) => {
    const Icon = () => React.createElement("span", { "data-icon": name });
    Icon.displayName = name;
    return Icon;
  };
  return {
    Archive: icon("Archive"),
    Bot: icon("Bot"),
    Check: icon("Check"),
    Folder: icon("Folder"),
    GitBranch: icon("GitBranch"),
    Plus: icon("Plus"),
    RotateCw: icon("RotateCw"),
    Trash2: icon("Trash2"),
  };
});

vi.mock("@/components/adaptive-modal-sheet", () => ({
  AdaptiveModalSheet: ({
    visible,
    header,
    children,
    testID,
  }: {
    visible: boolean;
    header?: { title: string };
    children: ReactNode;
    testID?: string;
  }) =>
    visible
      ? React.createElement(
          "section",
          { "data-testid": testID },
          React.createElement("h1", null, header?.title),
          children,
        )
      : null,
}));

vi.mock("@/components/ui/loading-spinner", () => ({
  LoadingSpinner: () => React.createElement("span", { "data-testid": "loading-spinner" }),
}));

vi.mock("@/components/ui/button", () => ({
  Button: ({
    children,
    onPress,
    disabled,
    loading,
    testID,
  }: {
    children: ReactNode;
    onPress?: () => void;
    disabled?: boolean;
    loading?: boolean;
    testID?: string;
  }) =>
    React.createElement(
      "button",
      {
        type: "button",
        "data-testid": testID,
        disabled: disabled || loading,
        onClick: onPress,
      },
      loading ? "Loading..." : children,
    ),
}));

vi.mock("@/components/ui/status-badge", () => ({
  StatusBadge: ({ label, variant }: { label: string; variant?: string }) =>
    React.createElement("span", { "data-testid": `status-badge-${variant ?? "default"}` }, label),
}));

vi.mock("@/components/ui/form-field", () => ({
  Field: ({ label, children, testID }: { label: string; children: ReactNode; testID?: string }) =>
    React.createElement(
      "div",
      { "data-testid": testID },
      React.createElement("label", null, label),
      children,
    ),
  FormTextInput: ({
    initialValue,
    onChangeText,
    placeholder,
    testID,
  }: {
    initialValue?: string;
    onChangeText?: (text: string) => void;
    placeholder?: string;
    testID?: string;
  }) =>
    React.createElement("input", {
      "data-testid": testID,
      defaultValue: initialValue,
      placeholder,
      onChange: (e: React.ChangeEvent<HTMLInputElement>) => onChangeText?.(e.target.value),
    }),
}));

const mockToastShow = vi.fn();
vi.mock("@/contexts/toast-context", () => ({
  useToast: () => ({
    show: mockToastShow,
    dismiss: vi.fn(),
  }),
}));

const mockConfirmDialog = vi.fn().mockResolvedValue(true);
vi.mock("@/utils/confirm-dialog", () => ({
  confirmDialog: (...args: unknown[]) => mockConfirmDialog(...args),
}));

const mockNavigateToWorkspace = vi.fn();
vi.mock("@/stores/navigation-active-workspace-store", () => ({
  navigateToWorkspace: (...args: unknown[]) => mockNavigateToWorkspace(...args),
}));

vi.mock("@/runtime/host-runtime", () => ({
  useHostRuntimeClient: () => null,
}));

vi.mock("@/stores/session-store", () => ({
  useSessionStore: vi.fn((selector) =>
    selector({
      sessions: {
        "test-server": {
          workspaces: new Map([
            [
              "ws-1",
              {
                id: "ws-1",
                workspaceDirectory: "/repos/main/feat-1",
                status: "running",
              },
            ],
            [
              "ws-2",
              {
                id: "ws-2",
                workspaceDirectory: "/repos/main/feat-2",
                status: "idle",
              },
            ],
          ]),
          agents: new Map([
            [
              "agent-1",
              {
                id: "agent-1",
                cwd: "/repos/main/feat-1",
                workspaceId: "ws-1",
                status: "running",
                turn: "working",
                title: "Code Assistant",
                provider: "anthropic",
              },
            ],
            [
              "agent-2",
              {
                id: "agent-2",
                cwd: "/repos/main/feat-2",
                workspaceId: "ws-2",
                status: "idle",
                turn: "idle",
                title: "Doc Reviewer",
                provider: "openai",
              },
            ],
          ]),
        },
      },
    }),
  ),
}));

describe("Worktree Fleet Manager", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  describe("Data Transformation (transformWorktrees)", () => {
    it("normalizes paths across Windows backslashes and case differences", () => {
      expect(normalizePath("C:\\Users\\User\\Repo\\Worktree\\")).toBe(
        "c:/users/user/repo/worktree",
      );
      expect(normalizePath("/var/repo/wt/")).toBe("/var/repo/wt");
      expect(normalizePath("")).toBe("");
      expect(normalizePath(null)).toBe("");
    });

    it("correctly identifies the current worktree", () => {
      const raw: PaseoWorktreeRaw[] = [
        {
          worktreePath: "/repos/paseo",
          createdAt: "2026-09-01T00:00:00Z",
          branchName: "main",
        },
        {
          worktreePath: "/repos/paseo-feature",
          createdAt: "2026-09-02T00:00:00Z",
          branchName: "feature-branch",
        },
      ];

      const items = transformWorktrees({
        rawWorktrees: raw,
        currentCwd: "/repos/paseo",
      });

      expect(items).toHaveLength(2);
      expect(items[0]?.isCurrent).toBe(true);
      expect(items[0]?.status).toBe("active");
      expect(items[1]?.isCurrent).toBe(false);
      expect(items[1]?.status).toBe("ready");
    });

    it("resolves branch names with fallbacks to short head or detached", () => {
      const raw: PaseoWorktreeRaw[] = [
        {
          worktreePath: "/repos/wt-1",
          createdAt: "2026-09-01T00:00:00Z",
          branchName: "feature/auth",
        },
        {
          worktreePath: "/repos/wt-2",
          createdAt: "2026-09-01T00:00:00Z",
          head: "abcdef1234567890",
        },
        {
          worktreePath: "/repos/wt-3",
          createdAt: "2026-09-01T00:00:00Z",
          branchName: null,
          head: null,
        },
      ];

      const items = transformWorktrees({
        rawWorktrees: raw,
        currentCwd: "/other",
      });

      expect(items[0]?.branchName).toBe("feature/auth");
      expect(items[1]?.branchName).toBe("abcdef1");
      expect(items[2]?.branchName).toBe("detached");
    });

    it("correlates agents by workspaceId and cwd, setting running status", () => {
      const raw: PaseoWorktreeRaw[] = [
        {
          worktreePath: "/repos/main/feat-1",
          createdAt: "2026-09-01T00:00:00Z",
          branchName: "feat-1",
        },
      ];

      const items = transformWorktrees({
        rawWorktrees: raw,
        currentCwd: "/repos/main",
        workspaces: [
          {
            id: "ws-1",
            workspaceDirectory: "/repos/main/feat-1",
          },
        ],
        agents: [
          {
            id: "agent-1",
            cwd: "/repos/main/feat-1",
            workspaceId: "ws-1",
            status: "running",
            title: "Builder",
            provider: "anthropic",
          },
        ],
      });

      expect(items).toHaveLength(1);
      expect(items[0]?.workspaceId).toBe("ws-1");
      expect(items[0]?.status).toBe("running");
      expect(items[0]?.agents).toHaveLength(1);
      expect(items[0]?.agents[0]?.title).toBe("Builder");
      expect(items[0]?.agents[0]?.status).toBe("running");
    });

    it("sets idle status when agents are present but not running", () => {
      const raw: PaseoWorktreeRaw[] = [
        {
          worktreePath: "/repos/main/feat-idle",
          createdAt: "2026-09-01T00:00:00Z",
          branchName: "feat-idle",
        },
      ];

      const items = transformWorktrees({
        rawWorktrees: raw,
        currentCwd: "/repos/other",
        agents: [
          {
            id: "agent-idle",
            cwd: "/repos/main/feat-idle",
            status: "idle",
            turn: "idle",
          },
        ],
      });

      expect(items[0]?.status).toBe("idle");
    });
  });

  describe("Client Flows (Listing, Creation, Archiving)", () => {
    it("lists active worktrees using client.listPaseoWorktrees", async () => {
      const mockList = vi.fn().mockResolvedValue({
        worktrees: [
          {
            worktreePath: "/repos/wt-main",
            createdAt: "2026-09-01T00:00:00Z",
            branchName: "main",
          },
        ],
      });

      const client: WorktreeClient = {
        listPaseoWorktrees: mockList,
        createPaseoWorktree: vi.fn(),
        archivePaseoWorktree: vi.fn(),
      };

      const result = await client.listPaseoWorktrees!({ cwd: "/repos/wt-main" });
      expect(mockList).toHaveBeenCalledWith({ cwd: "/repos/wt-main" });
      expect(result.worktrees).toHaveLength(1);
      expect(result.worktrees[0]?.branchName).toBe("main");
    });

    it("falls back to client.getPaseoWorktreeList when listPaseoWorktrees is undefined", async () => {
      const mockGetList = vi.fn().mockResolvedValue({
        worktrees: [
          {
            worktreePath: "/repos/wt-compat",
            createdAt: "2026-09-01T00:00:00Z",
            branchName: "compat-branch",
          },
        ],
      });

      const client: WorktreeClient = {
        getPaseoWorktreeList: mockGetList,
        createPaseoWorktree: vi.fn(),
        archivePaseoWorktree: vi.fn(),
      };

      const listFn = client.listPaseoWorktrees ?? client.getPaseoWorktreeList!;
      const result = await listFn({ cwd: "/repos/wt-compat" });
      expect(mockGetList).toHaveBeenCalledWith({ cwd: "/repos/wt-compat" });
      expect(result.worktrees).toHaveLength(1);
    });

    it("creates a worktree using client.createPaseoWorktree", async () => {
      const mockCreate = vi.fn().mockResolvedValue({
        workspace: { id: "ws-created" },
        error: null,
      });

      const client: WorktreeClient = {
        listPaseoWorktrees: vi.fn(),
        createPaseoWorktree: mockCreate,
        archivePaseoWorktree: vi.fn(),
      };

      const params: CreatePaseoWorktreeParams = {
        cwd: "/repos/wt-main",
        worktreeSlug: "feature-review-42",
        refName: "main",
      };

      const result = await client.createPaseoWorktree(params);
      expect(mockCreate).toHaveBeenCalledWith(params);
      expect(result.error).toBeNull();
      expect(result.workspace).toEqual({ id: "ws-created" });
    });

    it("archives a worktree using client.archivePaseoWorktree", async () => {
      const mockArchive = vi.fn().mockResolvedValue({
        success: true,
        removedAgents: ["agent-1"],
      });

      const client: WorktreeClient = {
        listPaseoWorktrees: vi.fn(),
        createPaseoWorktree: vi.fn(),
        archivePaseoWorktree: mockArchive,
      };

      const params: ArchivePaseoWorktreeParams = {
        worktreePath: "/repos/main/feat-1",
        repoRoot: "/repos/main",
        branchName: "feat-1",
        workspaceId: "ws-1",
      };

      const result = await client.archivePaseoWorktree(params);
      expect(mockArchive).toHaveBeenCalledWith(params);
      expect(result.success).toBe(true);
      expect(result.removedAgents).toEqual(["agent-1"]);
    });
  });

  describe("WorktreeFleetDrawer UI & Interaction", () => {
    it("renders active worktrees and displays branch, path, status, and agent indicators", async () => {
      const fakeClient: WorktreeClient = {
        listPaseoWorktrees: vi.fn().mockResolvedValue({
          worktrees: [
            {
              worktreePath: "/repos/main/feat-1",
              createdAt: "2026-09-01T00:00:00Z",
              branchName: "feat-1",
            },
          ],
        }),
        createPaseoWorktree: vi.fn(),
        archivePaseoWorktree: vi.fn(),
      };

      render(
        React.createElement(WorktreeFleetDrawer, {
          visible: true,
          onClose: vi.fn(),
          cwd: "/repos/main/feat-1",
          serverId: "test-server",
          client: fakeClient,
        }),
      );

      await waitFor(() => {
        expect(screen.getByTestId("worktree-branch-feat-1")).toBeDefined();
      });

      expect(screen.getByText("feat-1")).toBeDefined();
      expect(screen.getByTestId("worktree-path-/repos/main/feat-1")).toBeDefined();
      expect(screen.getByTestId("worktree-agents-/repos/main/feat-1")).toBeDefined();
      expect(screen.getByText("Code Assistant")).toBeDefined();
    });

    it("opens New Worktree form and invokes client.createPaseoWorktree on submit", async () => {
      const mockCreate = vi.fn().mockResolvedValue({ error: null });
      const mockList = vi.fn().mockResolvedValue({ worktrees: [] });

      const fakeClient: WorktreeClient = {
        listPaseoWorktrees: mockList,
        createPaseoWorktree: mockCreate,
        archivePaseoWorktree: vi.fn(),
      };

      render(
        React.createElement(WorktreeFleetDrawer, {
          visible: true,
          onClose: vi.fn(),
          cwd: "/repos/main",
          serverId: "test-server",
          client: fakeClient,
        }),
      );

      // Click "New Worktree" button
      const newButton = screen.getByTestId("new-worktree-button");
      fireEvent.click(newButton);

      // Form should be rendered
      expect(screen.getByTestId("new-worktree-form")).toBeDefined();

      // Fill in slug and ref
      const slugInput = screen.getByTestId("worktree-slug-input");
      const refInput = screen.getByTestId("worktree-ref-input");

      fireEvent.change(slugInput, { target: { value: "feature-xyz" } });
      fireEvent.change(refInput, { target: { value: "main" } });

      // Click Create Worktree submit button
      const submitButton = screen.getByTestId("create-worktree-submit-button");
      fireEvent.click(submitButton);

      await waitFor(() => {
        expect(mockCreate).toHaveBeenCalledWith({
          cwd: "/repos/main",
          worktreeSlug: "feature-xyz",
          refName: "main",
        });
      });

      expect(mockToastShow).toHaveBeenCalled();
    });

    it("invokes client.archivePaseoWorktree upon confirming archive", async () => {
      const mockArchive = vi.fn().mockResolvedValue({ success: true });
      const fakeClient: WorktreeClient = {
        listPaseoWorktrees: vi.fn().mockResolvedValue({
          worktrees: [
            {
              worktreePath: "/repos/main/feat-to-delete",
              createdAt: "2026-09-01T00:00:00Z",
              branchName: "feat-to-delete",
            },
          ],
        }),
        createPaseoWorktree: vi.fn(),
        archivePaseoWorktree: mockArchive,
      };

      render(
        React.createElement(WorktreeFleetDrawer, {
          visible: true,
          onClose: vi.fn(),
          cwd: "/repos/main",
          serverId: "test-server",
          client: fakeClient,
        }),
      );

      await waitFor(() => {
        expect(
          screen.getByTestId("archive-worktree-button-/repos/main/feat-to-delete"),
        ).toBeDefined();
      });

      const archiveButton = screen.getByTestId(
        "archive-worktree-button-/repos/main/feat-to-delete",
      );
      fireEvent.click(archiveButton);

      await waitFor(() => {
        expect(mockConfirmDialog).toHaveBeenCalled();
        expect(mockArchive).toHaveBeenCalledWith({
          worktreePath: "/repos/main/feat-to-delete",
          repoRoot: "/repos/main",
          branchName: "feat-to-delete",
          workspaceId: undefined,
        });
      });

      expect(mockToastShow).toHaveBeenCalled();
    });

    it("switches to worktree via onSwitchWorktree callback and closes drawer", async () => {
      const onSwitchWorktree = vi.fn();
      const onClose = vi.fn();

      const fakeClient: WorktreeClient = {
        listPaseoWorktrees: vi.fn().mockResolvedValue({
          worktrees: [
            {
              worktreePath: "/repos/main/feat-switch",
              createdAt: "2026-09-01T00:00:00Z",
              branchName: "feat-switch",
            },
          ],
        }),
        createPaseoWorktree: vi.fn(),
        archivePaseoWorktree: vi.fn(),
      };

      render(
        React.createElement(WorktreeFleetDrawer, {
          visible: true,
          onClose,
          cwd: "/repos/main/current",
          serverId: "test-server",
          client: fakeClient,
          onSwitchWorktree,
        }),
      );

      await waitFor(() => {
        expect(screen.getByTestId("switch-worktree-button-/repos/main/feat-switch")).toBeDefined();
      });

      const switchButton = screen.getByTestId("switch-worktree-button-/repos/main/feat-switch");
      fireEvent.click(switchButton);

      expect(onSwitchWorktree).toHaveBeenCalledWith(
        expect.objectContaining({
          worktreePath: "/repos/main/feat-switch",
          branchName: "feat-switch",
        }),
      );
      expect(onClose).toHaveBeenCalled();
    });
  });
});
