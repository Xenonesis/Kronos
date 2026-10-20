import { beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import type { BrowserContext, CDPSession, Page } from "playwright";
import { PlaywrightBrowserHostClient } from "./playwright-host.js";
import type { ScreencastFrame } from "./types.js";

type EventListener = (...args: unknown[]) => void;

interface FakeCDPSession {
  send: Mock;
  on: Mock;
  off: Mock;
  detach: Mock;
  emit: (event: string, payload: unknown) => void;
}

function createFakeCDPSession(): FakeCDPSession {
  const listeners = new Map<string, Set<EventListener>>();
  const send = vi.fn().mockResolvedValue({});
  const detach = vi.fn().mockResolvedValue(undefined);
  const on = vi.fn((event: string, listener: EventListener) => {
    let set = listeners.get(event);
    if (!set) {
      set = new Set();
      listeners.set(event, set);
    }
    set.add(listener);
    return fakeSession;
  });
  const off = vi.fn((event: string, listener: EventListener) => {
    listeners.get(event)?.delete(listener);
    return fakeSession;
  });

  const fakeSession: FakeCDPSession = {
    send,
    on,
    off,
    detach,
    emit: (event: string, payload: unknown) => {
      const set = listeners.get(event);
      if (set) {
        for (const listener of set) {
          listener(payload);
        }
      }
    },
  };

  return fakeSession;
}

interface FakePageOptions {
  url?: string;
  title?: string;
}

interface FakePageInstance {
  page: Page;
  closePage: () => void;
}

function createFakePage(options: FakePageOptions = {}): FakePageInstance {
  let currentUrl = options.url ?? "https://example.com/live";
  const listeners = new Map<string, Set<EventListener>>();

  const rawPage = {
    url: () => currentUrl,
    title: async () => options.title ?? "Test Page",
    goto: async (url: string) => {
      currentUrl = url;
    },
    close: async () => {
      const closeListeners = listeners.get("close");
      if (closeListeners) {
        for (const listener of closeListeners) {
          listener();
        }
      }
    },
    once: (event: string, listener: EventListener) => {
      let set = listeners.get(event);
      if (!set) {
        set = new Set();
        listeners.set(event, set);
      }
      const wrapped: EventListener = (...args: unknown[]) => {
        set?.delete(wrapped);
        listener(...args);
      };
      set.add(wrapped);
      return rawPage as unknown as Page;
    },
    on: (event: string, listener: EventListener) => {
      let set = listeners.get(event);
      if (!set) {
        set = new Set();
        listeners.set(event, set);
      }
      set.add(listener);
      return rawPage as unknown as Page;
    },
  };

  return {
    page: rawPage as unknown as Page,
    closePage: () => {
      const closeListeners = listeners.get("close");
      if (closeListeners) {
        for (const listener of closeListeners) {
          listener();
        }
      }
    },
  };
}

function createFakeContext(
  initialPages: Page[] = [],
  cdpSessionFactory?: (page: Page) => CDPSession,
): {
  context: BrowserContext;
  pagesList: Page[];
  closeListeners: Set<EventListener>;
} {
  const pagesList = [...initialPages];
  const closeListeners = new Set<EventListener>();

  const rawContext = {
    browser: () => ({
      close: async () => {},
    }),
    pages: () => [...pagesList],
    newPage: async () => {
      const fake = createFakePage();
      pagesList.push(fake.page);
      return fake.page;
    },
    newCDPSession: async (page: Page) => {
      if (cdpSessionFactory) {
        return cdpSessionFactory(page);
      }
      return createFakeCDPSession() as unknown as CDPSession;
    },
    close: async () => {
      for (const listener of closeListeners) {
        listener();
      }
    },
    on: (event: string, listener: EventListener) => {
      if (event === "close") {
        closeListeners.add(listener);
      }
      return rawContext as unknown as BrowserContext;
    },
  };

  return {
    context: rawContext as unknown as BrowserContext,
    pagesList,
    closeListeners,
  };
}

const mockLaunchPersistentContext = vi.fn();
const mockLaunch = vi.fn();

vi.mock("playwright", () => ({
  chromium: {
    launchPersistentContext: (...args: unknown[]) => mockLaunchPersistentContext(...args),
    launch: (...args: unknown[]) => mockLaunch(...args),
  },
}));

describe("PlaywrightBrowserHostClient Screencast", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("enables screencast via CDP on active page with default jpeg format and quality 60", async () => {
    const fakePage = createFakePage({ url: "https://example.com/app" });
    const fakeCDP = createFakeCDPSession();
    const fakeContext = createFakeContext([fakePage.page], () => fakeCDP as unknown as CDPSession);

    mockLaunchPersistentContext.mockResolvedValue(fakeContext.context);

    const client = new PlaywrightBrowserHostClient();
    await client.startScreencast();

    expect(client.isScreencastActive).toBe(true);
    expect(fakeCDP.send).toHaveBeenCalledWith("Page.startScreencast", {
      format: "jpeg",
      quality: 60,
    });
  });

  it("supports custom screencast quality, format, and dimension options", async () => {
    const fakePage = createFakePage({ url: "https://example.com/custom" });
    const fakeCDP = createFakeCDPSession();
    const fakeContext = createFakeContext([fakePage.page], () => fakeCDP as unknown as CDPSession);

    mockLaunchPersistentContext.mockResolvedValue(fakeContext.context);

    const client = new PlaywrightBrowserHostClient();
    await client.startScreencast({
      format: "png",
      quality: 85,
      maxWidth: 1920,
      maxHeight: 1080,
    });

    expect(fakeCDP.send).toHaveBeenCalledWith("Page.startScreencast", {
      format: "png",
      quality: 85,
      maxWidth: 1920,
      maxHeight: 1080,
    });
  });

  it("emits frames to onScreencastFrame callback and sends Page.screencastFrameAck", async () => {
    const fakePage = createFakePage({ url: "https://example.com/stream" });
    const fakeCDP = createFakeCDPSession();
    const fakeContext = createFakeContext([fakePage.page], () => fakeCDP as unknown as CDPSession);

    mockLaunchPersistentContext.mockResolvedValue(fakeContext.context);

    const receivedFrames: ScreencastFrame[] = [];
    const client = new PlaywrightBrowserHostClient({
      onScreencastFrame: (frame) => {
        receivedFrames.push(frame);
      },
    });

    await client.startScreencast();

    // Simulate incoming frame from CDP
    fakeCDP.emit("Page.screencastFrame", {
      data: "base64-frame-data-1",
      metadata: {
        timestamp: 1712000000.123,
      },
      sessionId: 101,
    });

    expect(receivedFrames.length).toBe(1);
    expect(receivedFrames[0]).toEqual({
      data: "base64-frame-data-1",
      metadata: {
        timestamp: 1712000000123,
        url: "https://example.com/stream",
      },
    });

    expect(fakeCDP.send).toHaveBeenCalledWith("Page.screencastFrameAck", {
      sessionId: 101,
    });
  });

  it("supports subscriber pattern with subscribeScreencast and unsubscribe", async () => {
    const fakePage = createFakePage({ url: "https://example.com/sub" });
    const fakeCDP = createFakeCDPSession();
    const fakeContext = createFakeContext([fakePage.page], () => fakeCDP as unknown as CDPSession);

    mockLaunchPersistentContext.mockResolvedValue(fakeContext.context);

    const client = new PlaywrightBrowserHostClient();
    const framesA: ScreencastFrame[] = [];
    const framesB: ScreencastFrame[] = [];

    const unsubscribeA = client.subscribeScreencast((frame) => {
      framesA.push(frame);
    });
    client.subscribeScreencast((frame) => {
      framesB.push(frame);
    });

    await client.startScreencast();

    fakeCDP.emit("Page.screencastFrame", {
      data: "frame-1",
      sessionId: 1,
    });

    expect(framesA.length).toBe(1);
    expect(framesB.length).toBe(1);

    // Unsubscribe A
    unsubscribeA();

    fakeCDP.emit("Page.screencastFrame", {
      data: "frame-2",
      sessionId: 2,
    });

    expect(framesA.length).toBe(1);
    expect(framesB.length).toBe(2);
  });

  it("cleans up CDP session and stops screencast on stopScreencast()", async () => {
    const fakePage = createFakePage({ url: "https://example.com/stop" });
    const fakeCDP = createFakeCDPSession();
    const fakeContext = createFakeContext([fakePage.page], () => fakeCDP as unknown as CDPSession);

    mockLaunchPersistentContext.mockResolvedValue(fakeContext.context);

    const client = new PlaywrightBrowserHostClient();
    await client.startScreencast();
    expect(client.isScreencastActive).toBe(true);

    await client.stopScreencast();
    expect(client.isScreencastActive).toBe(false);
    expect(fakeCDP.send).toHaveBeenCalledWith("Page.stopScreencast");
    expect(fakeCDP.detach).toHaveBeenCalled();
  });

  it("cleans up screencast session on page close", async () => {
    const fakePage = createFakePage({ url: "https://example.com/closing" });
    const fakeCDP = createFakeCDPSession();
    const fakeContext = createFakeContext([fakePage.page], () => fakeCDP as unknown as CDPSession);

    mockLaunchPersistentContext.mockResolvedValue(fakeContext.context);

    const client = new PlaywrightBrowserHostClient();
    await client.startScreencast();

    fakePage.closePage();

    await vi.waitFor(() => {
      expect(fakeCDP.send).toHaveBeenCalledWith("Page.stopScreencast");
      expect(fakeCDP.detach).toHaveBeenCalled();
    });
  });

  it("stops screencast on client.close()", async () => {
    const fakePage = createFakePage({ url: "https://example.com/close-client" });
    const fakeCDP = createFakeCDPSession();
    const fakeContext = createFakeContext([fakePage.page], () => fakeCDP as unknown as CDPSession);

    mockLaunchPersistentContext.mockResolvedValue(fakeContext.context);

    const client = new PlaywrightBrowserHostClient();
    await client.startScreencast();

    await client.close();

    expect(fakeCDP.send).toHaveBeenCalledWith("Page.stopScreencast");
    expect(fakeCDP.detach).toHaveBeenCalled();
  });

  it("tolerates subscriber errors without interrupting frame ACK", async () => {
    const fakePage = createFakePage({ url: "https://example.com/err" });
    const fakeCDP = createFakeCDPSession();
    const fakeContext = createFakeContext([fakePage.page], () => fakeCDP as unknown as CDPSession);

    mockLaunchPersistentContext.mockResolvedValue(fakeContext.context);

    const client = new PlaywrightBrowserHostClient({
      onScreencastFrame: () => {
        throw new Error("Faulty onScreencastFrame");
      },
    });

    const received: ScreencastFrame[] = [];
    client.subscribeScreencast((frame) => {
      received.push(frame);
    });

    await client.startScreencast();

    fakeCDP.emit("Page.screencastFrame", {
      data: "frame-safe",
      sessionId: 77,
    });

    expect(received.length).toBe(1);
    expect(fakeCDP.send).toHaveBeenCalledWith("Page.screencastFrameAck", {
      sessionId: 77,
    });
  });
});
