import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { WorkspaceDescriptor } from "@/stores/session-store";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import type { WorkspaceFileLocation } from "@/workspace/file-open";
import { workspaceTabTargetsEqual } from "@/workspace-tabs/identity";
import {
  type WorkspaceLayout,
  type WorkspaceTabPlacement,
  FOCUSED_PANE_PLACEMENT,
  getFocusedBrowserId,
} from "@/stores/workspace-layout-store";
import type { WorkspaceTabTarget } from "@/workspace-tabs/model";
import { getDesktopHost } from "@/desktop/host";
import { getIsElectron } from "@/constants/platform";
import type { TerminalTabDestination } from "@/screens/workspace/terminals/use-workspace-terminals";

export const NO_SHEETS = [] as const;
export const WORKSPACE_FLOATING_PANEL_PORTAL_HOST_PREFIX = "workspace-floating-panels";
export const EMPTY_WORKSPACE_SCRIPTS: WorkspaceDescriptor["scripts"] = [];
export const EMPTY_PINNED_AGENT_IDS = new Set<string>();
export const EMPTY_SET = new Set<string>();

export function getWorkspaceSetupStatus(workspace: WorkspaceDescriptor | null | undefined) {
  return workspace?.setupStatus ?? null;
}

export function getWorkspaceScripts(
  workspaceDescriptor: WorkspaceDescriptor | null | undefined,
): WorkspaceDescriptor["scripts"] {
  return workspaceDescriptor?.scripts ?? EMPTY_WORKSPACE_SCRIPTS;
}

export interface WorkspaceFileLocationFields {
  path: string | null;
  lineStart?: number;
  lineEnd?: number;
}

export function getWorkspaceFileLocationFields(
  tab: WorkspaceTabDescriptor | null,
): WorkspaceFileLocationFields {
  const target = tab?.target;
  if (target?.kind !== "file") {
    return { path: null };
  }
  return {
    path: target.path,
    lineStart: target.lineStart,
    lineEnd: target.lineEnd,
  };
}

export function buildWorkspaceFileLocation(
  fields: WorkspaceFileLocationFields,
): WorkspaceFileLocation | null {
  if (fields.path === null) {
    return null;
  }
  return {
    path: fields.path,
    lineStart: fields.lineStart,
    lineEnd: fields.lineEnd,
  };
}

