import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { ScrollView, Text, View } from "react-native";
import { useRouter } from "expo-router";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { useTranslation } from "react-i18next";
import { Buffer } from "buffer";
import {
  ArrowLeft,
  Settings,
  Palette,
  Server,
  Network,
  Bot,
  Boxes,
  Gauge,
  Keyboard,
  Stethoscope,
  Info,
  Bell,
  Shield,
  Puzzle,
  FolderGit2,
  SquareTerminal,
  Code2,
  Files,
  Smartphone,
  Sparkles,
  Mic,
  Blocks,
  Globe,
  PanelLeft,
  MessageSquare,
} from "lucide-react-native";
import { SidebarHeaderRow } from "@/components/sidebar/sidebar-header-row";
import { AppearanceSection } from "@/screens/settings/appearance/appearance-section";
import { OpenLocationSection } from "@/screens/settings/open-location/open-location-section";
import { TerminalSection } from "@/screens/settings/terminal/terminal-section";
import { ChatSection } from "@/screens/settings/chat/chat-section";
import { SidebarNavSection } from "@/screens/settings/sidebar/sidebar-nav-section";
import { SendingSection } from "@/screens/settings/general/sending-section";
import { useAppSettings } from "@/hooks/use-settings";
import { useHosts } from "@/runtime/host-runtime";
import {
  orderHostsLocalFirst,
  resolveActiveHostServerId,
  type HostProfile,
} from "@/types/host-connection";
import { TitlebarDragRegion } from "@/components/desktop/titlebar-drag-region";
import { WindowChromeRegion, WindowChromeSafeArea } from "@/utils/desktop-window";
import { BackHeader } from "@/components/headers/back-header";
import { PageLayout } from "@/components/page-layout";
import { AddHostMethodModal } from "@/components/add-host-method-modal";
import { AddHostModal } from "@/components/add-host-modal";
import { AddRemoteSshHostModal } from "@/components/add-remote-ssh-host-modal";
import { PairLinkModal } from "@/components/pair-link-modal";
import { KeyboardShortcutsSection } from "@/screens/settings/keyboard-shortcuts-section";
import { EditorSection } from "@/screens/settings/editor-section";
import { FilesSection } from "@/screens/settings/files-section";
import { DesktopPermissionsSection } from "@/desktop/components/desktop-permissions-section";
import { DesktopNotificationsSection } from "@/desktop/components/desktop-notifications-section";
import { BrowserDataSection } from "@/desktop/browser/settings/browser-data-section";
import { IntegrationsSection } from "@/desktop/components/integrations-section";
import { isElectronRuntime } from "@/desktop/host";
import { formatVersionWithPrefix } from "@/desktop/updates/desktop-updates";
import { resolveAppVersion } from "@/utils/app-version";
import { THINKING_TONE_NATIVE_PCM_BASE64 } from "@/utils/thinking-tone.native-pcm";
import { useVoiceAudioEngineOptional } from "@/contexts/voice-context";
import type { AppLanguage } from "@/i18n/locales";
import {
  HostConnectionsPage,
  HostPairDevicePage,
  HostAgentsPage,
  HostSettingsPage,
  HostProvidersPage,
  HostUsagePage,
  HostWorkspacesPage,
  HostTerminalsPage,
} from "@/screens/settings/host-page";
import { PluginSettingsContent } from "@/plugins/settings";
import { useInstalledPlugins } from "@/plugins/registry";
import { HostPluginsPage } from "@/screens/settings/plugins-page";
import { LanguageSection } from "@/screens/settings/general/language-section";
import { DiagnosticsSection } from "@/screens/settings/diagnostics-section";
import { AboutSection } from "@/screens/settings/about-section";
import {
  SettingsSidebarContent,
  type SidebarSectionItem,
  type HostSectionItem,
} from "@/screens/settings/settings-sidebar-content";
import { MetadataGenerationPage } from "@/screens/settings/metadata-generation-page";
import { HostVoicePage } from "@/screens/settings/voice/voice-page";
import ProjectsScreen from "@/screens/projects-screen";
import ProjectSettingsScreen from "@/screens/project-settings-screen";
import {
  SETTINGS_DESKTOP_DETAIL_MIN_WIDTH,
  SETTINGS_DESKTOP_SIDEBAR_WIDTH,
  useIsCompactFormFactor,
} from "@/constants/layout";
import { densityIconSize } from "@/styles/density";
import { useLocalDaemonServerId } from "@/hooks/use-is-local-daemon";
import { useEnableBuiltInDaemonOption } from "@/desktop/hooks/use-enable-built-in-daemon-option";
import {
  buildSettingsHostSectionRoute,
  buildSettingsSectionRoute,
  type HostSectionSlug,
  type SettingsSectionSlug,
} from "@/utils/host-routes";
import {
  selectSettingsHost,
  useSettingsHostSelection,
  useSettingsSidebarScroll,
} from "@/navigation/settings-presentation";
import { returnFromSettings, type SettingsView } from "@/navigation/settings-navigation";
import { isWeb } from "@/constants/platform";

