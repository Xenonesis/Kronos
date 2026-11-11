import { useCallback, useMemo } from "react";
import { ScrollView, Text, View } from "react-native";
import { useRouter } from "expo-router";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { StyleSheet } from "react-native-unistyles";
import { useTranslation } from "react-i18next";
import { useAppSettings } from "@/hooks/use-settings";
import { useHosts } from "@/runtime/host-runtime";
import { resolveActiveHostServerId } from "@/types/host-connection";
import { WindowChromeRegion } from "@/utils/desktop-window";
import { BackHeader } from "@/components/headers/back-header";
import { PageLayout } from "@/components/page-layout";
import { isElectronRuntime } from "@/desktop/host";
import { formatVersionWithPrefix } from "@/desktop/updates/desktop-updates";
import { resolveAppVersion } from "@/utils/app-version";
import type { AppLanguage } from "@/i18n/locales";
import { useInstalledPlugins } from "@/plugins/registry";
import { SETTINGS_DESKTOP_DETAIL_MIN_WIDTH, useIsCompactFormFactor } from "@/constants/layout";
import { useLocalDaemonServerId } from "@/hooks/use-is-local-daemon";
import {
  buildSettingsHostSectionRoute,
  buildSettingsSectionRoute,
  type HostSectionSlug,
  type SettingsSectionSlug,
} from "@/utils/host-routes";
import { selectSettingsHost, useSettingsHostSelection } from "@/navigation/settings-presentation";
import { returnFromSettings, type SettingsView } from "@/navigation/settings-navigation";
import {
  HOST_SECTION_ITEMS,
  SIDEBAR_SECTION_ITEMS,
  SettingsSidebar,
  useSortedHosts,
} from "@/screens/settings/settings-sidebar";
import { useAddHost } from "@/hosts/add-host";
import { useAudioPlaybackTest } from "@/screens/settings/use-audio-playback-test";
import { SettingsContentDispatcher } from "@/screens/settings/settings-content-dispatcher";

export interface SettingsScreenProps {
  view: SettingsView;
}

