import fs from "node:fs/promises";
import {
  chromium,
  type Browser,
  type BrowserContext,
  type CDPSession,
  type Page,
} from "playwright";
import {
  type BrowserAutomationCommand,
  type BrowserAutomationCommandName,
  type BrowserAutomationExecuteRequest,
  type BrowserAutomationResult,
  BROWSER_AUTOMATION_COMMAND_NAMES,
} from "@getpaseo/protocol/browser-automation/rpc-schemas";
import type { BrowserHostClient } from "./broker.js";
import {
  type CaptureCurrentFrameOptions,
  type CapturedFrame,
  type PlaywrightBrowserHostClientOptions,
  type PlaywrightBrowserHostOnResponse,
  type PlaywrightBrowserHostOptions,
  type ScreencastFrame,
  type ScreencastFrameListener,
  type StartScreencastOptions,
  resolveDefaultBrowserUserDataDir,
} from "./types.js";

export * from "./types.js";

export class PlaywrightBrowserHostClient implements BrowserHostClient {
  public readonly id = "playwright-daemon-browser-host";
  public readonly hostKind = "playwright-daemon";
  public readonly supportedCommands: readonly BrowserAutomationCommandName[] =
    BROWSER_AUTOMATION_COMMAND_NAMES;

  private readonly onResponse: PlaywrightBrowserHostOnResponse;
  private readonly options: PlaywrightBrowserHostOptions;
  private browser: Browser | null = null;
  private context: BrowserContext | null = null;
  private readonly tabs = new Map<string, Page>();
  private activeTabId: string | null = null;
  private initializingPromise: Promise<{
    browser: Browser | null;
    context: BrowserContext;
  }> | null = null;

  public onScreencastFrame?: (frame: {
    data: string;
    metadata: { timestamp: number; url: string };
  }) => void;

  private readonly screencastListeners = new Set<ScreencastFrameListener>();
  private screencastSession: CDPSession | null = null;
  private screencastPage: Page | null = null;
  private isScreencastingActive = false;
  private screencastOptions: StartScreencastOptions = { format: "jpeg", quality: 60 };

  public constructor(
    onResponse: PlaywrightBrowserHostOnResponse,
    options?: PlaywrightBrowserHostOptions,
  );
  public constructor(options?: PlaywrightBrowserHostClientOptions);
  public constructor(
    onResponseOrOptions?: PlaywrightBrowserHostOnResponse | PlaywrightBrowserHostClientOptions,
    options?: PlaywrightBrowserHostOptions,
  ) {
    if (typeof onResponseOrOptions === "function") {
      this.onResponse = onResponseOrOptions;
      this.options = options ?? {};
      if (options?.onScreencastFrame) {
        this.onScreencastFrame = options.onScreencastFrame;
      }
    } else {
      this.onResponse = onResponseOrOptions?.onResponse ?? (() => {});
      this.options = onResponseOrOptions ?? {};
      if (onResponseOrOptions?.onScreencastFrame) {
        this.onScreencastFrame = onResponseOrOptions.onScreencastFrame;
      }
    }
  }

  public get isPersistent(): boolean {
    return this.options.persistent ?? true;
  }

  public get userDataDir(): string | null {
    return this.isPersistent
      ? (this.options.userDataDir ?? resolveDefaultBrowserUserDataDir())
      : null;
  }

  public get activeBrowserId(): string | null {
    return this.activeTabId;
  }

  public get isScreencastActive(): boolean {
    return this.isScreencastingActive;
  }

  /**
   * Subscribes a listener to receive live screencast frames.
   * Returns an unsubscribe callback.
   */
  public subscribeScreencast(
    listener: (frame: { data: string; metadata: { timestamp: number; url: string } }) => void,
  ): () => void {
    this.screencastListeners.add(listener);
    return () => {
      this.screencastListeners.delete(listener);
    };
  }

  private emitScreencastFrame(frame: {
    data: string;
    metadata: { timestamp: number; url: string };
  }): void {
    try {
      this.onScreencastFrame?.(frame);
    } catch {
      // Prevent listener errors from interrupting host
    }
    for (const listener of this.screencastListeners) {
      try {
        listener(frame);
      } catch {
        // Prevent subscriber errors from interrupting host
      }
    }
  }