const SIDEBAR_SECTION_ITEMS: SidebarSectionItem[] = [
  { id: "general", labelKey: "settings.sections.general", icon: Settings },
  {
    id: "appearance",
    labelKey: "settings.sections.appearance",
    icon: Palette,
    Content: AppearanceSection,
  },
  {
    id: "sidebar",
    labelKey: "settings.sections.sidebar",
    icon: PanelLeft,
    Content: SidebarNavSection,
  },
  { id: "chat", labelKey: "settings.sections.chat", icon: MessageSquare, Content: ChatSection },
  {
    id: "terminal",
    labelKey: "settings.sections.terminal",
    icon: SquareTerminal,
    Content: TerminalSection,
  },
  {
    id: "browser",
    labelKey: "settings.sections.browser",
    icon: Globe,
    desktopOnly: true,
    Content: BrowserDataSection,
  },
  {
    id: "editor",
    labelKey: "settings.sections.editor",
    icon: Code2,
    webOnly: true,
    Content: EditorSection,
  },
  { id: "files", labelKey: "settings.sections.files", icon: Files, Content: FilesSection },
  {
    id: "shortcuts",
    labelKey: "settings.sections.shortcuts",
    icon: Keyboard,
    desktopOnly: true,
    Content: KeyboardShortcutsSection,
  },
  {
    id: "integrations",
    labelKey: "settings.sections.integrations",
    icon: Puzzle,
    desktopOnly: true,
    Content: IntegrationsSection,
  },
  {
    id: "notifications",
    labelKey: "settings.sections.notifications",
    icon: Bell,
    desktopOnly: true,
    Content: DesktopNotificationsSection,
  },
  {
    id: "permissions",
    labelKey: "settings.sections.permissions",
    icon: Shield,
    desktopOnly: true,
    Content: DesktopPermissionsSection,
  },
  { id: "diagnostics", labelKey: "settings.sections.diagnostics", icon: Stethoscope },
  { id: "about", labelKey: "settings.sections.about", icon: Info },
];

function isSectionAvailable(item: SidebarSectionItem, isDesktopApp: boolean): boolean {
  return (!item.desktopOnly || isDesktopApp) && (!item.webOnly || isWeb);
}

const HOST_SECTION_ITEMS: HostSectionItem[] = [
  { id: "host", labelKey: "settings.hostSections.host", icon: Server },
  { id: "projects", labelKey: "settings.hostSections.projects", icon: FolderGit2 },
  { id: "connections", labelKey: "settings.hostSections.connections", icon: Network },
  { id: "pair-device", labelKey: "openProject.tiles.pairDevice.title", icon: Smartphone },
  { id: "agents", labelKey: "settings.hostSections.agents", icon: Bot },
  { id: "metadata", labelKey: "settings.hostSections.metadata", icon: Sparkles },
  { id: "voice", labelKey: "settings.hostSections.voice", icon: Mic },
  { id: "workspaces", labelKey: "settings.hostSections.workspaces", icon: FolderGit2 },
  { id: "providers", labelKey: "settings.hostSections.providers", icon: Boxes },
  { id: "usage", labelKey: "settings.hostSections.usage", icon: Gauge },
  { id: "terminals", labelKey: "settings.hostSections.terminals", icon: SquareTerminal },
  { id: "plugins", labelKey: "settings.hostSections.plugins", icon: Blocks },
];

