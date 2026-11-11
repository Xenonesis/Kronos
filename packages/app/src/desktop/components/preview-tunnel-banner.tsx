import { memo, useState, useCallback } from "react";
import { View, Text, Pressable } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Globe, Copy, Check, ExternalLink, X, Radio } from "lucide-react-native";
import * as Clipboard from "expo-clipboard";
import { Button } from "@/components/ui/button";

export interface PreviewTunnelBannerProps {
  localPort: number;
  tunnelUrl: string;
  onStopTunnel: () => void;
  onOpenInBrowser?: (url: string) => void;
}

export const PreviewTunnelBanner = memo(function PreviewTunnelBanner({
  localPort,
  tunnelUrl,
  onStopTunnel,
  onOpenInBrowser,
}: PreviewTunnelBannerProps) {
  const { theme } = useUnistyles();
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    await Clipboard.setStringAsync(tunnelUrl);
    setCopied(true);
    setTimeout(() => {
      setCopied(false);
    }, 2000);
  }, [tunnelUrl]);

  const handleOpen = useCallback(() => {
    if (onOpenInBrowser) {
      onOpenInBrowser(tunnelUrl);
    } else if (typeof window !== "undefined") {
      window.open(tunnelUrl, "_blank", "noopener,noreferrer");
    }
  }, [tunnelUrl, onOpenInBrowser]);

  return (
    <View style={styles.container}>
      <View style={styles.left}>
        <View style={styles.liveIndicator}>
          <Radio size={14} color="#16a34a" />
          <Text style={styles.liveText}>LIVE PREVIEW</Text>
        </View>

        <View style={styles.portBadge}>
          <Text style={styles.portText}>localhost:{localPort}</Text>
        </View>

        <View style={styles.arrowContainer}>
          <Text style={styles.arrowText}>➔</Text>
        </View>

        <Pressable onPress={handleOpen} style={styles.urlContainer}>
          <Globe size={13} color={theme.colors.accent} />
          <Text style={styles.urlText} numberOfLines={1}>
            {tunnelUrl}
          </Text>
        </Pressable>
      </View>

      <View style={styles.right}>
        <Button size="sm" variant="outline" onPress={handleCopy}>
          {copied ? (
            <Check size={13} color={theme.colors.success ?? "#16a34a"} />
          ) : (
            <Copy size={13} color={theme.colors.foreground} />
          )}
          <Text style={styles.actionBtnText}>{copied ? "Copied" : "Copy URL"}</Text>
        </Button>

        <Button size="sm" variant="default" onPress={handleOpen}>
          <ExternalLink size={13} color="#ffffff" />
          <Text style={styles.openBtnText}>Open</Text>
        </Button>

        <Pressable onPress={onStopTunnel} style={styles.stopBtn} accessibilityLabel="Stop tunnel">
          <X size={15} color={theme.colors.foregroundMuted} />
        </Pressable>
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    backgroundColor: theme.colors.surface0,
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
    paddingHorizontal: theme.spacing[3],
    paddingVertical: theme.spacing[2],
    gap: theme.spacing[2],
  },
  left: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    flex: 1,
  },
  liveIndicator: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: "rgba(22, 163, 74, 0.12)",
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  liveText: {
    fontSize: 10,
    fontWeight: "700",
    color: "#16a34a",
    letterSpacing: 0.5,
  },
  portBadge: {
    backgroundColor: theme.colors.surface2,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
  },
  portText: {
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  arrowContainer: {
    paddingHorizontal: 2,
  },
  arrowText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  urlContainer: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    backgroundColor: "rgba(59, 130, 246, 0.08)",
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 4,
    borderRadius: theme.borderRadius.sm,
    borderWidth: 1,
    borderColor: "rgba(59, 130, 246, 0.2)",
    flex: 1,
    maxWidth: 380,
  },
  urlText: {
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.code,
    color: theme.colors.accent,
    fontWeight: "500",
  },
  right: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  actionBtnText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foreground,
    fontWeight: "500",
  },
  openBtnText: {
    fontSize: theme.fontSize.code,
    color: "#ffffff",
    fontWeight: "600",
  },
  stopBtn: {
    padding: theme.spacing[1],
  },
}));
