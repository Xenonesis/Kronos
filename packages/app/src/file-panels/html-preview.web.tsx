import React, { useCallback, useMemo, useRef, useState } from "react";
import { Linking, Platform, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";

import * as Clipboard from "expo-clipboard";
import type { DaemonClient } from "@getpaseo/client/internal/daemon-client";
import { useToast } from "@/contexts/toast-context";
import { BrowserTabBar, type BrowserTabItem } from "@/desktop/browser/pane/browser-tab-bar";
import { BrowserNavigationBar } from "@/desktop/browser/pane/browser-navigation-bar";
import {
  BrowserLoadFailureOverlay,
  parseHostFromUrl,
} from "@/desktop/browser/pane/browser-load-failure-overlay";
import { BrowserImportDialog } from "@/desktop/browser/pane/browser-import-dialog";
import { resolveIframeTargetUrl } from "@/desktop/browser/pane/url-utils";
import { useSendBrowserUrlToAgent } from "@/desktop/browser/pane/use-send-browser-url-to-agent";
import {
  InteractiveInspectOverlay,
  type InspectTargetInfo,
} from "@/desktop/browser/pane/interactive-inspect-overlay";
import {
  formatGrabPayloadAsText,
  type BrowserGrabPayload,
} from "@/desktop/browser/pane/browser-grab-payload";
import { withPreviewCsp } from "./html-preview-csp";
import { useInlinedHtmlAssets } from "./use-inlined-html-assets";

const SANDBOX = "allow-scripts allow-same-origin allow-forms allow-popups allow-modals";

interface HtmlTabState {
  id: string;
  url: string;
  title: string;
  isFile: boolean;
  history: string[];
  historyIndex: number;
  isLoading: boolean;
  hasLoadError: boolean;
}

export function FileHtmlPreview({
  html,
  cwd,
  filePath,
  client,
  testID = "file-html-preview",
}: {
  html: string;
  cwd?: string | null;
  filePath?: string | null;
  client?: DaemonClient | null;
  testID?: string;
}) {
  const toast = useToast();

  const fileName = useMemo(() => {
    if (!filePath) return "index.html";
    const parts = filePath.split(/[/\\]/);
    return parts[parts.length - 1] || "index.html";
  }, [filePath]);

  const displayFileAddress = useMemo(() => {
    return filePath || "index.html";
  }, [filePath]);

  // Tab management
  const [tabs, setTabs] = useState<HtmlTabState[]>([
    {
      id: "tab-file",
      url: displayFileAddress,
      title: fileName,
      isFile: true,
      history: [displayFileAddress],
      historyIndex: 0,
      isLoading: false,
      hasLoadError: false,
    },
  ]);
  const [activeTabId, setActiveTabId] = useState("tab-file");
  const [viewportPreset, setViewportPreset] = useState<
    "responsive" | "desktop" | "tablet" | "mobile"
  >("responsive");
  const [isInspectActive, setIsInspectActive] = useState(false);
  const [isImportOpen, setIsImportOpen] = useState(false);

  const activeTab = useMemo(
    () => tabs.find((item) => item.id === activeTabId) ?? tabs[0]!,
    [tabs, activeTabId],
  );

  // Inlined assets for the local HTML file
  const inlinedHtml = useInlinedHtmlAssets({ html, cwd, filePath, client });
  const inlinedDocument = useMemo(() => withPreviewCsp(inlinedHtml), [inlinedHtml]);

  const iframeRef = useRef<HTMLIFrameElement | null>(null);

  const navigateTo = useCallback(
    (newUrl: string) => {
      setTabs((prev) =>
        prev.map((tab) => {
          if (tab.id !== activeTabId) return tab;
          const newHistory = tab.history.slice(0, tab.historyIndex + 1);
          newHistory.push(newUrl);
          return {
            ...tab,
            url: newUrl,
            title: parseHostFromUrl(newUrl),
            isFile: false,
            hasLoadError: false,
            history: newHistory,
            historyIndex: newHistory.length - 1,
          };
        }),
      );
    },
    [activeTabId],
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
                isFile: prevUrl === displayFileAddress,
                title: prevUrl === displayFileAddress ? fileName : parseHostFromUrl(prevUrl),
                historyIndex: tab.historyIndex - 1,
              }
            : tab,
        ),
      );
    }
  }, [activeTab, activeTabId, displayFileAddress, fileName]);

  const handleGoForward = useCallback(() => {
    if (activeTab.historyIndex < activeTab.history.length - 1) {
      const nextUrl = activeTab.history[activeTab.historyIndex + 1]!;
      setTabs((prev) =>
        prev.map((tab) =>
          tab.id === activeTabId
            ? {
                ...tab,
                url: nextUrl,
                isFile: nextUrl === displayFileAddress,
                title: nextUrl === displayFileAddress ? fileName : parseHostFromUrl(nextUrl),
                historyIndex: tab.historyIndex + 1,
              }
            : tab,
        ),
      );
    }
  }, [activeTab, activeTabId, displayFileAddress, fileName]);

  const handleReload = useCallback(() => {
    setTabs((prev) =>
      prev.map((item) => (item.id === activeTabId ? { ...item, hasLoadError: false } : item)),
    );
    if (iframeRef.current) {
      if (activeTab.isFile) {
        iframeRef.current.srcdoc = inlinedDocument;
      } else {
        iframeRef.current.src = resolveIframeTargetUrl(activeTab.url);
      }
    }
  }, [activeTab, activeTabId, inlinedDocument]);
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

  const handleNewTab = useCallback(() => {
    const newId = `tab-${Date.now()}`;
    const defaultUrl = "https://google.com";
    const newTab: HtmlTabState = {
      id: newId,
      url: defaultUrl,
      title: "google.com",
      isFile: false,
      history: [defaultUrl],
      historyIndex: 0,
      isLoading: false,
      hasLoadError: false,
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTabId(newId);
  }, []);

  const handleCloseTab = useCallback(
    (tabId: string) => {
      setTabs((prev) => {
        if (prev.length <= 1) return prev;
        const filtered = prev.filter((item) => item.id !== tabId);
        if (activeTabId === tabId) {
          setActiveTabId(filtered[filtered.length - 1]!.id);
        }
        return filtered;
      });
    },
    [activeTabId],
  );

  const { sendToAgent } = useSendBrowserUrlToAgent();
  const handleSendToAgent = useCallback(() => {
    void sendToAgent(activeTab.url, activeTab.title);
  }, [activeTab.title, activeTab.url, sendToAgent]);
  const handleImportSession = useCallback(
    (_cookiesText: string, source: "chrome" | "edge" | "manual") => {
      toast.show(`Imported ${source} session cookies`);
      handleReload();
    },
    [handleReload, toast],
  );

  const handleOpenImport = useCallback(() => setIsImportOpen(true), []);
  const handleCloseImport = useCallback(() => setIsImportOpen(false), []);
  const handleToggleInspect = useCallback(() => setIsInspectActive((prev) => !prev), []);
  const handleCancelInspect = useCallback(() => setIsInspectActive(false), []);
  const handleIframeError = useCallback(() => {
    setTabs((prev) =>
      prev.map((item) => (item.id === activeTabId ? { ...item, hasLoadError: true } : item)),
    );
  }, [activeTabId]);

  const handleSelectElement = useCallback(
    (target: InspectTargetInfo) => {
      const elementDesc = `${target.tag}${target.id ? `#${target.id}` : ""}: "${(target.textSnippet || target.textContent || "").slice(0, 40)}"`;
      toast.show(`Selected element: ${elementDesc}`);
      setIsInspectActive(false);
      const payload: BrowserGrabPayload = {
        url:
          activeTab.url || target.payload?.url || (filePath ? `file://${filePath}` : "about:blank"),
        title: activeTab.title || target.payload?.title || (filePath ? filePath : undefined),
        target,
        timestamp: target.payload?.timestamp ?? Date.now(),
      };
      const formattedText = formatGrabPayloadAsText(payload);
      void sendToAgent(activeTab.url, formattedText);
    },
    [activeTab.title, activeTab.url, filePath, sendToAgent, toast],
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
      tabs.map((item) => ({
        id: item.id,
        url: item.url,
        title: item.title,
        isLoading: item.isLoading,
      })),
    [tabs],
  );

  return (
    <View style={styles.container} testID={testID ? `${testID}-shell` : undefined}>
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
        onSelectViewportPreset={setViewportPreset}
      />

      {/* 3. Main Viewport Content */}
      <View style={styles.viewportWrapper}>
        <View style={viewportStyle}>
          {Platform.OS === "web" && activeTab.isFile && (
            <iframe
              key={activeTab.id}
              ref={iframeRef}
              data-testid={testID}
              title={activeTab.title || "HTML Preview"}
              srcDoc={inlinedDocument}
              style={iframeInnerStyle}
              sandbox={SANDBOX}
              referrerPolicy="no-referrer"
            />
          )}
          {Platform.OS === "web" && !activeTab.isFile && (
            <iframe
              key={activeTab.id}
              ref={iframeRef}
              data-testid={testID}
              title={activeTab.title || "HTML Preview"}
              src={resolveIframeTargetUrl(activeTab.url)}
              style={iframeInnerStyle}
              sandbox={SANDBOX}
              referrerPolicy="no-referrer"
              onError={handleIframeError}
            />
          )}
          {/* Interactive Inspect Overlay on top of iframe */}
          <InteractiveInspectOverlay
            active={isInspectActive}
            iframeRef={iframeRef}
            onSelectElement={handleSelectElement}
            onCancel={handleCancelInspect}
          />

          {/* 4. Orca-style Load Failure Overlay if URL unreachable */}
          {activeTab.hasLoadError ? (
            <BrowserLoadFailureOverlay
              currentUrl={activeTab.url}
              onRetry={handleReload}
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

const iframeInnerStyle = {
  width: "100%",
  height: "100%",
  border: "none",
  backgroundColor: "white",
} as const;

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
