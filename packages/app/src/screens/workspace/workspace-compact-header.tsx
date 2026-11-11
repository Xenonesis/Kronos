import { useMemo, type ComponentProps } from "react";
import { CompactWorkspaceHeader } from "@/screens/workspace/compact-workspace-header";
import { MobileWorkspaceTabSwitcher } from "@/screens/workspace/compact-workspace-header/tab-switcher";
import {
  WorkspaceHeaderMenuMobile,
  type WorkspaceHeaderMenuMobileProps,
} from "@/screens/workspace/workspace-header-menu";

export interface WorkspaceCompactHeaderProps
  extends
    Pick<
      ComponentProps<typeof CompactWorkspaceHeader>,
      | "isLoading"
      | "title"
      | "subtitle"
      | "isSubtitleDistinct"
      | "onToggleExplorer"
      | "explorerToggleLabel"
      | "explorerToggleAccessibilityState"
    >,
    Pick<
      ComponentProps<typeof MobileWorkspaceTabSwitcher>,
      | "tabs"
      | "activeTabKey"
      | "onSelectSwitcherTab"
      | "onCreateDraftTab"
      | "onCreateTerminal"
      | "onCopyResumeCommand"
      | "onCopyAgentId"
      | "onCopyTerminalId"
      | "onCopyFilePath"
      | "onReloadAgent"
      | "onRenameTab"
      | "onCloseTab"
      | "onCloseTabsAbove"
      | "onCloseTabsBelow"
      | "onCloseOtherTabs"
    >,
    Pick<
      WorkspaceHeaderMenuMobileProps,
      | "normalizedServerId"
      | "normalizedWorkspaceId"
      | "scripts"
      | "currentBranchName"
      | "showWorkspaceSetup"
      | "showCreateBrowserTab"
      | "createTerminalDisabled"
      | "importAgentDisabled"
      | "copyPathDisabled"
      | "onCreateTerminalWithProfile"
      | "onCreateBrowser"
      | "onOpenImportSheet"
      | "onCopyWorkspacePath"
      | "onCopyBranchName"
      | "onOpenSetupTab"
    > {}

export function WorkspaceCompactHeader({
  isLoading,
  title,
  subtitle,
  isSubtitleDistinct,
  normalizedServerId,
  normalizedWorkspaceId,
  onToggleExplorer,
  explorerToggleLabel,
  explorerToggleAccessibilityState,
  tabs,
  activeTabKey,
  onSelectSwitcherTab,
  createTerminalDisabled,
  onCreateDraftTab,
  onCreateTerminal,
  onCopyResumeCommand,
  onCopyAgentId,
  onCopyTerminalId,
  onCopyFilePath,
  onReloadAgent,
  onRenameTab,
  onCloseTab,
  onCloseTabsAbove,
  onCloseTabsBelow,
  onCloseOtherTabs,
  scripts,
  currentBranchName,
  showWorkspaceSetup,
  showCreateBrowserTab,
  importAgentDisabled,
  copyPathDisabled,
  onCreateTerminalWithProfile,
  onCreateBrowser,
  onOpenImportSheet,
  onCopyWorkspacePath,
  onCopyBranchName,
  onOpenSetupTab,
}: WorkspaceCompactHeaderProps) {
  const switcher = useMemo(
    () => (
      <MobileWorkspaceTabSwitcher
        tabs={tabs}
        activeTabKey={activeTabKey}
        normalizedServerId={normalizedServerId}
        normalizedWorkspaceId={normalizedWorkspaceId}
        onSelectSwitcherTab={onSelectSwitcherTab}
        createTerminalDisabled={createTerminalDisabled}
        onCreateDraftTab={onCreateDraftTab}
        onCreateTerminal={onCreateTerminal}
        onCopyResumeCommand={onCopyResumeCommand}
        onCopyAgentId={onCopyAgentId}
        onCopyTerminalId={onCopyTerminalId}
        onCopyFilePath={onCopyFilePath}
        onReloadAgent={onReloadAgent}
        onRenameTab={onRenameTab}
        onCloseTab={onCloseTab}
        onCloseTabsAbove={onCloseTabsAbove}
        onCloseTabsBelow={onCloseTabsBelow}
        onCloseOtherTabs={onCloseOtherTabs}
      />
    ),
    [
      tabs,
      activeTabKey,
      normalizedServerId,
      normalizedWorkspaceId,
      onSelectSwitcherTab,
      createTerminalDisabled,
      onCreateDraftTab,
      onCreateTerminal,
      onCopyResumeCommand,
      onCopyAgentId,
      onCopyTerminalId,
      onCopyFilePath,
      onReloadAgent,
      onRenameTab,
      onCloseTab,
      onCloseTabsAbove,
      onCloseTabsBelow,
      onCloseOtherTabs,
    ],
  );

  return (
    <CompactWorkspaceHeader
      isLoading={isLoading}
      title={title}
      subtitle={subtitle}
      isSubtitleDistinct={isSubtitleDistinct}
      serverId={normalizedServerId}
      onToggleExplorer={onToggleExplorer}
      explorerToggleLabel={explorerToggleLabel}
      explorerToggleAccessibilityState={explorerToggleAccessibilityState}
      tabSwitcher={switcher}
      headerVariant="compact"
      actionsActive={false}
    >
      <WorkspaceHeaderMenuMobile
        normalizedServerId={normalizedServerId}
        normalizedWorkspaceId={normalizedWorkspaceId}
        scripts={scripts}
        currentBranchName={currentBranchName}
        showWorkspaceSetup={showWorkspaceSetup}
        showCreateBrowserTab={showCreateBrowserTab}
        createTerminalDisabled={createTerminalDisabled}
        importAgentDisabled={importAgentDisabled}
        copyPathDisabled={copyPathDisabled}
        onCreateTerminalWithProfile={onCreateTerminalWithProfile}
        onCreateBrowser={onCreateBrowser}
        onOpenImportSheet={onOpenImportSheet}
        onCopyWorkspacePath={onCopyWorkspacePath}
        onCopyBranchName={onCopyBranchName}
        onOpenSetupTab={onOpenSetupTab}
      />
    </CompactWorkspaceHeader>
  );
}
