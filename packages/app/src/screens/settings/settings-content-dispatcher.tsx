import React, { memo, type ReactNode } from "react";
import type { SettingsView } from "@/navigation/settings-navigation";
import type { AppLanguage } from "@/i18n/locales";
import { PluginSettingsContent } from "@/plugins/settings";
import { LanguageSection } from "@/screens/settings/general/language-section";
import { OpenLocationSection } from "@/screens/settings/open-location/open-location-section";
import { SendingSection } from "@/screens/settings/general/sending-section";
import { DiagnosticsSection } from "@/screens/settings/diagnostics-section";
import { AboutSection } from "@/screens/settings/about-section";
import ProjectSettingsScreen from "@/screens/project-settings-screen";
import { isSectionAvailable, SIDEBAR_SECTION_ITEMS } from "@/screens/settings/settings-sidebar";
import { renderHostSettingsContent } from "@/screens/settings/host-settings-dispatcher";

interface SettingsContentDispatcherProps {
  view: SettingsView;
  isCompactLayout: boolean;
  isDesktopApp: boolean;
  handleBackFromDetail: () => void;
  handleHostRemoved: () => void;
  language: AppLanguage;
  onLanguageChange: (language: AppLanguage) => void;
  canTestPlayback: boolean;
  isPlaybackTestRunning: boolean;
  playbackTestResult: string | null;
  onPlaybackTest: () => Promise<void>;
  appVersion: string | null;
  appVersionText: string;
}

export const SettingsContentDispatcher = memo(function SettingsContentDispatcher({
  view,
  isCompactLayout,
  isDesktopApp,
  handleBackFromDetail,
  handleHostRemoved,
  language,
  onLanguageChange,
  canTestPlayback,
  isPlaybackTestRunning,
  playbackTestResult,
  onPlaybackTest,
  appVersion,
  appVersionText,
}: SettingsContentDispatcherProps): ReactNode {
  if (view.kind === "plugin") {
    return (
      <PluginSettingsContent
        serverId={view.serverId}
        pluginId={view.pluginId}
        screenId={view.screenId}
        onBackToPlugins={handleBackFromDetail}
        showBackToPlugins={!isCompactLayout}
      />
    );
  }

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
            <LanguageSection language={language} onLanguageChange={onLanguageChange} />
            <SendingSection />
            {isDesktopApp ? <OpenLocationSection /> : null}
          </>
        );
      case "diagnostics":
        return (
          <DiagnosticsSection
            canTestPlayback={canTestPlayback}
            isPlaybackTestRunning={isPlaybackTestRunning}
            playbackTestResult={playbackTestResult}
            onPlaybackTest={onPlaybackTest}
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
});