  /**
   * Starts live screencast streaming on the active browser page via Chrome DevTools Protocol (CDP).
   */
  public async startScreencast(options?: StartScreencastOptions): Promise<void> {
    this.isScreencastingActive = true;
    this.screencastOptions = {
      format: options?.format ?? "jpeg",
      quality: options?.quality ?? 60,
      maxWidth: options?.maxWidth,
      maxHeight: options?.maxHeight,
    };
    if (options?.onFrame) {
      this.subscribeScreencast(options.onFrame);
    }

    const { context } = await this.ensureBrowser();
    if (this.tabs.size === 0) {
      const page = await context.newPage();
      this.registerPage(page);
    }

    const page = this.getActivePage(options?.browserId);
    await this.enableScreencastForPage(page);
  }

  /**
   * Stops live screencast streaming and cleans up the active CDP session.
   */
  public async stopScreencast(): Promise<void> {
    this.isScreencastingActive = false;
    await this.cleanupScreencastSession();
  }

  private async cleanupScreencastSession(): Promise<void> {
    const session = this.screencastSession;
    this.screencastSession = null;
    this.screencastPage = null;

    if (session) {
      try {
        await session.send("Page.stopScreencast");
      } catch {
        // Session or target may already be dead
      }
      try {
        await session.detach();
      } catch {
        // Session may already be detached
      }
    }
  }

  private async enableScreencastForPage(page: Page): Promise<void> {
    if (this.screencastPage === page && this.screencastSession) {
      return;
    }

    await this.cleanupScreencastSession();

    try {
      type PageWithContext = Page & { context?: () => BrowserContext };
      const pageWithContext = page as PageWithContext;
      const context =
        typeof pageWithContext.context === "function" ? pageWithContext.context() : this.context;

      type ContextWithCDP = BrowserContext & {
        newCDPSession?: (page: Page) => Promise<CDPSession>;
      };
      const contextWithCDP = context as ContextWithCDP | null;

      let session: CDPSession | null = null;
      if (contextWithCDP && typeof contextWithCDP.newCDPSession === "function") {
        session = await contextWithCDP.newCDPSession(page);
      }

      if (!session) {
        return;
      }

      this.screencastSession = session;
      this.screencastPage = page;

      session.on("Page.screencastFrame", async (payload) => {
        try {
          const rawTs = payload.metadata?.timestamp;
          const timestamp =
            typeof rawTs === "number" && rawTs > 0
              ? rawTs < 1e11
                ? Math.round(rawTs * 1000)
                : Math.round(rawTs)
              : Date.now();

          let url = "";
          try {
            url = page.url();
          } catch {
            // Page may be closed/closing
          }

          const frame: ScreencastFrame = {
            data: payload.data,
            metadata: {
              timestamp,
              url,
            },
          };

          this.emitScreencastFrame(frame);
        } finally {
          try {
            await session.send("Page.screencastFrameAck", { sessionId: payload.sessionId });
          } catch {
            // Session may be closed
          }
        }
      });

      const params: Record<string, unknown> = {
        format: this.screencastOptions.format ?? "jpeg",
        quality: this.screencastOptions.quality ?? 60,
      };
      if (this.screencastOptions.maxWidth !== undefined) {
        params.maxWidth = this.screencastOptions.maxWidth;
      }
      if (this.screencastOptions.maxHeight !== undefined) {
        params.maxHeight = this.screencastOptions.maxHeight;
      }

      await session.send("Page.startScreencast", params);
    } catch {
      // CDP session creation or screencast command failed gracefully
    }
  }

