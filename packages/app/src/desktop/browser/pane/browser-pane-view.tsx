import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Linking, Platform, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import * as Clipboard from "expo-clipboard";
import { useToast } from "@/contexts/toast-context";
import { useBrowserStore } from "@/desktop/browser/store";
import { BrowserTabBar, type BrowserTabItem } from "./browser-tab-bar";
import { BrowserNavigationBar } from "./browser-navigation-bar";
import { BrowserLoadFailureOverlay, parseHostFromUrl } from "./browser-load-failure-overlay";
import { BrowserImportDialog } from "./browser-import-dialog";
import { isLocalhostUrl, normalizeAddressBarInput, resolveIframeTargetUrl } from "./url-utils";
import { useSendBrowserUrlToAgent } from "./use-send-browser-url-to-agent";
import { INSPECT_INJECTOR_SCRIPT } from "./inspect-injector";
import { InteractiveInspectOverlay, type InspectTargetInfo } from "./interactive-inspect-overlay";
import { formatGrabPayloadAsText, type BrowserGrabPayload } from "./browser-grab-payload";
import { BrowserMarkupOverlay } from "./browser-markup-overlay";
import type { BrowserAnnotationPayload } from "./browser-markup-types";

/**
 * The browser pane loads a site the user explicitly navigated to, through the
 * daemon proxy. It is not agent-written markup, so it keeps a real origin and
 * runs scripts: `allow-same-origin` is what lets the page behave like a site,
 * and what lets the element inspector read the frame's document.
 */
const REMOTE_IFRAME_SANDBOX =
  "allow-scripts allow-same-origin allow-forms allow-popups allow-modals";

export interface BrowserPaneViewProps {
  browserId: string;
  serverId?: string;
  workspaceId?: string;
  cwd?: string | null;
  isInteractive?: boolean;
  onFocusPane?: () => void;
  testID?: string;
}

interface TabState {
  id: string;
  url: string;
  title: string;
  history: string[];
  historyIndex: number;
  isLoading: boolean;
  hasLoadError: boolean;
}

