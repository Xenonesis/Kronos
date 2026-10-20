import React, { useEffect, useMemo, useRef } from "react";
import {
  Image,
  Platform,
  Pressable,
  Text,
  View,
  type StyleProp,
  type ViewStyle,
} from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Globe, Monitor, RefreshCw, X } from "lucide-react-native";
import { LoadingSpinner } from "@/components/ui/loading-spinner";

export interface BrowserStreamFrameMetadata {
  timestamp?: number;
  url?: string;
  [key: string]: unknown;
}

export interface BrowserStreamFrame {
  data: string;
  metadata?: BrowserStreamFrameMetadata;
}

export interface BrowserStreamViewportProps {
  /**
   * Latest screencast frame data (base64 string, data URI, or frame object).
   */
  frame?: BrowserStreamFrame | string | null;

  /**
   * Current active URL to display in the header bar.
   * If omitted, falls back to frame.metadata.url or "about:blank".
   */
  url?: string;

  /**
   * Optional title to display.
   */
  title?: string;

  /**
   * Whether the stream or browser is currently loading.
   */
  isLoading?: boolean;

  /**
   * Mini toolbar callback to refresh the browser view.
   */
  onRefresh?: () => void;

  /**
   * Mini toolbar callback to close Picture-in-Picture (PiP) view.
   */
  onClosePiP?: () => void;

  /**
   * Whether the viewport is currently rendered in PiP mode.
   * Defaults to true if onClosePiP is supplied.
   */
  isPiP?: boolean;

  /**
   * Rendering mode for the frame: "image" (default) or "canvas" (for web).
   */
  renderMode?: "image" | "canvas";

  /**
   * Root container style.
   */
  style?: StyleProp<ViewStyle>;

  /**
   * Test ID for accessibility and testing.
   */
  testID?: string;
}