export function trimNonEmpty(value: string | null | undefined): string | null {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export function decodeSegment(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

export function useSyncWorkspaceActiveBrowser(input: {
  workspaceLayout: WorkspaceLayout | null;
  isRouteFocused: boolean;
  workspaceId: string;
}) {
  const focusedBrowserId = useMemo(
    () => getFocusedBrowserId(input.workspaceLayout),
    [input.workspaceLayout],
  );

  useEffect(() => {
    if (!getIsElectron()) {
      return;
    }
    void getDesktopHost()?.browser?.setWorkspaceActiveBrowser?.({
      workspaceId: input.workspaceId,
      browserId: focusedBrowserId,
    });
  }, [focusedBrowserId, input.workspaceId]);
}

export function useStableTabDescriptorMap(tabDescriptors: WorkspaceTabDescriptor[]) {
  const cacheRef = useRef(new Map<string, WorkspaceTabDescriptor>());
  const tabDescriptorMap = useMemo(() => {
    const next = new Map<string, WorkspaceTabDescriptor>();
    for (const tabDescriptor of tabDescriptors) {
      const cachedDescriptor = cacheRef.current.get(tabDescriptor.tabId);
      if (
        cachedDescriptor &&
        cachedDescriptor.key === tabDescriptor.key &&
        cachedDescriptor.kind === tabDescriptor.kind &&
        cachedDescriptor.state === tabDescriptor.state &&
        workspaceTabTargetsEqual(cachedDescriptor.target, tabDescriptor.target)
      ) {
        next.set(tabDescriptor.tabId, cachedDescriptor);
        continue;
      }
      next.set(tabDescriptor.tabId, tabDescriptor);
    }
    return next;
  }, [tabDescriptors]);
  useEffect(() => {
    cacheRef.current = tabDescriptorMap;
  }, [tabDescriptorMap]);

  return tabDescriptorMap;
}

export interface UseCloseTabsResult {
  closingTabIds: Set<string>;
  closeTab: (tabId: string, action: () => Promise<void>) => Promise<void>;
}

export function useCloseTabs(): UseCloseTabsResult {
  const pendingRef = useRef(new Set<string>());
  const [closingTabIds, setClosingTabIds] = useState<Set<string>>(EMPTY_SET);

  const closeTab = useCallback(async (tabId: string, action: () => Promise<void>) => {
    const normalized = tabId.trim();
    if (!normalized || pendingRef.current.has(normalized)) {
      return;
    }
    pendingRef.current.add(normalized);
    setClosingTabIds(new Set(pendingRef.current));
    try {
      await action();
    } finally {
      pendingRef.current.delete(normalized);
      setClosingTabIds(new Set(pendingRef.current));
    }
  }, []);

  return { closingTabIds, closeTab };
}

/**
 * A pane the user acted inside owns the tab: it opens there, and an existing tab
 * moves there. No pane means the open has no opinion beyond the focused pane.
 */
export function paneLocalPlacement(paneId: string | null | undefined): WorkspaceTabPlacement {
  return paneId ? { mode: "pane", paneId } : FOCUSED_PANE_PLACEMENT;
}

export interface WorkspaceTerminalTabActionsInput {
  persistenceKey: string | null;
  openWorkspaceTabFocused: (
    workspaceKey: string,
    target: WorkspaceTabTarget,
    placement?: WorkspaceTabPlacement,
  ) => string | null;
  replaceWorkspaceTabTarget: (
    workspaceKey: string,
    tabId: string,
    target: WorkspaceTabTarget,
  ) => string | null;
  labels: {
    workspacePathUnavailable: string;
    terminalQueued: string;
  };
  toast: {
    error: (message: string) => void;
    show: (message: string) => void;
  };
}

export interface WorkspaceTerminalTabActions {
  handleTerminalCreated: (input: {
    terminalId: string;
    destination: TerminalTabDestination;
  }) => void;
  handleScriptTerminalSelected: (terminalId: string) => void;
  handleWorkspacePathUnavailable: () => void;
  handleTerminalCreateQueued: () => void;
  handleTerminalCreateFailed: (reason: string) => void;
}

export function useWorkspaceTerminalTabActions({
  persistenceKey,
  openWorkspaceTabFocused,
  replaceWorkspaceTabTarget,
  labels,
  toast,
}: WorkspaceTerminalTabActionsInput): WorkspaceTerminalTabActions {
  const handleTerminalCreated = useCallback(
    ({ terminalId, destination }: { terminalId: string; destination: TerminalTabDestination }) => {
      if (!persistenceKey) {
        return;
      }
      if (destination.kind === "replace") {
        replaceWorkspaceTabTarget(persistenceKey, destination.tabId, {
          kind: "terminal",
          terminalId,
        });
        return;
      }
      openWorkspaceTabFocused(
        persistenceKey,
        { kind: "terminal", terminalId },
        paneLocalPlacement(destination.paneId),
      );
    },
    [openWorkspaceTabFocused, persistenceKey, replaceWorkspaceTabTarget],
  );
  const handleScriptTerminalSelected = useCallback(
    (terminalId: string) => {
      if (!persistenceKey) {
        return;
      }
      openWorkspaceTabFocused(
        persistenceKey,
        { kind: "terminal", terminalId },
        FOCUSED_PANE_PLACEMENT,
      );
    },
    [openWorkspaceTabFocused, persistenceKey],
  );
  const handleWorkspacePathUnavailable = useCallback(() => {
    toast.error(labels.workspacePathUnavailable);
  }, [labels.workspacePathUnavailable, toast]);
  const handleTerminalCreateQueued = useCallback(() => {
    toast.show(labels.terminalQueued);
  }, [labels.terminalQueued, toast]);
  const handleTerminalCreateFailed = useCallback(
    (reason: string) => {
      toast.error(reason);
    },
    [toast],
  );

  return {
    handleTerminalCreated,
    handleScriptTerminalSelected,
    handleWorkspacePathUnavailable,
    handleTerminalCreateQueued,
    handleTerminalCreateFailed,
  };
}

export function useLastMainPane(input: {
  workspaceKey: string | null;
  layout: WorkspaceLayout | null;
  explorerSidebarPaneId: string | null;
}) {
  const lastMainPaneRef = useRef<{
    workspaceKey: string | null;
    paneId: string | null;
  }>({
    workspaceKey: null,
    paneId: null,
  });
  if (lastMainPaneRef.current.workspaceKey !== input.workspaceKey) {
    lastMainPaneRef.current = {
      workspaceKey: input.workspaceKey,
      paneId: null,
    };
  }
  const focusedPaneId = input.layout?.focusedPaneId ?? null;
  if (focusedPaneId && focusedPaneId !== input.explorerSidebarPaneId) {
    lastMainPaneRef.current.paneId = focusedPaneId;
  }
  return lastMainPaneRef;
}
