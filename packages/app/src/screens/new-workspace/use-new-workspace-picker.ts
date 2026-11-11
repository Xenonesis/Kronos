import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { View } from "react-native";
import { useFetchQuery } from "@/data/query";
import { useForgeSearchQuery } from "@/git/use-forge-search-query";
import { useCheckoutStatusQuery } from "@/git/use-status-query";
import type { ForgeSearchItem } from "@getpaseo/protocol/messages";
import type { useHostRuntimeClient } from "@/runtime/host-runtime";
import {
  buildPickerOptionData,
  defaultBasePickerItem,
  pickerItemLabel,
  type PickerItem,
  type PickerOptionData,
} from "@/screens/new-workspace-picker-item";
import { syncPickerPrAttachment } from "@/screens/new-workspace-picker-state";
import { normalizeBranchDetails } from "./ref-picker-state";
import type { UserComposerAttachment } from "@/attachments/types";

interface UseNewWorkspacePickerProps {
  selectedServerId: string;
  selectedSourceDirectory: string | null;
  client: ReturnType<typeof useHostRuntimeClient>;
  clientReady: boolean;
  withConnectedClient: () => NonNullable<ReturnType<typeof useHostRuntimeClient>>;
  supportsForgeSearch: boolean;
  selectedItem: PickerItem | null;
  dispatchPickerSelection: (action: { type: "picker-selected"; item: PickerItem }) => void;
  chatAttachments: UserComposerAttachment[];
  setChatAttachments: (attachments: UserComposerAttachment[]) => void;
  t: (key: string) => string;
}

export function useNewWorkspacePicker({
  selectedServerId,
  selectedSourceDirectory,
  client,
  clientReady,
  withConnectedClient,
  supportsForgeSearch,
  selectedItem,
  dispatchPickerSelection,
  chatAttachments,
  setChatAttachments,
  t,
}: UseNewWorkspacePickerProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerSearchQuery, setPickerSearchQuery] = useState("");
  const [debouncedPickerSearchQuery, setDebouncedPickerSearchQuery] = useState("");
  const pickerAnchorRef = useRef<View>(null);

  useEffect(() => {
    const trimmed = pickerSearchQuery.trim();
    const timer = setTimeout(() => setDebouncedPickerSearchQuery(trimmed), 180);
    return () => clearTimeout(timer);
  }, [pickerSearchQuery]);

  const hasSelectedSourceDirectory = selectedSourceDirectory !== null;
  const pickerQueryEnabled = pickerOpen && clientReady && hasSelectedSourceDirectory;

  const { status: checkoutStatus } = useCheckoutStatusQuery({
    serverId: selectedServerId,
    cwd: selectedSourceDirectory ?? "",
  });

  const branchSuggestionsQuery = useFetchQuery({
    queryKey: [
      "branch-suggestions",
      selectedServerId,
      selectedSourceDirectory,
      debouncedPickerSearchQuery,
    ],
    dataShape: "value",
    staleTimeMs: 15_000,
    queryFn: async () => {
      if (!selectedSourceDirectory) {
        throw new Error("Choose a project");
      }
      const connectedClient = withConnectedClient();
      return connectedClient.getBranchSuggestions({
        cwd: selectedSourceDirectory,
        query: debouncedPickerSearchQuery || undefined,
        limit: 20,
      });
    },
    enabled: pickerQueryEnabled,
  });

  const githubPrSearchQuery = useForgeSearchQuery({
    client,
    serverId: selectedServerId,
    cwd: selectedSourceDirectory ?? "",
    query: debouncedPickerSearchQuery,
    kinds: ["change_request"],
    supportsForgeSearch,
    enabled: pickerQueryEnabled,
  });

  const branchDetails = useMemo(
    () => normalizeBranchDetails(branchSuggestionsQuery.data),
    [branchSuggestionsQuery.data],
  );
  const forgeSearchAuthenticated =
    !githubPrSearchQuery.data || githubPrSearchQuery.data.authState === "authenticated";
  const prItems: ForgeSearchItem[] = useMemo(() => {
    if (!forgeSearchAuthenticated) return [];
    return githubPrSearchQuery.data?.items ?? [];
  }, [forgeSearchAuthenticated, githubPrSearchQuery.data?.items]);

  const baseItem = useMemo(
    () => selectedItem ?? (checkoutStatus ? defaultBasePickerItem(checkoutStatus) : null),
    [checkoutStatus, selectedItem],
  );
  const { options, itemById, selectedOptionId }: PickerOptionData = useMemo(
    () =>
      buildPickerOptionData({
        branchDetails,
        prItems,
        baseItem,
      }),
    [baseItem, branchDetails, prItems],
  );
  const triggerLabel = useMemo(() => {
    const displayItem = itemById.get(selectedOptionId);
    return displayItem ? pickerItemLabel(displayItem) : "main";
  }, [itemById, selectedOptionId]);

  const selectPickerItem = useCallback(
    (item: PickerItem) => {
      const nextAttachments = syncPickerPrAttachment({
        attachments: chatAttachments,
        item,
      });

      dispatchPickerSelection({ type: "picker-selected", item });
      setChatAttachments(nextAttachments);
      setPickerOpen(false);
    },
    [chatAttachments, dispatchPickerSelection, setChatAttachments],
  );

  const handleSelectOption = useCallback(
    (id: string) => {
      const item = itemById.get(id);
      if (!item) return;
      selectPickerItem(item);
    },
    [itemById, selectPickerItem],
  );

  const handlePickerOpenChange = useCallback((nextOpen: boolean) => {
    setPickerOpen(nextOpen);
    if (!nextOpen) {
      setPickerSearchQuery("");
    }
  }, []);

  const openPicker = useCallback(() => {
    setPickerOpen(true);
  }, []);

  const pickerEmptyText =
    branchSuggestionsQuery.isFetching || githubPrSearchQuery.isFetching
      ? t("newWorkspace.refPicker.searching")
      : t("newWorkspace.refPicker.noMatchingRefs");

  return {
    pickerOpen,
    openPicker,
    handlePickerOpenChange,
    pickerAnchorRef,
    pickerSearchQuery,
    setPickerSearchQuery,
    checkoutStatus,
    options,
    itemById,
    selectedOptionId,
    triggerLabel,
    handleSelectOption,
    pickerEmptyText,
  };
}
