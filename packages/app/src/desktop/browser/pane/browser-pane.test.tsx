// @vitest-environment jsdom
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";

afterEach(() => {
  cleanup();
});

vi.mock("expo-clipboard", () => ({
  setStringAsync: vi.fn(),
  getStringAsync: vi.fn(),
}));

vi.mock("react-native-reanimated", () => ({
  default: { View: "div" },
  FadeIn: { duration: () => undefined },
  FadeOut: { duration: () => undefined },
  Keyframe: class {
    duration() {
      return this;
    }
  },
  runOnJS: (fn: unknown) => fn,
  useAnimatedStyle: (factory: () => unknown) => factory(),
  useSharedValue: (value: unknown) => ({ value }),
}));

const { theme } = vi.hoisted(() => ({
  theme: {
    colors: {
      background: "#000",
      foreground: "#fff",
      foregroundMuted: "#888",
      foregroundExtraMuted: "#777",
      border: "#222",
      borderAccent: "#444",
      accent: "#0a84ff",
      accentForeground: "#fff",
      destructive: "#ff453a",
      destructiveForeground: "#fff",
      interactionHighlight: "rgba(255,255,255,0.08)",
      popover: "#181818",
      popoverForeground: "#fff",
      surface0: "#111",
      surface1: "#181818",
      surface2: "#222",
      surface3: "#333",
      surfaceSidebarHover: "#222",
      palette: {
        green: { 500: "#22c55e" },
        red: { 500: "#ef4444" },
        zinc: { 600: "#52525b" },
      },
    },
    spacing: { 0.5: 2, 1: 4, 1.5: 6, 2: 8, 3: 12, 4: 16, 6: 24, 8: 32 },
    iconSize: { xs: 12, sm: 14, md: 16, lg: 20 },
    borderWidth: { 1: 1, 2: 2 },
    fontSize: { sm: 12, base: 14, lg: 16 },
    fontWeight: { normal: "400", medium: "500", semibold: "600" },
    borderRadius: { sm: 4, md: 6, lg: 8, xl: 12, full: 999 },
    opacity: { 50: 0.5 },
    shadow: { md: {} },
  },
}));

// The shared `react-native-unistyles` stub in `packages/app/test-stubs` carries a
// narrower theme than the browser pane reads, so this file mocks the module
// itself with the tokens the pane's components use.
vi.mock("react-native-unistyles", () => ({
  StyleSheet: {
    create: (factory: unknown) =>
      typeof factory === "function" ? (factory as (t: unknown) => unknown)(theme) : factory,
    absoluteFillObject: { position: "absolute", top: 0, left: 0, right: 0, bottom: 0 },
  },
  useUnistyles: () => ({
    theme,
    rt: { breakpoint: "lg", isCompact: false },
  }),
  withUnistyles: (c: unknown) => c,
}));

