import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  subscribeVfsWatch,
  unsubscribeVfsWatch,
  watchDirectoryNative,
  unwatchDirectoryNative,
} from "./vfs-watcher-native";

interface DesktopHostWindow {
  window?: {
    paseoDesktop?: {
      invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
  };
}

const env = globalThis as unknown as DesktopHostWindow;

describe("vfs-watcher-native", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    delete env.window;
  });

  describe("subscribeVfsWatch", () => {
    it("invokes watch_directory with path and ignore patterns", async () => {
      const mockInvoke = vi.fn().mockResolvedValueOnce("sub_12345");
      env.window = {
        paseoDesktop: {
          invoke: mockInvoke,
        },
      };

      const result = await subscribeVfsWatch("/workspace/root", ["node_modules/**", "target/**"]);

      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(mockInvoke).toHaveBeenCalledWith("watch_directory", {
        path: "/workspace/root",
        ignore: ["node_modules/**", "target/**"],
      });
      expect(result).toBe("sub_12345");
    });

    it("defaults ignore to null when omitted", async () => {
      const mockInvoke = vi.fn().mockResolvedValueOnce("sub_67890");
      env.window = {
        paseoDesktop: {
          invoke: mockInvoke,
        },
      };

      const result = await subscribeVfsWatch("/workspace/root");

      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(mockInvoke).toHaveBeenCalledWith("watch_directory", {
        path: "/workspace/root",
        ignore: null,
      });
      expect(result).toBe("sub_67890");
    });

    it("returns null when window is undefined", async () => {
      delete env.window;
      const result = await subscribeVfsWatch("/workspace/root");
      expect(result).toBeNull();
    });

    it("returns null when window.paseoDesktop is undefined", async () => {
      env.window = {
        paseoDesktop: undefined,
      };

      const result = await subscribeVfsWatch("/workspace/root");
      expect(result).toBeNull();
    });

    it("returns null when invoke is not a function", async () => {
      env.window = {
        paseoDesktop: {},
      };

      const result = await subscribeVfsWatch("/workspace/root");
      expect(result).toBeNull();
    });

    it("catches invocation failures and returns null", async () => {
      const mockInvoke = vi.fn().mockRejectedValueOnce(new Error("Watcher failed"));
      env.window = {
        paseoDesktop: {
          invoke: mockInvoke,
        },
      };

      const result = await subscribeVfsWatch("/workspace/root");
      expect(result).toBeNull();
    });

    it("returns null when invoke resolves with non-string or empty string", async () => {
      const mockInvoke = vi.fn().mockResolvedValueOnce("");
      env.window = {
        paseoDesktop: {
          invoke: mockInvoke,
        },
      };

      const result = await subscribeVfsWatch("/workspace/root");
      expect(result).toBeNull();
    });
  });

  describe("unsubscribeVfsWatch", () => {
    it("invokes unwatch_directory with subscriptionId and returns true on success", async () => {
      const mockInvoke = vi.fn().mockResolvedValueOnce(true);
      env.window = {
        paseoDesktop: {
          invoke: mockInvoke,
        },
      };

      const result = await unsubscribeVfsWatch("sub_12345");

      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(mockInvoke).toHaveBeenCalledWith("unwatch_directory", {
        subscriptionId: "sub_12345",
        subscription_id: "sub_12345",
      });
      expect(result).toBe(true);
    });

    it("returns false when unwatch_directory returns false", async () => {
      const mockInvoke = vi.fn().mockResolvedValueOnce(false);
      env.window = {
        paseoDesktop: {
          invoke: mockInvoke,
        },
      };

      const result = await unsubscribeVfsWatch("sub_unknown");
      expect(result).toBe(false);
    });

    it("returns false when window is undefined", async () => {
      delete env.window;
      const result = await unsubscribeVfsWatch("sub_12345");
      expect(result).toBe(false);
    });

    it("returns false when window.paseoDesktop or invoke is unavailable", async () => {
      env.window = {
        paseoDesktop: undefined,
      };

      const result = await unsubscribeVfsWatch("sub_12345");
      expect(result).toBe(false);
    });

    it("catches invocation failures and returns false", async () => {
      const mockInvoke = vi.fn().mockRejectedValueOnce(new Error("Unwatch error"));
      env.window = {
        paseoDesktop: {
          invoke: mockInvoke,
        },
      };

      const result = await unsubscribeVfsWatch("sub_12345");
      expect(result).toBe(false);
    });
  });

  describe("alias exports", () => {
    it("exports watchDirectoryNative and unwatchDirectoryNative aliases", () => {
      expect(watchDirectoryNative).toBe(subscribeVfsWatch);
      expect(unwatchDirectoryNative).toBe(unsubscribeVfsWatch);
    });
  });
});
