import type { ReactNode, RefObject } from "react";
import type { View } from "react-native";
import { useNewWorkspaceForm, type NewWorkspaceFormProps } from "./form";

export function useNewWorkspaceFormStack(props: {
  isCompact: boolean;
  isPending: boolean;
  pluginItems?: ReactNode;
  project: {
    anchorRef: RefObject<View | null>;
    open: () => void;
    openState: boolean;
    onOpenChange: (nextOpen: boolean) => void;
    options: NewWorkspaceFormProps["project"]["options"];
    projectByOptionId: NewWorkspaceFormProps["project"]["projectByOptionId"];
    iconDataByProjectViewKey: NewWorkspaceFormProps["project"]["iconDataByProjectViewKey"];
    selectedProject: NewWorkspaceFormProps["project"]["selectedProject"];
    selectedOptionId: string;
    triggerLabel: string;
    selectedServerId: string;
    supportsWorkspaceMultiplicity: boolean;
    onSelect: (id: string) => void;
    onAddProject: () => void;
  };
  host: {
    anchorRef: RefObject<View | null>;
    open: () => void;
    openState: boolean;
    onOpenChange: (nextOpen: boolean) => void;
    allHosts: NewWorkspaceFormProps["host"]["allHosts"];
    selectedServerId: string;
    onSelect: (id: string) => void;
    onAddHost: () => void;
  };
  isolation: {
    anchorRef: RefObject<View | null>;
    open: () => void;
    openState: boolean;
    onOpenChange: (nextOpen: boolean) => void;
    effectiveIsolation: NewWorkspaceFormProps["isolation"]["effectiveIsolation"];
    canCreateWorktree: boolean;
    onSelect: NewWorkspaceFormProps["isolation"]["onSelect"];
  };
  base: {
    anchorRef: RefObject<View | null>;
    open: () => void;
    openState: boolean;
    onOpenChange: (nextOpen: boolean) => void;
    selectedSourceDirectory: string | null;
    selectedItem: NewWorkspaceFormProps["base"]["selectedItem"];
    triggerLabel: string;
    options: NewWorkspaceFormProps["base"]["options"];
    itemById: NewWorkspaceFormProps["base"]["itemById"];
    selectedOptionId: string;
    onSelect: (id: string) => void;
    setSearchQuery: (query: string) => void;
    emptyText: string;
    showRefPicker: boolean;
  };
  launch: {
    serverId: string;
    target: NewWorkspaceFormProps["launch"]["target"];
    onChange: NewWorkspaceFormProps["launch"]["onChange"];
    profiles: NewWorkspaceFormProps["launch"]["profiles"];
    disabled: boolean;
  };
}) {
  return useNewWorkspaceForm({
    ...props,
    pluginItems: props.pluginItems ?? null,
  });
}
