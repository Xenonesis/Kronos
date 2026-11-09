import { useCallback, useEffect, useMemo, useState } from "react";
import { ScrollView, Text, View } from "react-native";
import * as Clipboard from "expo-clipboard";
import { useTranslation } from "react-i18next";
import { openExternalUrl } from "@/utils/open-external-url";
import { BookOpen, Copy, RotateCw, TriangleAlert } from "lucide-react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Button } from "@/components/ui/button";
import { PaseoLogo } from "@/components/icons/paseo-logo";
import { getDesktopDaemonLogs, type DesktopDaemonLogs } from "@/desktop/daemon/desktop-daemon";
import { TitlebarDragRegion } from "@/components/desktop/titlebar-drag-region";
import { isWeb } from "@/constants/platform";
import { CODE_SURFACE_DATASET } from "@/styles/code-surface";
import { LogoShimmer } from "./startup/logo-shimmer";

interface StartupSplashScreenProps {
  bootstrapState?: {
    splashError: string | null;
    retry: () => void;
  };
}

const GITHUB_ISSUE_URL = "https://github.com/getpaseo/paseo/issues/new";
const DOCS_URL = "https://paseo.sh/docs";

function openGithubIssue(): void {
  void openExternalUrl(GITHUB_ISSUE_URL);
}

function openDocs(): void {
  void openExternalUrl(DOCS_URL);
}

const ThemedCopy = withUnistyles(Copy, (theme) => ({
  color: theme.colors.foreground,
}));
const ThemedTriangleAlert = withUnistyles(TriangleAlert, (theme) => ({
  color: theme.colors.foreground,
}));
const ThemedBookOpen = withUnistyles(BookOpen, (theme) => ({
  color: theme.colors.foreground,
}));
const ThemedRotateCw = withUnistyles(RotateCw, (theme) => ({
  color: theme.colors.palette.white,
}));

