import { describe, expect, it, vi } from "vitest";
import { openProjectDirectly } from "./open-project";

describe("openProjectDirectly", () => {
  it("returns explicit error message when host is disconnected", async () => {
    const result = await openProjectDirectly({
      serverId: "local",
      projectPath: "C:/projects/demo",
      isConnected: false,
      canAddProject: true,
      client: null,
      upsertProject: vi.fn(),
      setHasHydratedWorkspaces: vi.fn(),
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("Connecting to host... Please wait a moment and try again.");
    }
  });

  it("returns explicit error message when canAddProject is false", async () => {
    const mockClient = {
      addProject: vi.fn(),
    };

    const result = await openProjectDirectly({
      serverId: "local",
      projectPath: "C:/projects/demo",
      isConnected: true,
      canAddProject: false,
      client: mockClient,
      upsertProject: vi.fn(),
      setHasHydratedWorkspaces: vi.fn(),
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("Update the host to add projects without creating a workspace.");
    }
  });

  it("propagates daemon error string directly on add failure", async () => {
    const mockClient = {
      addProject: vi.fn().mockResolvedValue({
        error: "Permission denied reading directory C:/root",
        errorCode: "permission_denied" as const,
      }),
    };

    const result = await openProjectDirectly({
      serverId: "local",
      projectPath: "C:/root",
      isConnected: true,
      canAddProject: true,
      client: mockClient,
      upsertProject: vi.fn(),
      setHasHydratedWorkspaces: vi.fn(),
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("Permission denied reading directory C:/root");
      expect(result.errorCode).toBe("permission_denied");
    }
  });

  it("returns explicit error message when project path is empty", async () => {
    const result = await openProjectDirectly({
      serverId: "local",
      projectPath: "   ",
      isConnected: true,
      canAddProject: true,
      client: { addProject: vi.fn() },
      upsertProject: vi.fn(),
      setHasHydratedWorkspaces: vi.fn(),
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("Project path cannot be empty.");
    }
  });
});
