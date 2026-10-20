import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Text, View } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { normalizeAddressBarInput } from "./url-utils";
import {
  ArrowLeft,
  ArrowRight,
  Bot,
  Copy,
  Download,
  ExternalLink,
  Globe,
  Maximize,
  Monitor,
  MousePointer2,
  PenTool,
  RotateCw,
  Smartphone,
  Tablet,
} from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";
import {
  EditingTextInput as TextInput,
  type EditingTextInputHandle,
} from "@/components/ui/text-input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { Theme } from "@/styles/theme";

type ViewportPreset = "responsive" | "desktop" | "tablet" | "mobile";

const ThemedArrowLeft = withUnistyles(ArrowLeft);
const ThemedArrowRight = withUnistyles(ArrowRight);
const ThemedBot = withUnistyles(Bot);
const ThemedCopy = withUnistyles(Copy);
const ThemedDownload = withUnistyles(Download);
const ThemedExternalLink = withUnistyles(ExternalLink);
const ThemedGlobe = withUnistyles(Globe);
const ThemedMaximize = withUnistyles(Maximize);
const ThemedMonitor = withUnistyles(Monitor);
const ThemedMousePointer2 = withUnistyles(MousePointer2);
const ThemedPenTool = withUnistyles(PenTool);
const ThemedRotateCw = withUnistyles(RotateCw);
const ThemedSmartphone = withUnistyles(Smartphone);
const ThemedTablet = withUnistyles(Tablet);

const foregroundIconMapping = (theme: Theme) => ({ color: theme.colors.foreground });
const mutedIconMapping = (theme: Theme) => ({ color: theme.colors.foregroundMuted });
const activeIconMapping = (theme: Theme) => ({ color: theme.colors.accentForeground });

export interface BrowserNavigationBarProps {
  url: string;
  canGoBack?: boolean;
  canGoForward?: boolean;
  isLoading?: boolean;
  viewportPreset?: ViewportPreset;
  onNavigate: (newUrl: string) => void;
  onGoBack?: () => void;
  onGoForward?: () => void;
  onReload?: () => void;
  onOpenImport?: () => void;
  onSendToAgent?: () => void;
  onCopyAddress?: (url: string) => void;
  onOpenExternally?: (url: string) => void;
  onToggleInspect?: () => void;
  isInspectActive?: boolean;
  onToggleMarkup?: () => void;
  isMarkupActive?: boolean;
  onSelectViewportPreset?: (preset: ViewportPreset) => void;
  testID?: string;
}

/** A chrome icon action with its tooltip, so the toolbar rows stay declarative. */
function ToolbarAction({
  tooltip,
  testID,
  onPress,
  disabled,
  variant,
  children,
}: {
  tooltip: string;
  testID: string;
  onPress: () => void;
  disabled?: boolean;
  variant?: "default" | "ghost" | "outline";
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger>
        <Button
          size="xs"
          variant={variant ?? "ghost"}
          style={styles.iconBtn}
          disabled={disabled}
          onPress={onPress}
          testID={testID}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="top" align="center" offset={8}>
        <Text style={styles.tooltipText}>{tooltip}</Text>
      </TooltipContent>
    </Tooltip>
  );
}

function BrowserNavigationControls({
  canGoBack,
  canGoForward,
  onGoBack,
  onGoForward,
  onReload,
}: {
  canGoBack: boolean;
  canGoForward: boolean;
  onGoBack?: () => void;
  onGoForward?: () => void;
  onReload?: () => void;
}) {
  return (
    <View style={styles.navControls}>
      <Button
        size="xs"
        variant="ghost"
        style={styles.iconBtn}
        disabled={!canGoBack || !onGoBack}
        onPress={onGoBack}
        testID="browser-back-btn"
      >
        <ThemedArrowLeft
          size={16}
          uniProps={canGoBack ? foregroundIconMapping : mutedIconMapping}
        />
      </Button>

      <Button
        size="xs"
        variant="ghost"
        style={styles.iconBtn}
        disabled={!canGoForward || !onGoForward}
        onPress={onGoForward}
        testID="browser-forward-btn"
      >
        <ThemedArrowRight
          size={16}
          uniProps={canGoForward ? foregroundIconMapping : mutedIconMapping}
        />
      </Button>

      <Button
        size="xs"
        variant="ghost"
        style={styles.iconBtn}
        disabled={!onReload}
        onPress={onReload}
        testID="browser-reload-btn"
      >
        <ThemedRotateCw size={14} uniProps={foregroundIconMapping} />
      </Button>
    </View>
  );
}