  private async ensureBrowser(): Promise<{ browser: Browser | null; context: BrowserContext }> {
    if (this.context) {
      return { browser: this.browser, context: this.context };
    }

    if (this.initializingPromise) {
      return this.initializingPromise;
    }

    this.initializingPromise = (async () => {
      try {
        const isPersistent = this.options.persistent ?? true;
        const headless = this.options.headless ?? true;
        const viewport = this.options.viewport === undefined ? null : this.options.viewport;
        const args = this.options.args;

        if (isPersistent) {
          const userDataDir = this.options.userDataDir ?? resolveDefaultBrowserUserDataDir();
          await fs.mkdir(userDataDir, { recursive: true });
          this.context = await chromium.launchPersistentContext(userDataDir, {
            headless,
            viewport,
            args,
          });
          this.browser = this.context.browser();
        } else {
          this.browser = await chromium.launch({
            headless,
            args,
          });
          this.context = await this.browser.newContext({
            viewport,
          });
        }

        this.context.on("close", () => {
          void this.cleanupScreencastSession();
          this.context = null;
          this.browser = null;
          this.tabs.clear();
          this.activeTabId = null;
        });

        this.context.on("page", (page) => {
          this.registerPage(page);
        });

        for (const page of this.context.pages()) {
          this.registerPage(page);
        }

        return { browser: this.browser, context: this.context };
      } finally {
        this.initializingPromise = null;
      }
    })();

    return this.initializingPromise;
  }

  private registerPage(page: Page, preferredId?: string): string {
    for (const [id, trackedPage] of this.tabs.entries()) {
      if (trackedPage === page) {
        return id;
      }
    }

    const tabId = preferredId ?? `${Date.now()}-${Math.random().toString(16).slice(2, 10)}`;
    this.tabs.set(tabId, page);
    if (!this.activeTabId) {
      this.activeTabId = tabId;
    }
    if (this.isScreencastingActive && !this.screencastSession) {
      void this.enableScreencastForPage(page);
    }

    page.once("close", () => {
      if (this.screencastPage === page) {
        void this.cleanupScreencastSession();
      }
      this.tabs.delete(tabId);
      if (this.activeTabId === tabId) {
        this.activeTabId = this.tabs.keys().next().value ?? null;
      }
      if (this.isScreencastingActive && this.activeTabId) {
        const nextTab = this.tabs.get(this.activeTabId);
        if (nextTab && nextTab !== page) {
          void this.enableScreencastForPage(nextTab);
        }
      }
    });

    return tabId;
  }

  private getActivePage(browserId?: string): Page {
    const id = browserId || this.activeTabId || this.tabs.keys().next().value;
    const page = id ? this.tabs.get(id) : null;
    if (!page) {
      throw new Error("No active browser tab found");
    }
    if (!this.activeTabId && id) {
      this.activeTabId = id;
    }
    if (this.isScreencastingActive && this.screencastPage && this.screencastPage !== page) {
      void this.enableScreencastForPage(page);
    }
    return page;
  }

  /**
   * Captures a live frame from the active (or specified) browser tab.
   * Encoded as base64 JPEG for real-time live agent stream inspection and preview.
   */
  public async captureCurrentFrame(options?: CaptureCurrentFrameOptions): Promise<CapturedFrame> {
    const { context } = await this.ensureBrowser();

    if (this.tabs.size === 0) {
      const page = await context.newPage();
      this.registerPage(page);
    }

    const page = this.getActivePage(options?.browserId);
    const rawQuality = options?.quality ?? 80;
    const quality = Math.min(100, Math.max(0, Math.round(rawQuality)));
    const fullPage = options?.fullPage ?? false;

    const buffer = await page.screenshot({
      type: "jpeg",
      quality,
      fullPage,
    });

    const data = buffer.toString("base64");
    const browserId = options?.browserId ?? this.activeTabId ?? this.tabs.keys().next().value ?? "";
    const url = page.url();
    const title = await page.title().catch(() => "");
    const timestamp = Date.now();

    return {
      data,
      dataUrl: `data:image/jpeg;base64,${data}`,
      mimeType: "image/jpeg",
      timestamp,
      url,
      title,
      browserId,
      toString() {
        return data;
      },
    };
  }

  public async sendBrowserAutomationRequest(
    request: BrowserAutomationExecuteRequest,
  ): Promise<void> {
    const { requestId, command } = request;
    try {
      const result = await this.execute(command);
      this.onResponse({
        type: "browser.automation.execute.response",
        payload: {
          requestId,
          ok: true,
          result: result as unknown as BrowserAutomationResult,
        },
      });
    } catch (err: unknown) {
      this.onResponse({
        type: "browser.automation.execute.response",
        payload: {
          requestId,
          ok: false,
          error: {
            code: "browser_unknown_error",
            message: err instanceof Error ? err.message : String(err),
            retryable: false,
          },
        },
      });
    }
  }

