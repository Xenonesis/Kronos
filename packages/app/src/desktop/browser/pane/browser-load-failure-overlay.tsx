import React, { useMemo } from "react";
import { Pressable, Text, View } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Copy, ExternalLink, Globe, RotateCw } from "lucide-react-native";
import { Button } from "@/components/ui/button";

export interface BrowserLoadFailureOverlayProps {
  currentUrl: string;
  onRetry: () => void;
  onTryHttps?: (httpsUrl: string) => void;
  onCopyAddress: (url: string) => void;
  onOpenExternally?: (url: string) => void;
  testID?: string;
}

export function parseHostFromUrl(rawUrl: string): string {
  try {
    const parsed = new URL(rawUrl.startsWith("http") ? rawUrl : `http://${rawUrl}`);
    return parsed.host || rawUrl;
  } catch {
    return rawUrl;
  }
}

export function getHttpsRecoveryUrl(rawUrl: string): string | null {
  try {
    const parsed = new URL(rawUrl.startsWith("http") ? rawUrl : `http://${rawUrl}`);
    if (parsed.protocol === "http:") {
      parsed.protocol = "https:";
      return parsed.toString();
    }
    return null;
  } catch {
    return null;
  }
}

export function BrowserLoadFailureOverlay({
  currentUrl,
  onRetry,
  onTryHttps,
  onCopyAddress,
  onOpenExternally,
  testID = "browser-load-failure-overlay",
}: BrowserLoadFailureOverlayProps) {
  const { theme } = useUnistyles();
  const host = useMemo(() => parseHostFromUrl(currentUrl), [currentUrl]);
  const httpsUrl = useMemo(() => getHttpsRecoveryUrl(currentUrl), [currentUrl]);

  return (
    <View style={styles.container} testID={testID}>
      <View style={styles.card}>
        {/* Globe icon circle */}
        <View style={styles.iconCircle}>
          <Globe size={24} color={theme.colors.foregroundMuted} />
        </View>

        {/* Heading */}
        <Text style={styles.title}>{`Can't reach ${host}`}</Text>

        {/* Subtitle */}
        <Text style={styles.description}>
          {
            "We couldn't connect to your local server.\nIf this should be a local app, make sure the server is running and listening on the expected port."
          }
        </Text>

        {/* Action Buttons Row */}
        <View style={styles.actionsRow}>
          {httpsUrl && onTryHttps ? (
            <Button
              size="sm"
              variant="default"
              style={styles.actionBtn}
              onPress={() => onTryHttps(httpsUrl)}
              testID="browser-try-https-btn"
            >
              Try HTTPS
            </Button>
          ) : null}

          <Button
            size="sm"
            variant="outline"
            style={styles.actionBtn}
            onPress={onRetry}
            testID="browser-retry-btn"
          >
            <RotateCw size={14} color={theme.colors.foreground} />
            <Text style={styles.btnText}>Retry</Text>
          </Button>

          <Button
            size="sm"
            variant="outline"
            style={styles.actionBtn}
            onPress={() => onCopyAddress(currentUrl)}
            testID="browser-copy-btn"
          >
            <Copy size={14} color={theme.colors.foreground} />
            <Text style={styles.btnText}>Copy Address</Text>
          </Button>
        </View>

        {/* Open Externally Link Button */}
        {onOpenExternally ? (
          <Pressable
            style={styles.externalLinkRow}
            onPress={() => onOpenExternally(currentUrl)}
            testID="browser-open-external-btn"
          >
            <ExternalLink size={14} color={theme.colors.foregroundMuted} />
            <Text style={styles.externalLinkText}>Open Externally</Text>
          </Pressable>
        ) : null}
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    ...StyleSheet.absoluteFillObject,
    backgroundColor: theme.colors.background,
    alignItems: "center",
    justifyContent: "center",
    padding: theme.spacing[4],
    zIndex: 20,
  },
  card: {
    maxWidth: 500,
    width: "100%",
    alignItems: "center",
    paddingHorizontal: theme.spacing[4],
    paddingVertical: theme.spacing[6],
  },
  iconCircle: {
    width: 56,
    height: 56,
    borderRadius: 28,
    borderWidth: 1,
    borderColor: theme.colors.border,
    backgroundColor: theme.colors.surface2,
    alignItems: "center",
    justifyContent: "center",
    marginBottom: theme.spacing[4],
  },
  title: {
    fontSize: theme.fontSize.lg,
    fontWeight: theme.fontWeight.semibold,
    color: theme.colors.foreground,
    textAlign: "center",
    marginBottom: theme.spacing[2],
  },
  description: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    textAlign: "center",
    lineHeight: 20,
    maxWidth: 440,
    marginBottom: theme.spacing[6],
  },
  actionsRow: {
    flexDirection: "row",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "center",
    gap: theme.spacing[2],
    marginBottom: theme.spacing[4],
  },
  actionBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1.5],
    height: 36,
    paddingHorizontal: theme.spacing[3],
  },
  btnText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foreground,
    fontWeight: theme.fontWeight.medium,
  },
  externalLinkRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[1.5],
    paddingVertical: theme.spacing[2],
    paddingHorizontal: theme.spacing[3],
  },
  externalLinkText: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
    textDecorationLine: "underline",
  },
}));