export function BrowserStreamViewport({
  frame,
  url,
  title,
  isLoading = false,
  onRefresh,
  onClosePiP,
  isPiP,
  renderMode = "image",
  style,
  testID = "browser-stream-viewport",
}: BrowserStreamViewportProps) {
  const { theme } = useUnistyles();
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  const imageUri = useMemo(() => {
    if (!frame) {
      return null;
    }
    const rawData = typeof frame === "string" ? frame : frame.data;
    if (!rawData || rawData.length === 0) {
      return null;
    }
    if (rawData.startsWith("data:")) {
      return rawData;
    }
    return `data:image/jpeg;base64,${rawData}`;
  }, [frame]);

  const activeUrl = useMemo(() => {
    if (url) {
      return url;
    }
    if (typeof frame === "object" && frame?.metadata?.url) {
      return frame.metadata.url;
    }
    return "about:blank";
  }, [url, frame]);

  useEffect(() => {
    if (
      renderMode === "canvas" &&
      Platform.OS === "web" &&
      imageUri &&
      canvasRef.current &&
      typeof window !== "undefined"
    ) {
      const canvas = canvasRef.current;
      let ctx: CanvasRenderingContext2D | null = null;
      try {
        ctx = canvas.getContext("2d");
      } catch {
        // Unsupported or unmocked canvas context in test/jsdom
      }
      if (ctx) {
        const win = window as unknown as Window & typeof globalThis;
        const img = new win.Image();
        img.onload = () => {
          canvas.width = img.naturalWidth || 1280;
          canvas.height = img.naturalHeight || 720;
          ctx.drawImage(img, 0, 0);
        };
        img.src = imageUri;
      }
    }
  }, [renderMode, imageUri]);
  const showClosePiP = Boolean(onClosePiP && isPiP !== false);

  return (
    <View testID={testID} style={[styles.container, style]}>
      {/* Top chrome with active URL and mini toolbar */}
      <View style={styles.header}>
        <View style={styles.urlContainer}>
          <Globe size={13} color={theme.colors.foregroundMuted} />
          <Text
            testID="browser-stream-url"
            accessibilityLabel={`Active URL: ${activeUrl}`}
            style={styles.urlText}
            numberOfLines={1}
            ellipsizeMode="tail"
          >
            {activeUrl}
          </Text>
        </View>

        <View style={styles.toolbar}>
          {isLoading ? (
            <View
              testID="browser-stream-loading-indicator"
              accessibilityLabel="Loading stream"
              style={styles.loadingWrapper}
            >
              <LoadingSpinner size="small" color={theme.colors.foregroundMuted} />
            </View>
          ) : null}

          <Pressable
            testID="browser-stream-refresh-btn"
            accessibilityRole="button"
            accessibilityLabel="Refresh"
            onPress={onRefresh}
            disabled={!onRefresh}
            style={({ pressed }) => [
              styles.toolbarButton,
              pressed ? styles.toolbarButtonPressed : null,
              !onRefresh ? styles.toolbarButtonDisabled : null,
            ]}
          >
            <RefreshCw size={13} color={theme.colors.foreground} />
          </Pressable>

          {showClosePiP ? (
            <Pressable
              testID="browser-stream-close-pip-btn"
              accessibilityRole="button"
              accessibilityLabel="Close PiP"
              onPress={onClosePiP}
              style={({ pressed }) => [
                styles.toolbarButton,
                pressed ? styles.toolbarButtonPressed : null,
              ]}
            >
              <X size={14} color={theme.colors.foreground} />
            </Pressable>
          ) : null}
        </View>
      </View>

      {/* Responsive canvas/image viewport */}
      <View style={styles.viewport}>
        {imageUri ? (
          renderMode === "canvas" && Platform.OS === "web" ? (
            <canvas
              ref={canvasRef as unknown as React.LegacyRef<HTMLCanvasElement>}
              data-testid="browser-stream-canvas"
              style={{
                width: "100%",
                height: "100%",
                objectFit: "contain",
                display: "block",
              }}
            />
          ) : (
            <Image
              testID="browser-stream-image"
              accessibilityRole="image"
              accessibilityLabel="Live browser screencast"
              source={{ uri: imageUri }}
              style={styles.streamImage}
              resizeMode="contain"
            />
          )
        ) : (
          <View testID="browser-stream-placeholder" style={styles.placeholderContainer}>
            {isLoading ? (
              <>
                <LoadingSpinner size="large" color={theme.colors.foregroundMuted} />
                <Text style={styles.placeholderText}>Connecting to browser stream...</Text>
              </>
            ) : (
              <>
                <Monitor size={32} color={theme.colors.foregroundMuted} />
                <Text style={styles.placeholderTitle}>{title || "No live stream"}</Text>
                <Text style={styles.placeholderText}>
                  Waiting for active browser session frames
                </Text>
              </>
            )}
          </View>
        )}
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    flex: 1,
    width: "100%",
    height: "100%",
    backgroundColor: theme.colors.surface0,
    overflow: "hidden",
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    backgroundColor: theme.colors.surface1,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    minHeight: 36,
    gap: theme.spacing[2],
  },
  urlContainer: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    minWidth: 0,
  },
  urlText: {
    flexShrink: 1,
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    fontFamily: Platform.select({ ios: "Menlo", default: "monospace" }),
  },
  toolbar: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1],
  },
  toolbarButton: {
    padding: theme.spacing[1],
    borderRadius: theme.borderRadius.sm,
    alignItems: "center",
    justifyContent: "center",
  },
  toolbarButtonPressed: {
    backgroundColor: theme.colors.surface2,
  },
  toolbarButtonDisabled: {
    opacity: 0.35,
  },
  loadingWrapper: {
    marginRight: theme.spacing[1],
    alignItems: "center",
    justifyContent: "center",
  },
  viewport: {
    flex: 1,
    width: "100%",
    height: "100%",
    backgroundColor: "#000000",
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
  },
  streamImage: {
    width: "100%",
    height: "100%",
  },
  placeholderContainer: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    padding: theme.spacing[4],
    gap: theme.spacing[2],
  },
  placeholderTitle: {
    fontSize: theme.fontSize.base,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  placeholderText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    textAlign: "center",
  },
}));