vi.mock("@/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => children,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => children,
  TooltipContent: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock("lucide-react-native", async (importOriginal) => {
  const actual = await importOriginal<Record<string, unknown>>();
  const createIcon =
    (name: string) =>
    ({ style: _style, ...props }: Record<string, unknown>) =>
      React.createElement("span", { ...props, "data-icon": name });
  // The icon set is large and the modules under test import different slices of
  // it, so every icon resolves to the same stub rather than a fixed list that a
  // new import would break.
  return new Proxy(actual, {
    get: (target, property: string) =>
      typeof target[property] === "undefined" ? createIcon(property) : target[property],
  });
});

import {
  BrowserLoadFailureOverlay,
  getHttpsRecoveryUrl,
  parseHostFromUrl,
} from "./browser-load-failure-overlay";
import { BrowserNavigationBar } from "./browser-navigation-bar";
import { BrowserTabBar } from "./browser-tab-bar";
import { BrowserImportDialog } from "./browser-import-dialog";
import { isLocalhostUrl, resolveIframeTargetUrl } from "./url-utils";
describe("browser URL helpers", () => {
  it("parses host from full URL", () => {
    expect(parseHostFromUrl("http://localhost:3000/medical-inquiry")).toBe("localhost:3000");
    expect(parseHostFromUrl("https://example.com/api/test")).toBe("example.com");
    expect(parseHostFromUrl("localhost:8080")).toBe("localhost:8080");
  });

  it("generates https recovery url", () => {
    expect(getHttpsRecoveryUrl("http://localhost:3000/medical-inquiry")).toBe(
      "https://localhost:3000/medical-inquiry",
    );
    expect(getHttpsRecoveryUrl("https://localhost:3000")).toBeNull();
  });

  it("identifies localhost URLs and routes external sites through reverse proxy", () => {
    expect(isLocalhostUrl("http://localhost:3000")).toBe(true);
    expect(isLocalhostUrl("http://127.0.0.1:5173/app")).toBe(true);
    expect(isLocalhostUrl("https://google.com")).toBe(false);
    expect(isLocalhostUrl("https://github.com/trending")).toBe(false);

    // Localhost stays direct
    expect(resolveIframeTargetUrl("http://localhost:3000/medical-inquiry")).toBe(
      "http://localhost:3000/medical-inquiry",
    );

    // External sites are proxied to remove X-Frame-Options
    const proxiedGoogle = resolveIframeTargetUrl("https://google.com");
    expect(proxiedGoogle).toContain("/api/browser-proxy?url=https%3A%2F%2Fgoogle.com");
  });
});

describe("BrowserTabBar", () => {
  it("renders multiple tabs, switches tab, closes tab, and creates new tab", () => {
    const onSelectTab = vi.fn();
    const onCloseTab = vi.fn();
    const onNewTab = vi.fn();

    const tabs = [
      { id: "tab-1", url: "http://localhost:3000/medical-inquiry", title: "localhost:3000" },
      { id: "tab-2", url: "http://localhost:5173/dashboard", title: "localhost:5173" },
    ];

    const { getByTestId, getByText } = render(
      <BrowserTabBar
        tabs={tabs}
        activeTabId="tab-1"
        onSelectTab={onSelectTab}
        onCloseTab={onCloseTab}
        onNewTab={onNewTab}
      />,
    );

    expect(getByText("localhost:3000")).toBeDefined();
    expect(getByText("localhost:5173")).toBeDefined();

    fireEvent.click(getByTestId("browser-tab-tab-2"));
    expect(onSelectTab).toHaveBeenCalledWith("tab-2");

    fireEvent.click(getByTestId("browser-tab-close-tab-2"));
    expect(onCloseTab).toHaveBeenCalledWith("tab-2");

    fireEvent.click(getByTestId("browser-new-tab-btn"));
    expect(onNewTab).toHaveBeenCalled();
  });
});

describe("BrowserImportDialog", () => {
  it("renders sources and triggers import callback", () => {
    const onClose = vi.fn();
    const onImport = vi.fn();

    const { getByTestId, getByText } = render(
      <BrowserImportDialog visible={true} onClose={onClose} onImport={onImport} />,
    );

    expect(getByText("Import Browser Session & Cookies")).toBeDefined();
    expect(getByTestId("import-source-chrome")).toBeDefined();
    expect(getByTestId("import-source-edge")).toBeDefined();

    fireEvent.click(getByTestId("import-source-edge"));

    fireEvent.click(getByTestId("browser-import-confirm-btn"));
    expect(onImport).toHaveBeenCalledWith("", "edge");
  });
});

describe("BrowserLoadFailureOverlay", () => {
  it("renders host error title and guidance description matching Orca design", () => {
    const onRetry = vi.fn();
    const onCopy = vi.fn();

    const { getByText, getByTestId } = render(
      <BrowserLoadFailureOverlay
        currentUrl="http://localhost:3000/medical-inquiry"
        onRetry={onRetry}
        onCopyAddress={onCopy}
      />,
    );

    expect(getByText("Can't reach localhost:3000")).toBeDefined();
    expect(getByText(/We couldn't connect to your local server/)).toBeDefined();

    expect(getByTestId("browser-retry-btn")).toBeDefined();
    expect(getByTestId("browser-copy-btn")).toBeDefined();
  });

  it("handles Try HTTPS and Open Externally action buttons", () => {
    const onRetry = vi.fn();
    const onTryHttps = vi.fn();
    const onCopy = vi.fn();
    const onOpenExternally = vi.fn();

    const { getByTestId } = render(
      <BrowserLoadFailureOverlay
        currentUrl="http://localhost:3000/medical-inquiry"
        onRetry={onRetry}
        onTryHttps={onTryHttps}
        onCopyAddress={onCopy}
        onOpenExternally={onOpenExternally}
      />,
    );

    const tryHttpsBtn = getByTestId("browser-try-https-btn");
    fireEvent.click(tryHttpsBtn);
    expect(onTryHttps).toHaveBeenCalledWith("https://localhost:3000/medical-inquiry");

    const retryBtn = getByTestId("browser-retry-btn");
    fireEvent.click(retryBtn);
    expect(onRetry).toHaveBeenCalled();

    const copyBtn = getByTestId("browser-copy-btn");
    fireEvent.click(copyBtn);
    expect(onCopy).toHaveBeenCalledWith("http://localhost:3000/medical-inquiry");

    const externalBtn = getByTestId("browser-open-external-btn");
    fireEvent.click(externalBtn);
    expect(onOpenExternally).toHaveBeenCalledWith("http://localhost:3000/medical-inquiry");
  });
});

describe("BrowserNavigationBar", () => {
  it("renders address input, navigation buttons, import, and send to agent", () => {
    const onNavigate = vi.fn();
    const onGoBack = vi.fn();
    const onGoForward = vi.fn();
    const onReload = vi.fn();
    const onOpenImport = vi.fn();
    const onSendToAgent = vi.fn();

    const { getByTestId } = render(
      <BrowserNavigationBar
        url="http://localhost:3000/medical-inquiry"
        canGoBack={true}
        canGoForward={false}
        onNavigate={onNavigate}
        onGoBack={onGoBack}
        onGoForward={onGoForward}
        onReload={onReload}
        onOpenImport={onOpenImport}
        onSendToAgent={onSendToAgent}
      />,
    );

    const input = getByTestId("browser-address-input");
    expect(input).toBeDefined();

    const importBtn = getByTestId("browser-import-btn");
    fireEvent.click(importBtn);
    expect(onOpenImport).toHaveBeenCalled();

    const sendAgentBtn = getByTestId("browser-send-agent-btn");
    fireEvent.click(sendAgentBtn);
    expect(onSendToAgent).toHaveBeenCalled();

    const backBtn = getByTestId("browser-back-btn");
    fireEvent.click(backBtn);
    expect(onGoBack).toHaveBeenCalled();

    const reloadBtn = getByTestId("browser-reload-btn");
    fireEvent.click(reloadBtn);
    expect(onReload).toHaveBeenCalled();
  });

  it("handles viewport preset selection", () => {
    const onNavigate = vi.fn();
    const onSelectViewport = vi.fn();

    const { getByTestId } = render(
      <BrowserNavigationBar
        url="http://localhost:3000"
        onNavigate={onNavigate}
        onSelectViewportPreset={onSelectViewport}
      />,
    );

    fireEvent.click(getByTestId("viewport-desktop"));
    expect(onSelectViewport).toHaveBeenCalledWith("desktop");

    fireEvent.click(getByTestId("viewport-mobile"));
    expect(onSelectViewport).toHaveBeenCalledWith("mobile");
  });
});
