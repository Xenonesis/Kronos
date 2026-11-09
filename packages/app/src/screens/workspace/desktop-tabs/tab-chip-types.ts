import type { WorkspaceTabDescriptor } from "@/screens/workspace/workspace-tabs-types";
import type { WorkspaceTabPresentation } from "@/screens/workspace/workspace-tab-presentation";

export interface WorkspaceDesktopTabRowItem {
  tab: WorkspaceTabDescriptor;
  isActive: boolean;
  isCloseHovered: boolean;
  isClosingTab: boolean;
}

export interface ResolvedWorkspaceDesktopTabRowItem extends WorkspaceDesktopTabRowItem {
  presentation: WorkspaceTabPresentation;
}