function renderHostSettingsContent(
  view: Extract<SettingsView, { kind: "host" }>,
  onHostRemoved: () => void,
): ReactNode {
  switch (view.section) {
    case "projects":
      return <ProjectsScreen serverId={view.serverId} />;
    case "connections":
      return <HostConnectionsPage serverId={view.serverId} />;
    case "pair-device":
      return <HostPairDevicePage serverId={view.serverId} />;
    case "agents":
      return <HostAgentsPage serverId={view.serverId} />;
    case "metadata":
      return <MetadataGenerationPage serverId={view.serverId} />;
    case "voice":
      return <HostVoicePage serverId={view.serverId} />;
    case "workspaces":
      return <HostWorkspacesPage serverId={view.serverId} />;
    case "providers":
      return <HostProvidersPage serverId={view.serverId} />;
    case "usage":
      return <HostUsagePage serverId={view.serverId} />;
    case "terminals":
      return <HostTerminalsPage serverId={view.serverId} />;
    case "plugins":
      return <HostPluginsPage serverId={view.serverId} />;
    case "host":
      return <HostSettingsPage serverId={view.serverId} onHostRemoved={onHostRemoved} />;
  }
}

/**
 * Local daemon first, then remaining hosts in their existing order.
 */
function useSortedHosts(hosts: HostProfile[], localServerId: string | null): HostProfile[] {
  return useMemo(() => orderHostsLocalFirst(hosts, localServerId), [hosts, localServerId]);
}

interface SettingsSidebarProps {
  view: SettingsView;
  onSelectSection: (section: SettingsSectionSlug) => void;
  onSelectHostSection: (section: HostSectionSlug) => void;
  onSelectHost: (serverId: string) => void;
  onAddHost: () => void;
  onBackToWorkspace: () => void;
  activeHostServerId: string | null;
  layout: "desktop" | "mobile";
}

function SettingsSidebar({
  view,
  onSelectSection,
  onSelectHostSection,
  onSelectHost,
  onAddHost,
  onBackToWorkspace,
  activeHostServerId,
  layout,
}: SettingsSidebarProps) {
  const { theme, rt } = useUnistyles();
  const { t } = useTranslation();
  const hosts = useHosts();
  const localServerId = useLocalDaemonServerId();
  const sortedHosts = useSortedHosts(hosts, localServerId);
  const enableBuiltInDaemonOption = useEnableBuiltInDaemonOption();
  const isDesktopApp = isElectronRuntime();
  const items = SIDEBAR_SECTION_ITEMS.filter((item) => isSectionAvailable(item, isDesktopApp));
  const insets = useSafeAreaInsets();
  const isDesktop = layout === "desktop";
  const sidebarScroll = useSettingsSidebarScroll(isDesktop);
  const outerContainerStyle = useMemo(
    () => [isDesktop ? sidebarStyles.desktopContainer : sidebarStyles.mobileContainer],
    [isDesktop],
  );
  const innerContainerStyle = useMemo(
    () => [{ flex: 1 }, isDesktop ? { paddingTop: insets.top } : null],
    [insets.top, isDesktop],
  );
  const selectedSectionId = view.kind === "section" ? view.section : null;
  let selectedHostSection: HostSectionSlug | null = null;
  if (view.kind === "host") selectedHostSection = view.section;
  if (view.kind === "project") selectedHostSection = "projects";
  if (view.kind === "plugin") selectedHostSection = "plugins";

  const sidebarBody = (
    <SettingsSidebarContent
      items={items}
      hostItems={HOST_SECTION_ITEMS}
      selectedSectionId={selectedSectionId}
      selectedHostSection={selectedHostSection}
      activeHostServerId={activeHostServerId}
      sortedHosts={sortedHosts}
      enableBuiltInDaemonOption={enableBuiltInDaemonOption}
      onSelectSection={onSelectSection}
      onSelectHostSection={onSelectHostSection}
      onSelectHost={onSelectHost}
      onAddHost={onAddHost}
      iconSize={densityIconSize(rt, "md")}
      foreground={theme.colors.foreground}
      foregroundMuted={theme.colors.foregroundMuted}
    />
  );

  return (
    <View
      accessibilityLabel={t("settings.title")}
      role="navigation"
      style={outerContainerStyle}
      testID="settings-sidebar"
    >
      {isDesktop ? (
        <View style={innerContainerStyle}>
          <View style={sidebarStyles.sidebarDragArea}>
            <TitlebarDragRegion />
            <WindowChromeSafeArea placement="below" />
            <SidebarHeaderRow
              icon={ArrowLeft}
              label={t("settings.backToWorkspace")}
              onPress={onBackToWorkspace}
              testID="settings-back-to-workspace"
            />
          </View>
          <ScrollView
            style={sidebarStyles.scrollBody}
            showsVerticalScrollIndicator={false}
            testID="settings-sidebar-scroll-body"
            {...sidebarScroll}
          >
            {sidebarBody}
          </ScrollView>
        </View>
      ) : (
        sidebarBody
      )}
    </View>
  );
}