  private async execute(command: BrowserAutomationCommand): Promise<Record<string, unknown>> {
    const { context } = await this.ensureBrowser();

    switch (command.command) {
      case "list_tabs": {
        const tabs = await Promise.all(
          Array.from(this.tabs.entries()).map(async ([id, page]) => {
            const title = await page.title().catch(() => page.url());
            const url = page.url();
            const isActive = id === this.activeTabId;
            return {
              browserId: id,
              title,
              url,
              active: isActive,
              isActive,
              isLoading: false,
            };
          }),
        );
        return { tabs };
      }

      case "new_tab": {
        const page = await context.newPage();
        const tabId = this.registerPage(page);
        this.activeTabId = tabId;
        if (command.args?.url) {
          await page.goto(command.args.url);
        }
        return {
          browserId: tabId,
          title: await page.title().catch(() => page.url()),
          url: page.url(),
        };
      }

      case "close_tab": {
        const targetId = command.args?.browserId || this.activeTabId;
        if (targetId && this.tabs.has(targetId)) {
          const page = this.tabs.get(targetId)!;
          await page.close().catch(() => {});
          this.tabs.delete(targetId);
          if (this.activeTabId === targetId) {
            this.activeTabId = this.tabs.keys().next().value ?? null;
          }
        }
        return { closed: true };
      }

      case "navigate": {
        const page = this.getActivePage(command.args?.browserId);
        await page.goto(command.args.url);
        return {
          title: await page.title().catch(() => page.url()),
          url: page.url(),
        };
      }

      case "snapshot": {
        const page = this.getActivePage(command.args?.browserId);
        const content = await page.content();
        return {
          snapshot: content,
          title: await page.title().catch(() => page.url()),
          url: page.url(),
        };
      }

      case "screenshot": {
        const page = this.getActivePage(command.args?.browserId);
        const buffer = await page.screenshot({ fullPage: command.args?.fullPage ?? false });
        return {
          data: buffer.toString("base64"),
          mimeType: "image/png",
        };
      }

      case "click": {
        const page = this.getActivePage(command.args?.browserId);
        if (command.args?.ref) {
          await page.click(command.args.ref);
        }
        return { clicked: true };
      }

      case "type": {
        const page = this.getActivePage(command.args?.browserId);
        if (command.args?.text) {
          await page.keyboard.type(command.args.text);
        }
        return { typed: true };
      }

      case "scroll": {
        const page = this.getActivePage(command.args?.browserId);
        await page.mouse.wheel(command.args?.deltaX ?? 0, command.args?.deltaY ?? 0);
        return { scrolled: true };
      }

      case "evaluate": {
        const page = this.getActivePage(command.args?.browserId);
        const result = await page.evaluate(command.args.function);
        return { result };
      }

      case "reload": {
        const page = this.getActivePage(command.args?.browserId);
        await page.reload();
        return {
          title: await page.title().catch(() => page.url()),
          url: page.url(),
        };
      }

      case "back": {
        const page = this.getActivePage(command.args?.browserId);
        await page.goBack();
        return {
          title: await page.title().catch(() => page.url()),
          url: page.url(),
        };
      }

      case "forward": {
        const page = this.getActivePage(command.args?.browserId);
        await page.goForward();
        return {
          title: await page.title().catch(() => page.url()),
          url: page.url(),
        };
      }

      case "keypress": {
        const page = this.getActivePage(command.args?.browserId);
        if (command.args?.key) {
          await page.keyboard.press(command.args.key);
        }
        return { pressed: true };
      }

      case "resize": {
        const page = this.getActivePage(command.args?.browserId);
        if (command.args?.width && command.args?.height) {
          await page.setViewportSize({
            width: command.args.width,
            height: command.args.height,
          });
        }
        return { resized: true };
      }

      default:
        return { acknowledged: true };
    }
  }
  public async close(): Promise<void> {
    await this.stopScreencast();
    const context = this.context;
    const browser = this.browser;
    this.context = null;
    this.browser = null;
    this.tabs.clear();
    this.activeTabId = null;

    if (context) {
      await context.close().catch(() => {});
    }
    if (browser) {
      await browser.close().catch(() => {});
    }
  }
}
