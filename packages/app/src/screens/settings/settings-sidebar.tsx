import { useMemo } from "react";
import { ScrollView, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { useTranslation } from "react-i18next";
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
import { TerminalSection } from "@/screens/settings/terminal/terminal-section";
import { ChatSection } from "@/screens/settings/chat/chat-section";
import { SidebarNavSection } from "@/screens/settings/sidebar/sidebar-nav-section";
import { useHosts } from "@/runtime/host-runtime";
import { orderHostsLocalFirst, type HostProfile } from "@/types/host-connection";
import { TitlebarDragRegion } from "@/components/desktop/titlebar-drag-region";
import { WindowChromeSafeArea } from "@/utils/desktop-window";
import { KeyboardShortcutsSection } from "@/screens/settings/keyboard-shortcuts-section";
import { EditorSection } from "@/screens/settings/editor-section";
import { FilesSection } from "@/screens/settings/files-section";
import { DesktopPermissionsSection } from "@/desktop/components/desktop-permissions-section";
import { DesktopNotificationsSection } from "@/desktop/components/desktop-notifications-section";
import { BrowserDataSection } from "@/desktop/browser/settings/browser-data-section";
import { IntegrationsSection } from "@/desktop/components/integrations-section";
import { isElectronRuntime } from "@/desktop/host";
import {
  SettingsSidebarContent,
  type SidebarSectionItem,
  type HostSectionItem,
} from "@/screens/settings/settings-sidebar-content";
import { SETTINGS_DESKTOP_SIDEBAR_WIDTH } from "@/constants/layout";
import { densityIconSize } from "@/styles/density";
import { useLocalDaemonServerId } from "@/hooks/use-is-local-daemon";
import { useEnableBuiltInDaemonOption } from "@/desktop/hooks/use-enable-built-in-daemon-option";
import type { HostSectionSlug, SettingsSectionSlug } from "@/utils/host-routes";
import { useSettingsSidebarScroll } from "@/navigation/settings-presentation";
import type { SettingsView } from "@/navigation/settings-navigation";
import { isWeb } from "@/constants/platform";

export const SIDEBAR_SECTION_ITEMS: SidebarSectionItem[] = [
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

export function isSectionAvailable(item: SidebarSectionItem, isDesktopApp: boolean): boolean {
  return (!item.desktopOnly || isDesktopApp) && (!item.webOnly || isWeb);
}

export const HOST_SECTION_ITEMS: HostSectionItem[] = [
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

export function useSortedHosts(hosts: HostProfile[], localServerId: string | null): HostProfile[] {
  return useMemo(() => orderHostsLocalFirst(hosts, localServerId), [hosts, localServerId]);
}

const ThemedSettingsSidebarContent = withUnistyles(SettingsSidebarContent, (theme, rt) => ({
  iconSize: densityIconSize(rt, "md"),
  foreground: theme.colors.foreground,
  foregroundMuted: theme.colors.foregroundMuted,
}));

export interface SettingsSidebarProps {
  view: SettingsView;
  onSelectSection: (section: SettingsSectionSlug) => void;
  onSelectHostSection: (section: HostSectionSlug) => void;
  onSelectHost: (serverId: string) => void;
  onAddHost: () => void;
  onBackToWorkspace: () => void;
  activeHostServerId: string | null;
  layout: "desktop" | "mobile";
}

export function SettingsSidebar({
  view,
  onSelectSection,
  onSelectHostSection,
  onSelectHost,
  onAddHost,
  onBackToWorkspace,
  activeHostServerId,
  layout,
}: SettingsSidebarProps) {
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
    <ThemedSettingsSidebarContent
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

const sidebarStyles = StyleSheet.create((theme) => ({
  desktopContainer: {
    width: SETTINGS_DESKTOP_SIDEBAR_WIDTH,
    borderRightWidth: 1,
    borderRightColor: theme.colors.border,
    backgroundColor: theme.colors.surface0,
    flexShrink: 0,
  },
  mobileContainer: {
    flex: 1,
  },
  sidebarDragArea: {
    paddingHorizontal: 8,
  },
  scrollBody: {
    flex: 1,
  },
}));
