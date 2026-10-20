import path from "node:path";
import type { BrowserAutomationExecuteResponse } from "@getpaseo/protocol/browser-automation/rpc-schemas";
import { resolvePaseoHome } from "../paseo-home.js";

export const DEFAULT_BROWSER_PROFILE_DIR_NAME = "browser-profile";

/**
 * Resolves the default persistent browser user data directory.
 * Defaults to `<paseoHome>/browser-profile` (e.g. `~/.paseo/browser-profile`).
 */
export function resolveDefaultBrowserUserDataDir(paseoHome?: string): string {
  const home = paseoHome ?? resolvePaseoHome();
  return path.join(home, DEFAULT_BROWSER_PROFILE_DIR_NAME);
}

/**
 * Callback function to receive command execution responses.
 */
export type PlaywrightBrowserHostOnResponse = (response: BrowserAutomationExecuteResponse) => void;

/**
 * Options for configuring Playwright browser host instance.
 */
export interface PlaywrightBrowserHostOptions {
  /**
   * Path to the persistent browser user data directory where cookies,
   * local storage, cache, and session state are preserved.
   *
   * Defaults to `${resolvePaseoHome()}/browser-profile`.
   */
  userDataDir?: string;

  /**
   * Whether to launch the browser in headless mode.
   *
   * Defaults to `true`.
   */
  headless?: boolean;

  /**
   * Whether to use persistent browser storage across sessions.
   * When `true`, uses `chromium.launchPersistentContext(userDataDir, ...)`.
   * When `false`, launches an ephemeral context via `chromium.launch(...)`.
   *
   * Defaults to `true`.
   */
  persistent?: boolean;

  /**
   * Default viewport dimensions for browser pages.
   * Set to `null` to use default window size.
   */
  viewport?: {
    width: number;
    height: number;
  } | null;

  /**
   * Additional command-line flags passed to the Chromium process.
   */
  args?: string[];

  /**
   * Callback function called whenever a live screencast frame is emitted.
   */
  onScreencastFrame?: (frame: ScreencastFrame) => void;
}

/**
 * Configuration options when constructing a PlaywrightBrowserHostClient,
 * optionally including the response callback.
 */
export interface PlaywrightBrowserHostClientOptions extends PlaywrightBrowserHostOptions {
  onResponse?: PlaywrightBrowserHostOnResponse;
  onScreencastFrame?: (frame: ScreencastFrame) => void;
}

/**
 * Frame data delivered by the live screencast stream.
 */
export interface ScreencastFrame {
  /**
   * Base64-encoded image data.
   */
  data: string;

  /**
   * Screencast frame metadata including timestamp and active page URL.
   */
  metadata: {
    timestamp: number;
    url: string;
  };
}

/**
 * Listener function called whenever a screencast frame arrives.
 */
export type ScreencastFrameListener = (frame: ScreencastFrame) => void;

/**
 * Options for configuring live screencast streaming.
 */
export interface StartScreencastOptions {
  /**
   * Compression format for screencast frames (`jpeg` or `png`). Defaults to `jpeg`.
   */
  format?: "jpeg" | "png";

  /**
   * Compression quality between 0 and 100. Defaults to 60.
   */
  quality?: number;

  /**
   * Target browser tab ID to screencast. If omitted, uses active tab.
   */
  browserId?: string;

  /**
   * Maximum frame width in pixels.
   */
  maxWidth?: number;

  /**
   * Maximum frame height in pixels.
   */
  maxHeight?: number;

  /**
   * Optional listener to receive screencast frames.
   */
  onFrame?: ScreencastFrameListener;
}

/**
 * Options for capturing a live stream frame from a browser page.
 */
export interface CaptureCurrentFrameOptions {
  /**
   * Target browser tab ID to capture. If omitted, captures the active tab.
   */
  browserId?: string;

  /**
   * JPEG compression quality between 0 and 100.
   *
   * Defaults to 80.
   */
  quality?: number;

  /**
   * When true, takes a screenshot of the entire scrollable page.
   * When false, captures only the current viewport.
   *
   * Defaults to `false`.
   */
  fullPage?: boolean;
}

/**
 * Result of capturing a live frame from the active browser page.
 */
export interface CapturedFrame {
  /**
   * Base64-encoded JPEG image string (without data URI prefix).
   */
  data: string;

  /**
   * Full data URI suitable for direct `<img src="...">` rendering.
   */
  dataUrl: string;

  /**
   * MIME type of the captured image (`image/jpeg`).
   */
  mimeType: "image/jpeg";

  /**
   * Timestamp in milliseconds when the frame was captured.
   */
  timestamp: number;

  /**
   * URL of the page at the time of capture.
   */
  url: string;

  /**
   * Title of the page at the time of capture.
   */
  title: string;

  /**
   * ID of the browser tab that was captured.
   */
  browserId: string;

  /**
   * Returns the base64 JPEG image string.
   */
  toString(): string;
}
