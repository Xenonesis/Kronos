import path from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Browser, BrowserContext, Page } from "playwright";
import type { BrowserAutomationExecuteResponse } from "@getpaseo/protocol/browser-automation/rpc-schemas";
import {
  DEFAULT_BROWSER_PROFILE_DIR_NAME,
  PlaywrightBrowserHostClient,
  resolveDefaultBrowserUserDataDir,
} from "./playwright-host.js";

type EventListener = (...args: unknown[]) => void;

interface FakePageOptions {
  url?: string;
  title?: string;
}

function createFakePage(options: FakePageOptions = {}): Page {
  let currentUrl = options.url ?? "https://example.com/initial";
  let currentTitle = options.title ?? "Initial Page";
  const listeners = new Map<string, Set<EventListener>>();

  const page = {
    url: () => currentUrl,
    title: async () => currentTitle,
    goto: async (url: string) => {
      currentUrl = url;
      currentTitle = `Title for ${url}`;
    },
    content: async () => `<html><body>${currentTitle}</body></html>`,
    screenshot: async (opts?: { type?: "png" | "jpeg"; quality?: number; fullPage?: boolean }) => {
      const type = opts?.type ?? "png";
      const quality = opts?.quality ?? 80;
      return Buffer.from(`fake-${type}-screenshot-q${quality}`);
    },
    click: async (_selector: string) => {},
    keyboard: {
      type: async (_text: string) => {},
      press: async (_key: string) => {},
    },
    mouse: {
      wheel: async (_deltaX: number, _deltaY: number) => {},
    },
    evaluate: async <T>(fn: string | ((...args: unknown[]) => T)) => {
      if (typeof fn === "string") {
        return `evaluated: ${fn}` as unknown as T;
      }
      return fn();
    },
    reload: async () => {},
    goBack: async () => {},
    goForward: async () => {},
    setViewportSize: async (_size: { width: number; height: number }) => {},
    close: async () => {
      const closeListeners = listeners.get("close");
      if (closeListeners) {
        for (const listener of closeListeners) {
          listener();
        }
      }
    },
    once: (event: string, listener: EventListener) => {
      if (!listeners.has(event)) {
        listeners.set(event, new Set());
      }
      const set = listeners.get(event)!;
      const wrapped: EventListener = (...args: unknown[]) => {
        set.delete(wrapped);
        listener(...args);
      };
      set.add(wrapped);
      return page as unknown as Page;
    },
    on: (event: string, listener: EventListener) => {
      if (!listeners.has(event)) {
        listeners.set(event, new Set());
      }
      listeners.get(event)!.add(listener);
      return page as unknown as Page;
    },
  };

  return page as unknown as Page;
}

function createFakeContext(initialPages: Page[] = []): {
  context: BrowserContext;
  pagesList: Page[];
  closeListeners: Set<EventListener>;
  pageListeners: Set<(page: Page) => void>;
} {
  const pagesList = [...initialPages];
  const closeListeners = new Set<EventListener>();
  const pageListeners = new Set<(page: Page) => void>();

  const context = {
    pages: () => [...pagesList],
    newPage: async () => {
      const page = createFakePage({ url: "about:blank", title: "New Tab" });
      pagesList.push(page);
      for (const listener of pageListeners) {
        listener(page);
      }
      return page;
    },
    browser: () => null,
    close: async () => {
      for (const listener of closeListeners) {
        listener();
      }
    },
    on: (event: string, listener: EventListener) => {
      if (event === "close") {
        closeListeners.add(listener);
      } else if (event === "page") {
        pageListeners.add(listener as (page: Page) => void);
      }
      return context as unknown as BrowserContext;
    },
  };

  return {
    context: context as unknown as BrowserContext,
    pagesList,
    closeListeners,
    pageListeners,
  };
}

const mockLaunchPersistentContext = vi.fn();
const mockLaunch = vi.fn();

vi.mock("playwright", () => ({
  chromium: {
    launchPersistentContext: (dir: string, opts: unknown) => mockLaunchPersistentContext(dir, opts),
    launch: (opts: unknown) => mockLaunch(opts),
  },
}));

