import React, { useCallback, useEffect, useState } from "react";
import {
  type NativeSyntheticEvent,
  Pressable,
  Text,
  TextInput,
  type TextInputSubmitEditingEventData,
  View,
} from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { parseHostFromUrl } from "./browser-load-failure-overlay";
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
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
export interface BrowserNavigationBarProps {
  url: string;
  canGoBack?: boolean;
  canGoForward?: boolean;
  isLoading?: boolean;
  viewportPreset?: "responsive" | "desktop" | "tablet" | "mobile";
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
  onSelectViewportPreset?: (preset: "responsive" | "desktop" | "tablet" | "mobile") => void;
  testID?: string;
}

export function BrowserNavigationBar({
  url,
  canGoBack = false,
  canGoForward = false,
  isLoading = false,
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
  const { theme } = useUnistyles();
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

  return (
    <View style={styles.container} testID={testID}>
      {/* Navigation Controls */}
      <View style={styles.navControls}>
        <Button
          size="xs"
          variant="ghost"
          style={styles.iconBtn}
          disabled={!canGoBack || !onGoBack}
          onPress={onGoBack}
          testID="browser-back-btn"
        >
          <ArrowLeft
            size={16}
            color={canGoBack ? theme.colors.foreground : theme.colors.foregroundMuted}
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
          <ArrowRight
            size={16}
            color={canGoForward ? theme.colors.foreground : theme.colors.foregroundMuted}
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
          <RotateCw size={14} color={theme.colors.foreground} />
        </Button>
      </View>

      {/* Address Bar */}
      <View style={styles.addressBar}>
        <Globe size={14} color={theme.colors.foregroundMuted} style={styles.globeIcon} />
        <TextInput
          value={addressInput}
          onChangeText={setAddressInput}
          onSubmitEditing={handleSubmit}
          returnKeyType="go"
          autoCapitalize="none"
          autoCorrect={false}
          placeholder="Enter address or localhost URL..."
          placeholderTextColor={theme.colors.foregroundMuted}
          style={styles.addressInput}
          testID="browser-address-input"
        />
      </View>

      {/* Action Tools */}
      <View style={styles.tools}>
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
                <Download size={12} color={theme.colors.foreground} />
                <Text style={styles.importBtnText}>Import</Text>
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top" align="center" offset={8}>
              <Text style={styles.tooltipText}>
                Import browser cookies & sessions (Chrome/Edge)
              </Text>
            </TooltipContent>
          </Tooltip>
        ) : null}

        {onToggleInspect ? (
          <Tooltip>
            <TooltipTrigger>
              <Button
                size="xs"
                variant={isInspectActive ? "default" : "ghost"}
                style={styles.iconBtn}
                onPress={onToggleInspect}
                testID="browser-inspect-btn"
              >
                <MousePointer2
                  size={14}
                  color={isInspectActive ? "#fff" : theme.colors.foreground}
                />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top" align="center" offset={8}>
              <Text style={styles.tooltipText}>Inspect & select element on webpage</Text>
            </TooltipContent>
          </Tooltip>
        ) : null}

        {onToggleMarkup ? (
          <Tooltip>
            <TooltipTrigger>
              <Button
                size="xs"
                variant={isMarkupActive ? "default" : "ghost"}
                style={styles.iconBtn}
                onPress={onToggleMarkup}
                testID="browser-markup-btn"
              >
                <PenTool size={14} color={isMarkupActive ? "#fff" : theme.colors.foreground} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top" align="center" offset={8}>
              <Text style={styles.tooltipText}>Draw visual annotations & crops for agent</Text>
            </TooltipContent>
          </Tooltip>
        ) : null}
        {onSendToAgent ? (
          <Tooltip>
            <TooltipTrigger>
              <Button
                size="xs"
                variant="ghost"
                style={styles.iconBtn}
                onPress={onSendToAgent}
                testID="browser-send-agent-btn"
              >
                <Bot size={15} color={theme.colors.foreground} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top" align="center" offset={8}>
              <Text style={styles.tooltipText}>Send current URL to AI Agent</Text>
            </TooltipContent>
          </Tooltip>
        ) : null}

        {onCopyAddress ? (
          <Tooltip>
            <TooltipTrigger>
              <Button
                size="xs"
                variant="ghost"
                style={styles.iconBtn}
                onPress={() => onCopyAddress(url)}
                testID="browser-nav-copy-btn"
              >
                <Copy size={14} color={theme.colors.foreground} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top" align="center" offset={8}>
              <Text style={styles.tooltipText}>Copy URL to clipboard</Text>
            </TooltipContent>
          </Tooltip>
        ) : null}

        {onOpenExternally ? (
          <Tooltip>
            <TooltipTrigger>
              <Button
                size="xs"
                variant="ghost"
                style={styles.iconBtn}
                onPress={() => onOpenExternally(url)}
                testID="browser-nav-external-btn"
              >
                <ExternalLink size={14} color={theme.colors.foreground} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top" align="center" offset={8}>
              <Text style={styles.tooltipText}>Open in default browser (Chrome/Edge)</Text>
            </TooltipContent>
          </Tooltip>
        ) : null}
        {onSelectViewportPreset ? (
          <View style={styles.viewportControls}>
            <Pressable
              style={[
                styles.viewportBtn,
                viewportPreset === "responsive" && styles.viewportBtnActive,
              ]}
              onPress={() => onSelectViewportPreset("responsive")}
              testID="viewport-responsive"
            >
              <Maximize
                size={12}
                color={
                  viewportPreset === "responsive"
                    ? theme.colors.foreground
                    : theme.colors.foregroundMuted
                }
              />
            </Pressable>
            <Pressable
              style={[styles.viewportBtn, viewportPreset === "desktop" && styles.viewportBtnActive]}
              onPress={() => onSelectViewportPreset("desktop")}
              testID="viewport-desktop"
            >
              <Monitor
                size={12}
                color={
                  viewportPreset === "desktop"
                    ? theme.colors.foreground
                    : theme.colors.foregroundMuted
                }
              />
            </Pressable>
            <Pressable
              style={[styles.viewportBtn, viewportPreset === "tablet" && styles.viewportBtnActive]}
              onPress={() => onSelectViewportPreset("tablet")}
              testID="viewport-tablet"
            >
              <Tablet
                size={12}
                color={
                  viewportPreset === "tablet"
                    ? theme.colors.foreground
                    : theme.colors.foregroundMuted
                }
              />
            </Pressable>
            <Pressable
              style={[styles.viewportBtn, viewportPreset === "mobile" && styles.viewportBtnActive]}
              onPress={() => onSelectViewportPreset("mobile")}
              testID="viewport-mobile"
            >
              <Smartphone
                size={12}
                color={
                  viewportPreset === "mobile"
                    ? theme.colors.foreground
                    : theme.colors.foregroundMuted
                }
              />
            </Pressable>
          </View>
        ) : null}
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
  viewportBtn: {
    width: 20,
    height: 20,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 2,
  },
  viewportBtnActive: {
    backgroundColor: theme.colors.surface3,
  },
  tooltipText: {
    fontSize: theme.fontSize.sm,
    color: "#ffffff",
    fontWeight: theme.fontWeight.medium,
  },
}));