export function StartupSplashScreen({ bootstrapState }: StartupSplashScreenProps) {
  const { t } = useTranslation();
  const [daemonLogs, setDaemonLogs] = useState<DesktopDaemonLogs | null>(null);
  const [logsError, setLogsError] = useState<string | null>(null);
  const [isLoadingLogs, setIsLoadingLogs] = useState(false);

  const isError = bootstrapState !== undefined && bootstrapState.splashError !== null;

  useEffect(() => {
    if (!isError) {
      return;
    }

    let isCancelled = false;
    setIsLoadingLogs(true);
    setLogsError(null);

    void getDesktopDaemonLogs()
      .then((logs) => {
        if (!isCancelled) {
          setDaemonLogs(logs);
          setIsLoadingLogs(false);
        }
        return null;
      })
      .catch((error) => {
        if (!isCancelled) {
          setLogsError(error instanceof Error ? error.message : t("startup.logs.loadFailed"));
          setIsLoadingLogs(false);
        }
        return null;
      });

    return () => {
      isCancelled = true;
    };
  }, [isError, t]);

  const logsText = useMemo(() => {
    if (isLoadingLogs) {
      return t("startup.logs.loading");
    }
    if (daemonLogs?.contents) {
      return daemonLogs.contents;
    }
    if (logsError) {
      return logsError;
    }
    return t("startup.logs.unavailable");
  }, [daemonLogs?.contents, isLoadingLogs, logsError, t]);

  const handleCopyLogs = useCallback(() => {
    const payload = daemonLogs?.logPath
      ? `${daemonLogs.logPath}\n\n${daemonLogs.contents}`
      : logsText;
    void Clipboard.setStringAsync(payload);
  }, [daemonLogs?.logPath, daemonLogs?.contents, logsText]);

  const copyIcon = useMemo(() => <ThemedCopy size={16} />, []);
  const warningIcon = useMemo(() => <ThemedTriangleAlert size={16} />, []);
  const bookIcon = useMemo(() => <ThemedBookOpen size={16} />, []);
  const retryIcon = useMemo(() => <ThemedRotateCw size={16} />, []);

  if (!isError) {
    return (
      <View testID="startup-splash" style={styles.container}>
        <TitlebarDragRegion />
        <LogoShimmer />
      </View>
    );
  }

  return (
    <View style={styles.errorScreen}>
      <TitlebarDragRegion />
      <ScrollView
        style={styles.errorScrollView}
        contentContainerStyle={styles.errorScrollContent}
        showsVerticalScrollIndicator
      >
        <View style={styles.errorContent}>
          <View style={styles.errorHeader}>
            <PaseoLogo size={64} />
            <Text style={styles.title}>{t("startup.errorTitle")}</Text>
          </View>

          <Text style={styles.errorDescription}>{t("startup.errorDescription")}</Text>

          <Text dataSet={CODE_SURFACE_DATASET} style={styles.errorMessage}>
            {bootstrapState.splashError}
          </Text>

          {daemonLogs?.logPath ? <Text style={styles.logsMeta}>{daemonLogs.logPath}</Text> : null}

          <View style={styles.logsContainer}>
            <ScrollView
              style={styles.logsScroll}
              contentContainerStyle={styles.logsContent}
              showsVerticalScrollIndicator
            >
              <Text dataSet={CODE_SURFACE_DATASET} selectable style={styles.logsText}>
                {logsText}
              </Text>
            </ScrollView>
          </View>

          <View style={styles.actionRow}>
            <Button variant="secondary" leftIcon={copyIcon} onPress={handleCopyLogs}>
              Copy logs
            </Button>
            <Button variant="outline" leftIcon={warningIcon} onPress={openGithubIssue}>
              Open GitHub issue
            </Button>
            <Button variant="outline" leftIcon={bookIcon} onPress={openDocs}>
              Docs
            </Button>
            <Button variant="default" leftIcon={retryIcon} onPress={bootstrapState.retry}>
              Retry
            </Button>
          </View>
        </View>
      </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  container: {
    position: "relative",
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: theme.colors.surface0,
    paddingHorizontal: theme.spacing[8],
    paddingVertical: theme.spacing[8],
  },
  errorScreen: {
    position: "relative",
    flex: 1,
    backgroundColor: theme.colors.surface0,
  },
  errorScrollView: {
    flex: 1,
    ...(isWeb
      ? {
          overflowX: "auto",
          overflowY: "auto",
          WebkitAppRegion: "no-drag",
        }
      : null),
  },
  errorScrollContent: {
    flexGrow: 1,
    alignItems: "center",
    justifyContent: "flex-start",
    paddingHorizontal: theme.spacing[8],
    paddingVertical: theme.spacing[8],
    paddingTop: theme.spacing[16],
  },
  errorContent: {
    alignItems: "stretch",
    maxWidth: 720,
    width: "100%",
    gap: theme.spacing[6],
  },
  errorHeader: {
    alignItems: "flex-start",
  },
  title: {
    color: theme.colors.foreground,
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.semibold,
    textAlign: "left",
  },
  errorDescription: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
    lineHeight: 22,
  },
  errorMessage: {
    color: theme.colors.destructive,
    fontSize: theme.fontSize.code,
    lineHeight: 20,
    fontFamily: theme.fontFamily.mono,
  },
  logsMeta: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
  logsContainer: {
    height: 200,
    borderRadius: theme.borderRadius.xl,
    backgroundColor: theme.colors.surface1,
    borderWidth: theme.borderWidth[1],
    borderColor: theme.colors.border,
    overflow: "hidden",
  },
  logsScroll: {
    flexGrow: 0,
  },
  logsContent: {
    padding: theme.spacing[4],
  },
  logsText: {
    fontFamily: theme.fontFamily.mono,
    fontSize: theme.fontSize.code,
    color: theme.colors.foreground,
    lineHeight: 18,
    ...(isWeb
      ? {
          whiteSpace: "pre",
          overflowWrap: "normal",
        }
      : null),
  },
  actionRow: {
    flexDirection: "row",
    gap: theme.spacing[3],
    flexWrap: "wrap",
  },
}));
