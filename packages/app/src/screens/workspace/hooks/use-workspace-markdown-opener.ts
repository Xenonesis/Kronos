import { useEffect, useMemo } from "react";
import type { DaemonClient } from "@getpaseo/client/internal/daemon-client";
import { useStableEvent } from "@/hooks/use-stable-event";
import { createInlinePathOpener, type InlinePathTarget } from "@/assistant-file-links";
import { useOpenOutsideWorkspaceFolder } from "@/workspace/open-in-file-manager/outside-folder";
import type { OpenFileDisposition, WorkspaceFileLocation } from "@/workspace/file-open";
import type { ExplorerSidebarView } from "@/workspace-tabs/explorer-sidebar";

export interface UseWorkspaceMarkdownOpenerInput {
  isRouteFocused: boolean;
  activeTabId: string | null;
  workspaceDirectory: string | null;
  client: DaemonClient | null;
  normalizedServerId: string;
  persistenceKey: string;
  isMobile: boolean;
  handleOpenPreferredAssistantFile: (input: {
    location: WorkspaceFileLocation;
    parentTabId: string | null;
  }) => void;
  revealDirectory: (directoryPath: string) => Promise<unknown> | void;
  openExplorerSidebarView: (input: {
    isCompact: boolean;
    workspaceKey: string;
    view: ExplorerSidebarView;
  }) => void;
}

export function useWorkspaceMarkdownOpener({
  isRouteFocused,
  activeTabId,
  workspaceDirectory,
  client,
  normalizedServerId,
  persistenceKey,
  isMobile,
  handleOpenPreferredAssistantFile,
  revealDirectory,
  openExplorerSidebarView,
}: UseWorkspaceMarkdownOpenerInput) {
  const openMarkdownFile = useStableEvent((location: WorkspaceFileLocation) => {
    handleOpenPreferredAssistantFile({ location, parentTabId: activeTabId });
  });

  const openMarkdownDirectory = useStableEvent((directoryPath: string) => {
    void revealDirectory(directoryPath);
    openExplorerSidebarView({
      isCompact: isMobile,
      workspaceKey: persistenceKey,
      view: "files",
    });
  });

  const openMarkdownOutsideDirectory = useOpenOutsideWorkspaceFolder(normalizedServerId);

  const markdownPathOpener = useMemo(
    () =>
      createInlinePathOpener({
        workspaceRoot: workspaceDirectory ?? "",
        client,
        openFile: openMarkdownFile,
        openDirectory: openMarkdownDirectory,
        openOutsideDirectory: openMarkdownOutsideDirectory,
      }),
    [
      workspaceDirectory,
      client,
      openMarkdownFile,
      openMarkdownDirectory,
      openMarkdownOutsideDirectory,
    ],
  );

  useEffect(() => {
    if (!isRouteFocused) markdownPathOpener.cancel();
    return () => markdownPathOpener.cancel();
  }, [markdownPathOpener, isRouteFocused, activeTabId]);

  const openMarkdownPath = useStableEvent(
    (target: InlinePathTarget, disposition: OpenFileDisposition) => {
      if (isRouteFocused) void markdownPathOpener.open(target, disposition);
    },
  );

  return {
    openMarkdownPath,
    markdownPathOpener,
  };
}
