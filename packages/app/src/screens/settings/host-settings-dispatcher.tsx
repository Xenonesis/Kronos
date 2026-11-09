import type { ReactNode } from "react";
import type { SettingsView } from "@/navigation/settings-navigation";
import ProjectsScreen from "@/screens/projects-screen";
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
import { MetadataGenerationPage } from "@/screens/settings/metadata-generation-page";
import { HostVoicePage } from "@/screens/settings/voice/voice-page";
import { HostPluginsPage } from "@/screens/settings/plugins-page";

export function renderHostSettingsContent(
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