function BrowserAddressBar({
  url,
  onChangeAddress,
  onSubmit,
}: {
  url: string;
  onChangeAddress: (value: string) => void;
  onSubmit: () => void;
}) {
  const inputRef = useRef<EditingTextInputHandle | null>(null);

  // The editor owns its text; a navigation elsewhere in the pane replaces it.
  useEffect(() => {
    inputRef.current?.replaceText(url);
    onChangeAddress(url);
  }, [onChangeAddress, url]);

  return (
    <View style={styles.addressBar}>
      <ThemedGlobe size={14} uniProps={mutedIconMapping} style={styles.globeIcon} />
      <TextInput
        ref={inputRef}
        initialValue={url}
        onChangeText={onChangeAddress}
        onSubmitEditing={onSubmit}
        returnKeyType="go"
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="Enter address or localhost URL..."
        placeholderTextColor={styles.addressPlaceholder.color}
        style={styles.addressInput}
        testID="browser-address-input"
      />
    </View>
  );
}

function BrowserActionTools({
  onOpenImport,
  onToggleInspect,
  isInspectActive,
  onToggleMarkup,
  isMarkupActive,
  onSendToAgent,
}: {
  onOpenImport?: () => void;
  onToggleInspect?: () => void;
  isInspectActive: boolean;
  onToggleMarkup?: () => void;
  isMarkupActive: boolean;
  onSendToAgent?: () => void;
}) {
  return (
    <>
      {onOpenImport ? (
        <Tooltip>
          <TooltipTrigger>
            <Button
              size="xs"
              variant="outline"
              style={styles.importBtn}
              onPress={onOpenImport}
              testID="browser-import-btn"
            >
              <ThemedDownload size={12} uniProps={foregroundIconMapping} />
              <Text style={styles.importBtnText}>Import</Text>
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top" align="center" offset={8}>
            <Text style={styles.tooltipText}>Import browser cookies & sessions (Chrome/Edge)</Text>
          </TooltipContent>
        </Tooltip>
      ) : null}

      {onToggleInspect ? (
        <ToolbarAction
          tooltip="Inspect & select element on webpage"
          testID="browser-inspect-btn"
          onPress={onToggleInspect}
          variant={isInspectActive ? "default" : "ghost"}
        >
          <ThemedMousePointer2
            size={14}
            uniProps={isInspectActive ? activeIconMapping : foregroundIconMapping}
          />
        </ToolbarAction>
      ) : null}

      {onToggleMarkup ? (
        <ToolbarAction
          tooltip="Draw visual annotations & crops for agent"
          testID="browser-markup-btn"
          onPress={onToggleMarkup}
          variant={isMarkupActive ? "default" : "ghost"}
        >
          <ThemedPenTool
            size={14}
            uniProps={isMarkupActive ? activeIconMapping : foregroundIconMapping}
          />
        </ToolbarAction>
      ) : null}

      {onSendToAgent ? (
        <ToolbarAction
          tooltip="Send current URL to AI Agent"
          testID="browser-send-agent-btn"
          onPress={onSendToAgent}
        >
          <ThemedBot size={15} uniProps={foregroundIconMapping} />
        </ToolbarAction>
      ) : null}
    </>
  );
}

function BrowserUrlTools({
  url,
  onCopyAddress,
  onOpenExternally,
}: {
  url: string;
  onCopyAddress?: (url: string) => void;
  onOpenExternally?: (url: string) => void;
}) {
  const handleCopyAddress = useCallback(() => {
    onCopyAddress?.(url);
  }, [onCopyAddress, url]);

  const handleOpenExternally = useCallback(() => {
    onOpenExternally?.(url);
  }, [onOpenExternally, url]);

  return (
    <>
      {onCopyAddress ? (
        <ToolbarAction
          tooltip="Copy URL to clipboard"
          testID="browser-nav-copy-btn"
          onPress={handleCopyAddress}
        >
          <ThemedCopy size={14} uniProps={foregroundIconMapping} />
        </ToolbarAction>
      ) : null}

      {onOpenExternally ? (
        <ToolbarAction
          tooltip="Open in default browser (Chrome/Edge)"
          testID="browser-nav-external-btn"
          onPress={handleOpenExternally}
        >
          <ThemedExternalLink size={14} uniProps={foregroundIconMapping} />
        </ToolbarAction>
      ) : null}
    </>
  );
}