export interface SettingsScreenProps {
  view: SettingsView;
  openAddHostIntent?: string | null;
}

export default function SettingsScreen({ view, openAddHostIntent = null }: SettingsScreenProps) {
  const router = useRouter();
  const { t } = useTranslation();
  const voiceAudioEngine = useVoiceAudioEngineOptional();
  const { settings, isLoading: settingsLoading, updateSettings } = useAppSettings();
  const [isAddHostMethodVisible, setIsAddHostMethodVisible] = useState(false);
  const [isDirectHostVisible, setIsDirectHostVisible] = useState(false);
  const [isRemoteSshVisible, setIsRemoteSshVisible] = useState(false);
  const [isPasteLinkVisible, setIsPasteLinkVisible] = useState(false);
  const [isPlaybackTestRunning, setIsPlaybackTestRunning] = useState(false);
  const [playbackTestResult, setPlaybackTestResult] = useState<string | null>(null);
  const lastOpenedAddHostIntentRef = useRef<string | null>(null);
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

  const handlePlaybackTest = useCallback(async () => {
    if (!voiceAudioEngine || isPlaybackTestRunning) {
      return;
    }

    setIsPlaybackTestRunning(true);
    setPlaybackTestResult(null);

    try {
      const bytes = Buffer.from(THINKING_TONE_NATIVE_PCM_BASE64, "base64");
      await voiceAudioEngine.initialize();
      voiceAudioEngine.stop();
      await voiceAudioEngine.play({
        type: "audio/pcm;rate=16000;bits=16",
        size: bytes.byteLength,
        async arrayBuffer() {
          return Uint8Array.from(bytes).buffer;
        },
      });
      setPlaybackTestResult(null);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[Settings] Playback test failed", error);
      setPlaybackTestResult(t("settings.diagnostics.playbackFailed", { message }));
    } finally {
      setIsPlaybackTestRunning(false);
    }
  }, [isPlaybackTestRunning, t, voiceAudioEngine]);

  const closeAddConnectionFlow = useCallback(() => {
    setIsAddHostMethodVisible(false);
    setIsDirectHostVisible(false);
    setIsRemoteSshVisible(false);
    setIsPasteLinkVisible(false);
  }, []);

  const goBackToAddConnectionMethods = useCallback(() => {
    setIsDirectHostVisible(false);
    setIsRemoteSshVisible(false);
    setIsPasteLinkVisible(false);
    setIsAddHostMethodVisible(true);
  }, []);

  const handleAddHost = useCallback(() => {
    setIsAddHostMethodVisible(true);
  }, []);

  useEffect(() => {
    if (!openAddHostIntent || lastOpenedAddHostIntentRef.current === openAddHostIntent) {
      return;
    }
    lastOpenedAddHostIntentRef.current = openAddHostIntent;
    handleAddHost();
  }, [handleAddHost, openAddHostIntent]);

  const handleSelectDirectConnection = useCallback(() => {
    setIsAddHostMethodVisible(false);
    setIsDirectHostVisible(true);
  }, []);

  const handleSelectRemoteSsh = useCallback(() => {
    setIsAddHostMethodVisible(false);
    setIsRemoteSshVisible(true);
  }, []);

  const handleSelectPasteLink = useCallback(() => {
    setIsAddHostMethodVisible(false);
    setIsPasteLinkVisible(true);
  }, []);

  const handleHostAdded = useCallback(
    ({ serverId }: { serverId: string }) => {
      const target = buildSettingsHostSectionRoute(serverId, "connections");
      if (isCompactLayout) {
        router.push(target);
      } else {
        router.replace(target);
      }
    },
    [isCompactLayout, router],
  );

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

  const handleScanQr = useCallback(() => {
    closeAddConnectionFlow();
    router.push({
      pathname: "/pair-scan",
      params: { source: "settings" },
    });
  }, [closeAddConnectionFlow, router]);

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

  const content: ReactNode = (() => {
    if (view.kind === "plugin")
      return (
        <PluginSettingsContent
          serverId={view.serverId}
          pluginId={view.pluginId}
          screenId={view.screenId}
          onBackToPlugins={handleBackFromDetail}
          showBackToPlugins={!isCompactLayout}
        />
      );
    if (view.kind === "host") {
      return renderHostSettingsContent(view, handleHostRemoved);
    }
    if (view.kind === "project") {
      return (
        <ProjectSettingsScreen
          serverId={view.serverId}
          projectId={view.projectId}
          onBackToProjects={handleBackFromDetail}
          showBackToProjects={!isCompactLayout}
        />
      );
    }
    if (view.kind === "section") {
      const item = SIDEBAR_SECTION_ITEMS.find((candidate) => candidate.id === view.section);
      if (!item || !isSectionAvailable(item, isDesktopApp)) return null;
      switch (view.section) {
        case "general":
          return (
            <>
              <LanguageSection
                language={settings.language}
                onLanguageChange={handleLanguageChange}
              />
              <SendingSection />
              {isDesktopApp ? <OpenLocationSection /> : null}
            </>
          );
        case "diagnostics":
          return (
            <DiagnosticsSection
              canTestPlayback={Boolean(voiceAudioEngine)}
              isPlaybackTestRunning={isPlaybackTestRunning}
              playbackTestResult={playbackTestResult}
              onPlaybackTest={handlePlaybackTest}
            />
          );
        case "about":
          return (
            <AboutSection
              appVersion={appVersion}
              appVersionText={appVersionText}
              isDesktopApp={isDesktopApp}
            />
          );
        default:
          return item.Content ? <item.Content /> : null;
      }
    }
    return null;
  })();

  if (settingsLoading) {
    return (
      <View style={styles.loadingContainer}>
        <Text style={styles.loadingText}>{t("settings.loading")}</Text>
      </View>
    );
  }

  const addHostModals = (
    <>
      <AddHostMethodModal
        visible={isAddHostMethodVisible}
        onClose={closeAddConnectionFlow}
        onDirectConnection={handleSelectDirectConnection}
        onRemoteSsh={handleSelectRemoteSsh}
        onPasteLink={handleSelectPasteLink}
        onScanQr={handleScanQr}
      />
      <AddHostModal
        visible={isDirectHostVisible}
        onClose={closeAddConnectionFlow}
        onCancel={goBackToAddConnectionMethods}
        onSaved={handleHostAdded}
      />
      <AddRemoteSshHostModal
        visible={isRemoteSshVisible}
        onClose={closeAddConnectionFlow}
        onCancel={goBackToAddConnectionMethods}
        onSaved={handleHostAdded}
      />
      <PairLinkModal
        visible={isPasteLinkVisible}
        onClose={closeAddConnectionFlow}
        onCancel={goBackToAddConnectionMethods}
        onSaved={handleHostAdded}
      />
    </>
  );

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
        {addHostModals}
      </View>
    );
  }

  if (isCompactLayout) {
    return (
      <View style={styles.container}>
        <PageLayout title={detailTitle} onBack={handleBackFromDetail}>
          {content}
        </PageLayout>
        {addHostModals}
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
      {addHostModals}
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

const sidebarStyles = StyleSheet.create((theme) => ({
  desktopContainer: {
    width: SETTINGS_DESKTOP_SIDEBAR_WIDTH,
    flexShrink: 1,
    borderRightWidth: 1,
    borderRightColor: theme.colors.border,
    backgroundColor: theme.colors.surfaceSidebar,
  },
  scrollBody: {
    flex: 1,
  },
  sidebarDragArea: {
    position: "relative",
  },
  mobileContainer: {
    paddingVertical: theme.spacing[2],
    paddingHorizontal: theme.spacing[2],
  },
}));
