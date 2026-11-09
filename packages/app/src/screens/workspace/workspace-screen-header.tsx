import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { isNative, isWeb } from "@/constants/platform";
import { supportsStreamTopOverlay } from "@/agent-stream/top-overlay";
import { WorkspaceTabPresentationResolver } from "@/screens/workspace/workspace-tab-presentation";
import {
  resolveWorkspaceHeaderRenderState,
  type WorkspaceHeaderCheckoutState,
} from "@/screens/workspace/workspace-header-source";
import type { CheckoutStatusPayload } from "@/git/use-status-query";
import type { WorkspaceDescriptor } from "@/stores/session-store";
import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";

export interface WorkspaceHeaderFields {
  isWorkspaceHeaderLoading: boolean;
  workspaceHeaderTitle: string;
  workspaceHeaderSubtitle: string;
  isWorkspaceHeaderSubtitleDistinct: boolean;
  isGitCheckout: boolean;
  currentBranchName: string | null;
}

export function buildWorkspaceHeaderCheckoutState(input: {
  isCheckoutStatusLoading: boolean;
  isError: boolean;
  data: CheckoutStatusPayload | undefined;
}): WorkspaceHeaderCheckoutState {
  if (input.isCheckoutStatusLoading) {
    return { kind: "pending" };
  }
  if (input.isError || !input.data) {
    return { kind: "error" };
  }
  return {
    kind: "ready",
    checkout: {
      isGit: input.data.isGit,
      currentBranch: input.data.currentBranch,
    },
  };
}

export function deriveWorkspaceHeaderFields(input: {
  workspace: WorkspaceDescriptor | null;
  checkoutState: WorkspaceHeaderCheckoutState;
}): WorkspaceHeaderFields {
  const renderState = resolveWorkspaceHeaderRenderState(input);
  if (renderState.kind !== "ready") {
    return {
      isWorkspaceHeaderLoading: true,
      workspaceHeaderTitle: "",
      workspaceHeaderSubtitle: "",
      isWorkspaceHeaderSubtitleDistinct: false,
      isGitCheckout: false,
      currentBranchName: null,
    };
  }
  return {
    isWorkspaceHeaderLoading: false,
    workspaceHeaderTitle: renderState.title,
    workspaceHeaderSubtitle: renderState.subtitle,
    isWorkspaceHeaderSubtitleDistinct: renderState.isSubtitleDistinct,
    isGitCheckout: renderState.isGitCheckout,
    currentBranchName: renderState.currentBranchName,
  };
}

export function WorkspaceDocumentTitleEffect({
  label,
  titleState,
}: {
  label: string;
  titleState: "ready" | "loading";
}) {
  const { t } = useTranslation();
  useEffect(() => {
    if (isNative || typeof document === "undefined") {
      return;
    }
    const resolvedLabel = label.trim();
    document.title =
      titleState === "loading"
        ? t("workspace.tabs.loading")
        : resolvedLabel || t("workspace.tabs.fallback.workspace");
  }, [label, titleState, t]);

  return null;
}

export function WorkspaceDocumentTitleEffectSlot({
  tab,
  serverId,
  workspaceId,
  isRouteFocused,
}: {
  tab: WorkspaceTabDescriptor | null;
  serverId: string;
  workspaceId: string;
  isRouteFocused: boolean;
}) {
  if (!isRouteFocused || !isWeb || !tab) {
    return null;
  }

  return (
    <WorkspaceTabPresentationResolver tab={tab} serverId={serverId} workspaceId={workspaceId}>
      {(presentation) => (
        <WorkspaceDocumentTitleEffect
          label={presentation.label}
          titleState={presentation.titleState}
        />
      )}
    </WorkspaceTabPresentationResolver>
  );
}

export function floatsHeaderOverContent(activeTab: WorkspaceTabDescriptor | null): boolean {
  return supportsStreamTopOverlay && activeTab?.kind === "agent";
}

/** A fullscreen composer covers the compact header; the column lays out without it. */
export function resolveCompactHeaderLayout(input: {
  header: ReactNode;
  activeTab: WorkspaceTabDescriptor | null;
  composerFullscreen: boolean;
}): { header: ReactNode; floating: boolean } {
  if (input.composerFullscreen) return { header: null, floating: false };
  return {
    header: input.header,
    floating: floatsHeaderOverContent(input.activeTab),
  };
}

export function shouldShowWorkspaceScreenHeader(input: {
  isFocusModeEnabled: boolean;
  isMobile: boolean;
}): boolean {
  return !input.isFocusModeEnabled || input.isMobile;
}