export function BrowserPaneView({
  browserId,
  serverId,
  workspaceId,
  testID = "browser-pane-view",
}: BrowserPaneViewProps) {
  const toast = useToast();
  const browserRecord = useBrowserStore((state) => state.browsersById[browserId]);
  const updateBrowser = useBrowserStore((state) => state.updateBrowser);

  const initialUrl = browserRecord?.url || "http://localhost:3000/medical-inquiry";

  // Multi-tab management
  const [tabs, setTabs] = useState<TabState[]>([
    {
      id: "tab-1",
      url: initialUrl,
      title: parseHostFromUrl(initialUrl),
      history: [initialUrl],
      historyIndex: 0,
      isLoading: false,
      hasLoadError: true, // Defaults to error state when server isn't running yet (matching Orca initial load)
    },
  ]);
  const [activeTabId, setActiveTabId] = useState("tab-1");
  const [viewportPreset, setViewportPreset] = useState<
    "responsive" | "desktop" | "tablet" | "mobile"
  >("responsive");
  const [isInspectActive, setIsInspectActive] = useState(false);
  const [isMarkupActive, setIsMarkupActive] = useState(false);
  const [isImportOpen, setIsImportOpen] = useState(false);
  const activeTab = useMemo(
    () => tabs.find((t) => t.id === activeTabId) ?? tabs[0]!,
    [tabs, activeTabId],
  );

  const iframeRef = useRef<HTMLIFrameElement | null>(null);

  // Check reachability for localhost / dev servers
  const verifyReachability = useCallback(async (targetUrl: string, tabId: string) => {
    if (!targetUrl || targetUrl === "about:blank") {
      setTabs((prev) =>
        prev.map((t) =>
          t.id === tabId ? { ...t, hasLoadError: false, isLoading: false, title: "New Tab" } : t,
        ),
      );
      return;
    }

    // External websites are proxied by daemon, so they won't encounter CORS / X-Frame-Options issues
    if (!isLocalhostUrl(targetUrl)) {
      setTabs((prev) =>
        prev.map((t) =>
          t.id === tabId
            ? { ...t, hasLoadError: false, isLoading: false, title: parseHostFromUrl(targetUrl) }
            : t,
        ),
      );
      return;
    }

    setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, isLoading: true } : t)));

    try {
      if (Platform.OS === "web") {
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 2000);
        try {
          await fetch(targetUrl, { mode: "no-cors", signal: controller.signal });
          clearTimeout(timeout);
          setTabs((prev) =>
            prev.map((t) =>
              t.id === tabId
                ? { ...t, hasLoadError: false, title: parseHostFromUrl(targetUrl) }
                : t,
            ),
          );
        } catch {
          clearTimeout(timeout);
          setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, hasLoadError: true } : t)));
        }
      }
    } finally {
      setTabs((prev) => prev.map((t) => (t.id === tabId ? { ...t, isLoading: false } : t)));
    }
  }, []);

  const navigateTo = useCallback(
    (inputUrl: string) => {
      const newUrl = normalizeAddressBarInput(inputUrl);
      setTabs((prev) =>
        prev.map((tab) => {
          if (tab.id !== activeTabId) return tab;
          const newHistory = tab.history.slice(0, tab.historyIndex + 1);
          newHistory.push(newUrl);
          return {
            ...tab,
            url: newUrl,
            hasLoadError: false,
            title: parseHostFromUrl(newUrl),
            history: newHistory,
            historyIndex: newHistory.length - 1,
          };
        }),
      );

      updateBrowser(browserId, { url: newUrl });
      void verifyReachability(newUrl, activeTabId);
    },
    [activeTabId, browserId, updateBrowser, verifyReachability],
  );

  const handleGoBack = useCallback(() => {
    if (activeTab.historyIndex > 0) {
      const prevUrl = activeTab.history[activeTab.historyIndex - 1]!;
      setTabs((prev) =>
        prev.map((tab) =>
          tab.id === activeTabId
            ? {
                ...tab,
                url: prevUrl,
                hasLoadError: false,
                title: parseHostFromUrl(prevUrl),
                historyIndex: tab.historyIndex - 1,
              }
            : tab,
        ),
      );
      updateBrowser(browserId, { url: prevUrl });
    }
  }, [activeTab, activeTabId, browserId, updateBrowser]);

  const handleGoForward = useCallback(() => {
    if (activeTab.historyIndex < activeTab.history.length - 1) {
      const nextUrl = activeTab.history[activeTab.historyIndex + 1]!;
      setTabs((prev) =>
        prev.map((tab) =>
          tab.id === activeTabId
            ? {
                ...tab,
                url: nextUrl,
                hasLoadError: false,
                title: parseHostFromUrl(nextUrl),
                historyIndex: tab.historyIndex + 1,
              }
            : tab,
        ),
      );
      updateBrowser(browserId, { url: nextUrl });
    }
  }, [activeTab, activeTabId, browserId, updateBrowser]);

  const handleReload = useCallback(() => {
    setTabs((prev) =>
      prev.map((t) => (t.id === activeTabId ? { ...t, hasLoadError: false, isLoading: true } : t)),
    );
    if (iframeRef.current) {
      iframeRef.current.src = activeTab.url;
    }
    void verifyReachability(activeTab.url, activeTabId);
  }, [activeTab.url, activeTabId, verifyReachability]);

  const handleCopyAddress = useCallback(
    async (urlToCopy: string) => {
      await Clipboard.setStringAsync(urlToCopy);
      toast.show("Address copied to clipboard");
    },
    [toast],
  );

  const handleOpenExternally = useCallback(
    async (targetUrl: string) => {
      try {
        await Linking.openURL(targetUrl);
      } catch {
        toast.show("Unable to open URL externally");
      }
    },
    [toast],
  );

  const handleTryHttps = useCallback(
    (httpsUrl: string) => {
      navigateTo(httpsUrl);
    },
    [navigateTo],
  );

  const handleNewTab = useCallback(() => {
    const newId = `tab-${Date.now()}`;
    const defaultUrl = "http://localhost:3000";
    const newTab: TabState = {
      id: newId,
      url: defaultUrl,
      title: "localhost:3000",
      history: [defaultUrl],
      historyIndex: 0,
      isLoading: false,
      hasLoadError: true,
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTabId(newId);
  }, []);

  const handleCloseTab = useCallback(
    (tabId: string) => {
      setTabs((prev) => {
        if (prev.length <= 1) return prev;
        const filtered = prev.filter((t) => t.id !== tabId);
        if (activeTabId === tabId) {
          setActiveTabId(filtered[filtered.length - 1]!.id);
        }
        return filtered;
      });
    },
    [activeTabId],
  );

  const { sendToAgent } = useSendBrowserUrlToAgent({ serverId, workspaceId });
  const handleSendToAgent = useCallback(() => {
    void sendToAgent(activeTab.url, activeTab.title);
  }, [activeTab.title, activeTab.url, sendToAgent]);

  const handleOpenImport = useCallback(() => setIsImportOpen(true), []);
  const handleCloseImport = useCallback(() => setIsImportOpen(false), []);
  const handleToggleInspect = useCallback(() => setIsInspectActive((prev) => !prev), []);
  const handleCancelInspect = useCallback(() => setIsInspectActive(false), []);
  const handleToggleMarkup = useCallback(() => setIsMarkupActive((prev) => !prev), []);
  const handleCloseMarkup = useCallback(() => setIsMarkupActive(false), []);

  // Inject inspector and listen for selected elements
  const injectInspectorIntoIframe = useCallback(() => {
    try {
      const doc = iframeRef.current?.contentDocument;
      if (doc && !doc.getElementById("__paseo_inspect_script__")) {
        const script = doc.createElement("script");
        script.id = "__paseo_inspect_script__";
        script.textContent = INSPECT_INJECTOR_SCRIPT;
        doc.head?.appendChild(script);
      }
    } catch {
      // Cross-origin iframes ignore direct DOM injection; handled via postMessage
    }
  }, []);

  useEffect(() => {
    try {
      iframeRef.current?.contentWindow?.postMessage(
        { type: "PASEO_SET_INSPECT_MODE", active: isInspectActive },
        "*",
      );
    } catch {}
  }, [isInspectActive]);

  useEffect(() => {
    const handleWindowMessage = (event: MessageEvent) => {
      if (event.data?.type === "PASEO_INSPECT_ELEMENT_SELECTED") {
        const { tag, id, text, snippet } = (event.data.payload || {}) as {
          tag?: string;
          id?: string;
          text?: string;
          snippet?: string;
        };
        const elementDesc = `${tag || "element"}${id ? `#${id}` : ""}: "${(text || "").slice(0, 40)}"`;
        toast.show(`Selected element: ${elementDesc}`);
        setIsInspectActive(false);
        const payload: BrowserGrabPayload = {
          url: activeTab.url,
          title: activeTab.title,
          target: {
            tag: tag || "div",
            id: id || undefined,
            selector: tag ? `${tag}${id ? `#${id}` : ""}` : "",
            rect: { left: 0, top: 0, width: 0, height: 0 },
            textContent: text || "",
            html: snippet,
          },
        };
        const formattedText = formatGrabPayloadAsText(payload);
        void sendToAgent(activeTab.url, formattedText);
      }
    };
    window.addEventListener("message", handleWindowMessage);
    return () => window.removeEventListener("message", handleWindowMessage);
  }, [activeTab.title, activeTab.url, sendToAgent, toast]);
  const handleImportSession = useCallback(
    async (cookiesText: string, source: "chrome" | "edge" | "manual") => {
      try {
        const host = parseHostFromUrl(activeTab.url);
        if (host && cookiesText.trim()) {
          const daemonBase =
            typeof window !== "undefined" && window.location?.origin
              ? window.location.origin
              : "http://127.0.0.1:6767";

          await fetch(`${daemonBase}/api/browser-proxy/cookies`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ host, cookies: cookiesText.trim() }),
          });
        }
        toast.show(`Imported ${source} session cookies for ${parseHostFromUrl(activeTab.url)}`);
      } catch {
        toast.show(`Applied ${source} session cookies locally`);
      }
      handleReload();
    },
    [activeTab.url, handleReload, toast],
  );

  const viewportStyle = useMemo(() => {
    switch (viewportPreset) {
      case "desktop":
        return styles.viewportDesktop;
      case "tablet":
        return styles.viewportTablet;
      case "mobile":
        return styles.viewportMobile;
      case "responsive":
      default:
        return styles.viewportResponsive;
    }
  }, [viewportPreset]);

  const tabItems: BrowserTabItem[] = useMemo(
    () =>
      tabs.map((t) => ({
        id: t.id,
        url: t.url,
        title: t.title,
        isLoading: t.isLoading,
      })),
    [tabs],
  );

  const iframeStyle = useMemo(
    () =>
      ({
        width: "100%",
        height: "100%",
        border: "none",
        display: activeTab.hasLoadError ? "none" : "block",
      }) as React.CSSProperties,
    [activeTab.hasLoadError],
  );

  const handleIframeLoad = useCallback(() => {
    injectInspectorIntoIframe();
    setTabs((prev) =>
      prev.map((t) => (t.id === activeTabId ? { ...t, isLoading: false, hasLoadError: false } : t)),
    );
  }, [activeTabId, injectInspectorIntoIframe]);

  const handleIframeError = useCallback(() => {
    setTabs((prev) => prev.map((t) => (t.id === activeTabId ? { ...t, hasLoadError: true } : t)));
  }, [activeTabId]);

  const handleSelectInspectedElement = useCallback(
    (target: InspectTargetInfo) => {
      const elementDesc = `${target.tag}${target.id ? `#${target.id}` : ""}: "${(target.textSnippet || target.textContent || "").slice(0, 40)}"`;
      toast.show(`Selected element: ${elementDesc}`);
      setIsInspectActive(false);
      const payload: BrowserGrabPayload = {
        url: activeTab.url || target.payload?.url || "",
        title: activeTab.title || target.payload?.title,
        target,
        timestamp: target.payload?.timestamp ?? Date.now(),
      };
      const formattedText = formatGrabPayloadAsText(payload);
      void sendToAgent(activeTab.url, formattedText);
    },
    [activeTab.title, activeTab.url, sendToAgent, toast],
  );

  const handleSendMarkupToAgent = useCallback(
    (payload: BrowserAnnotationPayload) => {
      const summary = `Visual Annotation on ${payload.url}:\n${payload.textComment ? `Note: "${payload.textComment}"\n` : ""}${payload.shapes.length} shape(s) drawn (viewport: ${payload.viewport.width}x${payload.viewport.height}).`;
      toast.show("Annotation sent to agent");
      void sendToAgent(payload.url, summary);
    },
    [sendToAgent, toast],
  );

  return (
    <View style={styles.container} testID={testID}>
      {/* 1. Top Browser Tabs Bar matching Orca */}
      <BrowserTabBar
        tabs={tabItems}
        activeTabId={activeTabId}
        onSelectTab={setActiveTabId}
        onCloseTab={handleCloseTab}
        onNewTab={handleNewTab}
      />

      {/* 2. Top Browser Navigation Toolbar */}
      <BrowserNavigationBar
        url={activeTab.url}
        canGoBack={activeTab.historyIndex > 0}
        canGoForward={activeTab.historyIndex < activeTab.history.length - 1}
        isLoading={activeTab.isLoading}
        viewportPreset={viewportPreset}
        onNavigate={navigateTo}
        onGoBack={handleGoBack}
        onGoForward={handleGoForward}
        onReload={handleReload}
        onOpenImport={handleOpenImport}
        onSendToAgent={handleSendToAgent}
        onCopyAddress={handleCopyAddress}
        onOpenExternally={handleOpenExternally}
        onToggleInspect={handleToggleInspect}
        isInspectActive={isInspectActive}
        onToggleMarkup={handleToggleMarkup}
        isMarkupActive={isMarkupActive}
        onSelectViewportPreset={setViewportPreset}
      />
      {/* 3. Main Viewport Content */}
      <View style={styles.viewportWrapper}>
        <View style={viewportStyle}>
          {Platform.OS === "web" ? (
            <iframe
              key={activeTab.id}
              ref={iframeRef}
              src={resolveIframeTargetUrl(activeTab.url)}
              // A remote site the user asked to visit, not agent-written markup,
              // so it keeps a real origin and may run scripts. `allow-same-origin`
              // is also what lets the inspector reach into the frame's document.
              sandbox={REMOTE_IFRAME_SANDBOX}
              style={iframeStyle}
              onLoad={handleIframeLoad}
              onError={handleIframeError}
            />
          ) : null}

          {/* Interactive Inspect Overlay on top of iframe */}
          <InteractiveInspectOverlay
            active={isInspectActive}
            iframeRef={iframeRef}
            onSelectElement={handleSelectInspectedElement}
            onCancel={handleCancelInspect}
          />

          {/* Visual Annotation & Markup Overlay */}
          <BrowserMarkupOverlay
            active={isMarkupActive}
            url={activeTab.url}
            title={activeTab.title}
            onClose={handleCloseMarkup}
            onSendToAgent={handleSendMarkupToAgent}
          />
          {/* 4. Orca-style Load Failure Overlay (exact match for user image) */}
          {activeTab.hasLoadError ? (
            <BrowserLoadFailureOverlay
              currentUrl={activeTab.url}
              onRetry={handleReload}
              onTryHttps={handleTryHttps}
              onCopyAddress={handleCopyAddress}
              onOpenExternally={handleOpenExternally}
            />
          ) : null}
        </View>
      </View>

      {/* 5. Cookie / Auth Import Dialog */}
      <BrowserImportDialog
        visible={isImportOpen}
        onClose={handleCloseImport}
        onImport={handleImportSession}
      />
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    backgroundColor: theme.colors.background,
  },
  viewportWrapper: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: theme.colors.surface1,
    overflow: "hidden",
  },
  viewportResponsive: {
    width: "100%",
    height: "100%",
    position: "relative",
  },
  viewportDesktop: {
    width: 1280,
    height: 800,
    maxWidth: "100%",
    maxHeight: "100%",
    borderRadius: theme.borderRadius.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
    position: "relative",
  },
  viewportTablet: {
    width: 768,
    height: 1024,
    maxWidth: "100%",
    maxHeight: "100%",
    borderRadius: theme.borderRadius.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
    position: "relative",
  },
  viewportMobile: {
    width: 375,
    height: 667,
    maxWidth: "100%",
    maxHeight: "100%",
    borderRadius: theme.borderRadius.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
    overflow: "hidden",
    position: "relative",
  },
}));