describe("PlaywrightBrowserHostClient", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("directory resolution and options", () => {
    it("resolves default user data dir based on Paseo home", () => {
      const resolved = resolveDefaultBrowserUserDataDir("/custom/paseo/home");
      expect(resolved).toBe(path.join("/custom/paseo/home", DEFAULT_BROWSER_PROFILE_DIR_NAME));
    });

    it("initializes with persistent context by default pointing to browser-profile", () => {
      const client = new PlaywrightBrowserHostClient();
      expect(client.isPersistent).toBe(true);
      expect(client.userDataDir).toContain(DEFAULT_BROWSER_PROFILE_DIR_NAME);
    });

    it("respects custom userDataDir in options", () => {
      const customDir = path.join("/var", "paseo-profile");
      const client = new PlaywrightBrowserHostClient({ userDataDir: customDir });
      expect(client.isPersistent).toBe(true);
      expect(client.userDataDir).toBe(customDir);
    });

    it("sets userDataDir to null when persistent is false", () => {
      const client = new PlaywrightBrowserHostClient({ persistent: false });
      expect(client.isPersistent).toBe(false);
      expect(client.userDataDir).toBeNull();
    });

    it("supports constructor with onResponse callback as first argument", () => {
      const onResponse = vi.fn();
      const client = new PlaywrightBrowserHostClient(onResponse, { persistent: false });
      expect(client.isPersistent).toBe(false);
    });
  });

  describe("browser lifecycle and persistent context", () => {
    it("launches persistent context with configured userDataDir and headless options", async () => {
      const fake = createFakeContext([createFakePage({ url: "https://example.com" })]);
      mockLaunchPersistentContext.mockResolvedValueOnce(fake.context);

      const customDir = path.join("/tmp", "test-browser-profile");
      const client = new PlaywrightBrowserHostClient({
        userDataDir: customDir,
        headless: true,
      });

      const frame = await client.captureCurrentFrame();
      expect(mockLaunchPersistentContext).toHaveBeenCalledWith(
        customDir,
        expect.objectContaining({
          headless: true,
          viewport: null,
        }),
      );
      expect(frame.mimeType).toBe("image/jpeg");
      expect(Buffer.from(frame.data, "base64").toString("utf8")).toContain("fake-jpeg-screenshot");

      await client.close();
    });

    it("launches ephemeral browser when persistent is false", async () => {
      const fake = createFakeContext([createFakePage({ url: "https://ephemeral.test" })]);
      const fakeBrowser = {
        newContext: vi.fn().mockResolvedValue(fake.context),
        close: vi.fn().mockResolvedValue(undefined),
      };
      mockLaunch.mockResolvedValueOnce(fakeBrowser as unknown as Browser);

      const client = new PlaywrightBrowserHostClient({
        persistent: false,
        headless: false,
      });

      const frame = await client.captureCurrentFrame();
      expect(mockLaunch).toHaveBeenCalledWith(
        expect.objectContaining({
          headless: false,
        }),
      );
      expect(fakeBrowser.newContext).toHaveBeenCalled();
      expect(frame.url).toBe("https://ephemeral.test");

      await client.close();
      expect(fakeBrowser.close).toHaveBeenCalled();
    });
  });

  describe("captureCurrentFrame()", () => {
    it("captures base64 JPEG screenshot with metadata and dataUrl", async () => {
      const initialPage = createFakePage({
        url: "https://app.paseo.sh/dashboard",
        title: "Paseo Dashboard",
      });
      const fake = createFakeContext([initialPage]);
      mockLaunchPersistentContext.mockResolvedValueOnce(fake.context);

      const client = new PlaywrightBrowserHostClient();
      const frame = await client.captureCurrentFrame({ quality: 90 });

      expect(frame.mimeType).toBe("image/jpeg");
      expect(frame.url).toBe("https://app.paseo.sh/dashboard");
      expect(frame.title).toBe("Paseo Dashboard");
      expect(frame.data).toBe(Buffer.from("fake-jpeg-screenshot-q90").toString("base64"));
      expect(frame.dataUrl).toBe(`data:image/jpeg;base64,${frame.data}`);
      expect(frame.toString()).toBe(frame.data);
      expect(frame.timestamp).toBeGreaterThan(0);
      expect(frame.browserId).toBeTruthy();

      await client.close();
    });

    it("automatically opens a new tab if no pages are open yet", async () => {
      const fake = createFakeContext([]);
      mockLaunchPersistentContext.mockResolvedValueOnce(fake.context);

      const client = new PlaywrightBrowserHostClient();
      const frame = await client.captureCurrentFrame();

      expect(fake.pagesList.length).toBe(1);
      expect(frame.mimeType).toBe("image/jpeg");
      expect(frame.browserId).toBeTruthy();

      await client.close();
    });

    it("clamps quality to 0-100 range", async () => {
      const initialPage = createFakePage({ url: "https://test.com" });
      const fake = createFakeContext([initialPage]);
      mockLaunchPersistentContext.mockResolvedValueOnce(fake.context);

      const client = new PlaywrightBrowserHostClient();
      const frameHigh = await client.captureCurrentFrame({ quality: 150 });
      expect(frameHigh.data).toBe(Buffer.from("fake-jpeg-screenshot-q100").toString("base64"));

      await client.close();
    });
  });

  describe("execute() via sendBrowserAutomationRequest()", () => {
    it("handles new_tab, navigate, and list_tabs commands", async () => {
      const fake = createFakeContext([]);
      mockLaunchPersistentContext.mockResolvedValueOnce(fake.context);

      const responses: BrowserAutomationExecuteResponse[] = [];
      const client = new PlaywrightBrowserHostClient((resp) => {
        responses.push(resp);
      });

      // 1. new_tab
      await client.sendBrowserAutomationRequest({
        requestId: "req-1",
        command: {
          command: "new_tab",
          args: { url: "https://example.com/welcome" },
        },
      });

      expect(responses).toHaveLength(1);
      expect(responses[0]?.payload.ok).toBe(true);
      const newTabResult = responses[0]?.payload.result as {
        browserId: string;
        url: string;
        title: string;
      };
      expect(newTabResult.url).toBe("https://example.com/welcome");
      const tabId = newTabResult.browserId;

      // 2. list_tabs
      await client.sendBrowserAutomationRequest({
        requestId: "req-2",
        command: { command: "list_tabs" },
      });

      expect(responses).toHaveLength(2);
      const listResult = responses[1]?.payload.result as {
        tabs: Array<{ browserId: string; url: string; isActive: boolean }>;
      };
      expect(listResult.tabs).toHaveLength(1);
      expect(listResult.tabs[0]?.browserId).toBe(tabId);
      expect(listResult.tabs[0]?.isActive).toBe(true);

      // 3. navigate
      await client.sendBrowserAutomationRequest({
        requestId: "req-3",
        command: {
          command: "navigate",
          args: { browserId: tabId, url: "https://example.com/next" },
        },
      });

      expect(responses).toHaveLength(3);
      const navResult = responses[2]?.payload.result as { url: string };
      expect(navResult.url).toBe("https://example.com/next");

      // 4. snapshot
      await client.sendBrowserAutomationRequest({
        requestId: "req-4",
        command: { command: "snapshot", args: { browserId: tabId } },
      });

      expect(responses).toHaveLength(4);
      const snapResult = responses[3]?.payload.result as { snapshot: string };
      expect(snapResult.snapshot).toContain("Title for https://example.com/next");

      // 5. screenshot (returns PNG for protocol compatibility)
      await client.sendBrowserAutomationRequest({
        requestId: "req-5",
        command: { command: "screenshot", args: { browserId: tabId, fullPage: false } },
      });

      expect(responses).toHaveLength(5);
      const screenshotResult = responses[4]?.payload.result as {
        mimeType: string;
        data: string;
      };
      expect(screenshotResult.mimeType).toBe("image/png");

      // 6. close_tab
      await client.sendBrowserAutomationRequest({
        requestId: "req-6",
        command: { command: "close_tab", args: { browserId: tabId } },
      });

      expect(responses).toHaveLength(6);
      const closeResult = responses[5]?.payload.result as { closed: boolean };
      expect(closeResult.closed).toBe(true);

      await client.close();
    });

    it("returns error response on failure", async () => {
      const fake = createFakeContext([]);
      mockLaunchPersistentContext.mockResolvedValueOnce(fake.context);

      const responses: BrowserAutomationExecuteResponse[] = [];
      const client = new PlaywrightBrowserHostClient((resp) => {
        responses.push(resp);
      });

      // Calling navigate without existing tabs should return ok: false with browser_unknown_error
      await client.sendBrowserAutomationRequest({
        requestId: "req-fail",
        command: {
          command: "navigate",
          args: { browserId: "non-existent-id", url: "https://error.com" },
        },
      });

      expect(responses).toHaveLength(1);
      expect(responses[0]?.payload.ok).toBe(false);
      expect(responses[0]?.payload.error?.code).toBe("browser_unknown_error");

      await client.close();
    });
  });
});