function BrowserViewportControls({
  viewportPreset,
  onSelectViewportPreset,
}: {
  viewportPreset: ViewportPreset;
  onSelectViewportPreset: (preset: ViewportPreset) => void;
}) {
  const handleSelectResponsive = useCallback(() => {
    onSelectViewportPreset("responsive");
  }, [onSelectViewportPreset]);
  const handleSelectDesktop = useCallback(() => {
    onSelectViewportPreset("desktop");
  }, [onSelectViewportPreset]);
  const handleSelectTablet = useCallback(() => {
    onSelectViewportPreset("tablet");
  }, [onSelectViewportPreset]);
  const handleSelectMobile = useCallback(() => {
    onSelectViewportPreset("mobile");
  }, [onSelectViewportPreset]);

  return (
    <View style={styles.viewportControls}>
      <ToolbarButton
        label="Responsive viewport"
        selected={viewportPreset === "responsive"}
        onPress={handleSelectResponsive}
        testID="viewport-responsive"
      >
        <ThemedMaximize
          size={12}
          uniProps={viewportPreset === "responsive" ? foregroundIconMapping : mutedIconMapping}
        />
      </ToolbarButton>
      <ToolbarButton
        label="Desktop viewport"
        selected={viewportPreset === "desktop"}
        onPress={handleSelectDesktop}
        testID="viewport-desktop"
      >
        <ThemedMonitor
          size={12}
          uniProps={viewportPreset === "desktop" ? foregroundIconMapping : mutedIconMapping}
        />
      </ToolbarButton>
      <ToolbarButton
        label="Tablet viewport"
        selected={viewportPreset === "tablet"}
        onPress={handleSelectTablet}
        testID="viewport-tablet"
      >
        <ThemedTablet
          size={12}
          uniProps={viewportPreset === "tablet" ? foregroundIconMapping : mutedIconMapping}
        />
      </ToolbarButton>
      <ToolbarButton
        label="Mobile viewport"
        selected={viewportPreset === "mobile"}
        onPress={handleSelectMobile}
        testID="viewport-mobile"
      >
        <ThemedSmartphone
          size={12}
          uniProps={viewportPreset === "mobile" ? foregroundIconMapping : mutedIconMapping}
        />
      </ToolbarButton>
    </View>
  );
}

export function BrowserNavigationBar({
  url,
  canGoBack = false,
  canGoForward = false,
  isLoading: _isLoading = false,
  viewportPreset = "responsive",
  onNavigate,
  onGoBack,
  onGoForward,
  onReload,
  onOpenImport,
  onSendToAgent,
  onCopyAddress,
  onOpenExternally,
  onToggleInspect,
  isInspectActive = false,
  onToggleMarkup,
  isMarkupActive = false,
  onSelectViewportPreset,
  testID = "browser-navigation-bar",
}: BrowserNavigationBarProps) {
  const [addressInput, setAddressInput] = useState(url);

  useEffect(() => {
    setAddressInput(url);
  }, [url]);

  const handleSubmit = useCallback(() => {
    const clean = addressInput.trim();
    if (!clean) return;
    const resolvedUrl = normalizeAddressBarInput(clean);
    onNavigate(resolvedUrl);
  }, [addressInput, onNavigate]);

  const handleChangeAddress = useCallback((value: string) => {
    setAddressInput(value);
  }, []);

  const viewportControls = useMemo(() => {
    if (!onSelectViewportPreset) return null;
    return (
      <BrowserViewportControls
        viewportPreset={viewportPreset}
        onSelectViewportPreset={onSelectViewportPreset}
      />
    );
  }, [onSelectViewportPreset, viewportPreset]);

  return (
    <View style={styles.container} testID={testID}>
      {/* Navigation Controls */}
      <BrowserNavigationControls
        canGoBack={canGoBack}
        canGoForward={canGoForward}
        onGoBack={onGoBack}
        onGoForward={onGoForward}
        onReload={onReload}
      />

      {/* Address Bar */}
      <BrowserAddressBar url={url} onChangeAddress={handleChangeAddress} onSubmit={handleSubmit} />

      {/* Action Tools */}
      <View style={styles.tools}>
        <BrowserActionTools
          onOpenImport={onOpenImport}
          onToggleInspect={onToggleInspect}
          isInspectActive={isInspectActive}
          onToggleMarkup={onToggleMarkup}
          isMarkupActive={isMarkupActive}
          onSendToAgent={onSendToAgent}
        />
        <BrowserUrlTools
          url={url}
          onCopyAddress={onCopyAddress}
          onOpenExternally={onOpenExternally}
        />
        {viewportControls}
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[1.5],
    backgroundColor: theme.colors.background,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    minHeight: 42,
  },
  navControls: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  iconBtn: {
    width: 28,
    height: 28,
    padding: 0,
    alignItems: "center",
    justifyContent: "center",
  },
  importBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    height: 28,
    paddingHorizontal: theme.spacing[2],
  },
  importBtnText: {
    fontSize: 11,
    fontWeight: theme.fontWeight.medium,
    color: theme.colors.foreground,
  },
  addressBar: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: theme.colors.surface0,
    borderRadius: theme.borderRadius.md,
    borderWidth: 1,
    borderColor: theme.colors.border,
    paddingHorizontal: theme.spacing[2],
    height: 30,
  },
  globeIcon: {
    marginRight: theme.spacing[1.5],
  },
  addressInput: {
    flex: 1,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    paddingVertical: 0,
    height: "100%",
  },
  addressPlaceholder: {
    color: theme.colors.foregroundMuted,
  },
  tools: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  viewportControls: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: theme.colors.surface1,
    borderRadius: theme.borderRadius.sm,
    padding: 2,
    gap: 2,
    borderWidth: 1,
    borderColor: theme.colors.border,
  },
  tooltipText: {
    fontSize: theme.fontSize.sm,
    color: "#ffffff",
    fontWeight: theme.fontWeight.medium,
  },
}));