export default function SettingsScreen({ view }: SettingsScreenProps) {
  const router = useRouter();
  const { t } = useTranslation();
  const { settings, isLoading: settingsLoading, updateSettings } = useAppSettings();
  const { canTestPlayback, isPlaybackTestRunning, playbackTestResult, handlePlaybackTest } =
    useAudioPlaybackTest(t);
  const isDesktopApp = isElectronRuntime();
  const appVersion = resolveAppVersion();
  const appVersionText = formatVersionWithPrefix(appVersion);
  const isCompactLayout = useIsCompactFormFactor();
  const insets = useSafeAreaInsets();
  const insetBottomStyle = useMemo(() => ({ paddingBottom: insets.bottom }), [insets.bottom]);
  const hosts = useHosts();
  const localServerId = useLocalDaemonServerId();
  const sortedHosts = useSortedHosts(hosts, localServerId);
  const selectedSettingsHostServerId = useSettingsHostSelection();

  // The host the four sections scope to: the host on the active view, otherwise
  // the picker choice, otherwise the connected local daemon, otherwise the first host.
  const activeHostServerId = useMemo(() => {
    if (view.kind === "host" || view.kind === "project" || view.kind === "plugin")
      return view.serverId;
    return resolveActiveHostServerId({
      selectedServerId: selectedSettingsHostServerId,
      localServerId,
      hosts,
      orderedHosts: sortedHosts,
    });
  }, [view, selectedSettingsHostServerId, localServerId, hosts, sortedHosts]);

  const handleLanguageChange = useCallback(
    (language: AppLanguage) => {
      void updateSettings({ language });
    },
    [updateSettings],
  );

  const handleHostAdded = useCallback(
    (serverId: string) => {
      selectSettingsHost(serverId);
      const target = buildSettingsHostSectionRoute(serverId, "host");
      if (isCompactLayout) {
        router.push(target);
      } else {
        router.replace(target);
      }
    },
    [isCompactLayout, router],
  );

  const handleAddHost = useAddHost({ onAdded: handleHostAdded });

  const handleSelectSection = useCallback(
    (section: SettingsSectionSlug) => {
      const target = buildSettingsSectionRoute(section);
      if (isCompactLayout) {
        router.push(target);
      } else {
        router.replace(target);
      }
    },
    [isCompactLayout, router],
  );

  // Picker: choose the host for host-section rows. If the user is already on a
  // host detail route, keep that detail section and swap only the host segment.
  const handleSelectHost = useCallback(
    (serverId: string) => {
      selectSettingsHost(serverId);
      if (view.kind === "project") {
        const target = buildSettingsHostSectionRoute(serverId, "projects");
        if (isCompactLayout) {
          router.push(target);
        } else {
          router.replace(target);
        }
        return;
      }
      if (view.kind !== "host") {
        return;
      }
      const target = buildSettingsHostSectionRoute(serverId, view.section);
      if (isCompactLayout) {
        router.push(target);
      } else {
        router.replace(target);
      }
    },
    [isCompactLayout, router, view],
  );

  const handleSelectHostSection = useCallback(
    (section: HostSectionSlug) => {
      if (!activeHostServerId) {
        handleAddHost();
        return;
      }
      const target = buildSettingsHostSectionRoute(activeHostServerId, section);
      if (isCompactLayout) {
        router.push(target);
      } else {
        router.replace(target);
      }
    },
    [activeHostServerId, handleAddHost, isCompactLayout, router],
  );

  const handleHostRemoved = useCallback(() => {
    const fallback = buildSettingsSectionRoute("general");
    if (isCompactLayout) {
      router.replace("/settings");
    } else {
      router.replace(fallback);
    }
  }, [isCompactLayout, router]);

  const handleBackFromDetail = useCallback(() => {
    returnFromSettings(view);
  }, [view]);

  const handleBackToWorkspace = useCallback(() => {
    returnFromSettings({ kind: "root" });
  }, []);

  const installedPlugins = useInstalledPlugins();
  const detailTitle = ((): string | undefined => {
    if (view.kind === "plugin") {
      const plugin = installedPlugins.find(
        (candidate) => candidate.serverId === view.serverId && candidate.id === view.pluginId,
      );
      const screen = plugin?.settingsScreens.find((candidate) => candidate.id === view.screenId);
      return `${view.pluginId} · ${plugin && screen ? plugin.i18n.resolve(screen.title, plugin.locale) : t("settings.title")}`;
    }
    if (view.kind === "host") {
      const item = HOST_SECTION_ITEMS.find((s) => s.id === view.section);
      return item ? t(item.labelKey) : undefined;
    }
    if (view.kind === "section") {
      const item = SIDEBAR_SECTION_ITEMS.find((s) => s.id === view.section);
      return item ? t(item.labelKey) : undefined;
    }
    if (view.kind === "project") {
      return t("settings.projects");
    }
    return undefined;
  })();

  const content = (
    <SettingsContentDispatcher
      view={view}
      isCompactLayout={isCompactLayout}
      isDesktopApp={isDesktopApp}
      handleBackFromDetail={handleBackFromDetail}
      handleHostRemoved={handleHostRemoved}
      language={settings.language}
      onLanguageChange={handleLanguageChange}
      canTestPlayback={canTestPlayback}
      isPlaybackTestRunning={isPlaybackTestRunning}
      playbackTestResult={playbackTestResult}
      onPlaybackTest={handlePlaybackTest}
      appVersion={appVersion}
      appVersionText={appVersionText}
    />
  );

  if (settingsLoading) {
    return (
      <View style={styles.loadingContainer}>
        <Text style={styles.loadingText}>{t("settings.loading")}</Text>
      </View>
    );
  }

  // Mobile root: full-screen sidebar-as-list.
  if (isCompactLayout && view.kind === "root") {
    return (
      <View style={styles.container}>
        <BackHeader title={t("settings.title")} onBack={handleBackToWorkspace} />
        <ScrollView style={styles.scrollView} contentContainerStyle={insetBottomStyle}>
          <SettingsSidebar
            view={view}
            onSelectSection={handleSelectSection}
            onSelectHostSection={handleSelectHostSection}
            onSelectHost={handleSelectHost}
            onAddHost={handleAddHost}
            onBackToWorkspace={handleBackToWorkspace}
            activeHostServerId={activeHostServerId}
            layout="mobile"
          />
        </ScrollView>
      </View>
    );
  }

  if (isCompactLayout) {
    return (
      <View style={styles.container}>
        <PageLayout title={detailTitle} onBack={handleBackFromDetail}>
          {content}
        </PageLayout>
      </View>
    );
  }

  // Desktop split view — mirrors AppContainer: sidebar owns the titlebar drag
  // region + traffic-light padding; detail pane renders whatever header the
  // selected section provides.
  return (
    <View style={styles.container}>
      <View style={desktopStyles.row}>
        <WindowChromeRegion corners="top-left">
          <SettingsSidebar
            view={view}
            onSelectSection={handleSelectSection}
            onSelectHostSection={handleSelectHostSection}
            onSelectHost={handleSelectHost}
            onAddHost={handleAddHost}
            onBackToWorkspace={handleBackToWorkspace}
            activeHostServerId={activeHostServerId}
            layout="desktop"
          />
        </WindowChromeRegion>
        <WindowChromeRegion corners="top-right">
          <View style={desktopStyles.contentPane} testID="settings-detail-pane">
            <PageLayout
              title={detailTitle}
              titleTestID="settings-detail-header-title"
              testID="settings-detail-scroll-body"
            >
              {content}
            </PageLayout>
          </View>
        </WindowChromeRegion>
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  loadingContainer: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
    alignItems: "center",
    justifyContent: "center",
  },
  loadingText: {
    color: theme.colors.foreground,
    fontSize: theme.fontSize.base,
  },
  container: {
    flex: 1,
    backgroundColor: theme.colors.surface0,
  },
  scrollView: {
    flex: 1,
  },
}));

const desktopStyles = StyleSheet.create({
  row: {
    flex: 1,
    flexDirection: "row",
  },
  contentPane: {
    flex: 1,
    minWidth: SETTINGS_DESKTOP_DETAIL_MIN_WIDTH,
  },
});
