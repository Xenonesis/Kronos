/**
 * @vitest-environment jsdom
 */
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { theme } = vi.hoisted(() => ({
  theme: {
    spacing: { 0: 0, 1: 4, 2: 8, 3: 12, 4: 16 },
    iconSize: { sm: 14, md: 18 },
    borderWidth: { 1: 1 },
    borderRadius: { sm: 4, md: 6, lg: 8, xl: 12, full: 999 },
    fontSize: { xs: 11, sm: 13, base: 15 },
    fontWeight: { normal: "400", medium: "500", semibold: "600" },
    opacity: { 50: 0.5 },
    shadow: { md: {} },
    colors: {
      surface0: "#000000",
      surface1: "#111111",
      surface2: "#222222",
      surface3: "#333333",
      foreground: "#ffffff",
      foregroundMuted: "#aaaaaa",
      foregroundExtraMuted: "#777777",
      border: "#444444",
      borderAccent: "#666666",
      accent: "#0a84ff",
      accentForeground: "#000000",
      destructive: "#ff4444",
      destructiveForeground: "#000000",
      interactionHighlight: "rgba(255, 255, 255, 0.08)",
      popover: "#222222",
      popoverForeground: "#ffffff",
      palette: {
        green: { 500: "#22c55e" },
        red: { 300: "#f87171" },
      },
    },
  },
}));

vi.mock("react-native-unistyles", () => ({
  StyleSheet: {
    create: (factory: unknown) =>
      typeof factory === "function" ? (factory as (t: typeof theme) => unknown)(theme) : factory,
  },
  useUnistyles: () => ({ theme, rt: { breakpoint: undefined } }),
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

vi.mock("lucide-react-native", () => {
  const createIcon = (name: string) => (props: Record<string, unknown>) =>
    React.createElement("span", { ...props, "data-icon": name });
  return {
    Check: createIcon("Check"),
    CheckCircle: createIcon("CheckCircle"),
    ChevronLeft: createIcon("ChevronLeft"),
    ChevronRight: createIcon("ChevronRight"),
    Globe: createIcon("Globe"),
    RefreshCw: createIcon("RefreshCw"),
    X: createIcon("X"),
    Monitor: createIcon("Monitor"),
  };
});

vi.mock("@/components/ui/loading-spinner", () => ({
  LoadingSpinner: ({ color, size }: { color?: string; size?: string }) =>
    React.createElement("span", {
      "data-testid": "mock-loading-spinner",
      "data-color": color,
      "data-size": size,
    }),
}));

vi.stubGlobal("React", React);
vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);

/** A frame with its own metadata URL, hoisted so the JSX prop is not a fresh object. */
const frameWithMetadata = {
  data: "abc",
  metadata: { timestamp: 1234, url: "https://example.com/from-frame" },
};

import { BrowserStreamViewport } from "./browser-stream-viewport.js";

describe("BrowserStreamViewport", () => {
  let root: Root | null = null;
  let container: HTMLElement | null = null;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) {
      act(() => {
        root?.unmount();
      });
    }
    root = null;
    container?.remove();
    container = null;
  });

  it("renders active URL from prop, frame metadata, or fallback to about:blank", () => {
    act(() => {
      root?.render(<BrowserStreamViewport url="https://getpaseo.dev" />);
    });
    let urlEl = container?.querySelector('[data-testid="browser-stream-url"]');
    expect(urlEl?.textContent).toBe("https://getpaseo.dev");

    act(() => {
      root?.render(<BrowserStreamViewport frame={frameWithMetadata} />);
    });
    urlEl = container?.querySelector('[data-testid="browser-stream-url"]');
    expect(urlEl?.textContent).toBe("https://example.com/from-frame");

    act(() => {
      root?.render(<BrowserStreamViewport />);
    });
    urlEl = container?.querySelector('[data-testid="browser-stream-url"]');
    expect(urlEl?.textContent).toBe("about:blank");
  });

  it("renders the latest JPEG frame in responsive image view", () => {
    act(() => {
      root?.render(
        <BrowserStreamViewport frame="test-base64-frame-payload" url="https://example.com" />,
      );
    });

    const img = container?.querySelector('[data-testid="browser-stream-image"]');
    expect(img).not.toBeNull();
    const src = img?.getAttribute("src") ?? img?.querySelector("img")?.getAttribute("src");
    expect(src).toBe("data:image/jpeg;base64,test-base64-frame-payload");
  });

  it("preserves existing data URI if already prefixed", () => {
    const dataUri = "data:image/jpeg;base64,already-formatted-bytes";
    act(() => {
      root?.render(<BrowserStreamViewport frame={dataUri} />);
    });

    const img = container?.querySelector('[data-testid="browser-stream-image"]');
    const src = img?.getAttribute("src") ?? img?.querySelector("img")?.getAttribute("src");
    expect(src).toBe(dataUri);
  });

  it("renders canvas element when renderMode is canvas", () => {
    act(() => {
      root?.render(<BrowserStreamViewport renderMode="canvas" frame="base64-frame-canvas" />);
    });

    const canvas = container?.querySelector('[data-testid="browser-stream-canvas"]');
    expect(canvas).not.toBeNull();
  });

  it("shows placeholder when frame is null and not loading", () => {
    act(() => {
      root?.render(<BrowserStreamViewport frame={null} isLoading={false} />);
    });

    const placeholder = container?.querySelector('[data-testid="browser-stream-placeholder"]');
    expect(placeholder).not.toBeNull();
    expect(placeholder?.textContent).toContain("Waiting for active browser session frames");
  });

  it("shows loading indicator when isLoading is true", () => {
    act(() => {
      root?.render(<BrowserStreamViewport frame="some-frame" isLoading={true} />);
    });

    const indicator = container?.querySelector('[data-testid="browser-stream-loading-indicator"]');
    expect(indicator).not.toBeNull();
  });

  it("triggers onRefresh callback when Refresh button is pressed", () => {
    const handleRefresh = vi.fn();
    act(() => {
      root?.render(<BrowserStreamViewport frame="some-frame" onRefresh={handleRefresh} />);
    });

    const refreshBtn = container?.querySelector('[data-testid="browser-stream-refresh-btn"]');
    expect(refreshBtn).not.toBeNull();

    act(() => {
      refreshBtn?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(handleRefresh).toHaveBeenCalledTimes(1);
  });

  it("renders and triggers onClosePiP when provided", () => {
    const handleClosePiP = vi.fn();
    act(() => {
      root?.render(<BrowserStreamViewport frame="some-frame" onClosePiP={handleClosePiP} />);
    });

    const closeBtn = container?.querySelector('[data-testid="browser-stream-close-pip-btn"]');
    expect(closeBtn).not.toBeNull();

    act(() => {
      closeBtn?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(handleClosePiP).toHaveBeenCalledTimes(1);
  });

  it("omits Close PiP button when onClosePiP is not provided", () => {
    act(() => {
      root?.render(<BrowserStreamViewport frame="some-frame" />);
    });

    const closeBtn = container?.querySelector('[data-testid="browser-stream-close-pip-btn"]');
    expect(closeBtn).toBeNull();
  });
});
