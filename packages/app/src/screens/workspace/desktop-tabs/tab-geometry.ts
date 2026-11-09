import type { Dispatch, SetStateAction } from "react";
import type { LayoutChangeEvent } from "react-native";
import { paneContentToolbarTrailingPadding } from "@/components/ui/pane-content-toolbar";
import { smallIconButtonChromeFrameSize } from "@/components/ui/icon-button-chrome";
import { retainWorkspaceTabMeasuredWidth } from "@/screens/workspace/workspace-tab-layout";

export const DROPDOWN_WIDTH = 220;
export const DEFAULT_INLINE_ADD_BUTTON_RESERVED_WIDTH = 36;
export const PANE_SPLIT_ACTIONS_HORIZONTAL_PADDING = 2;
export const PANE_SPLIT_ACTIONS_OUTER_MARGIN =
  paneContentToolbarTrailingPadding(false, "glyph") - PANE_SPLIT_ACTIONS_HORIZONTAL_PADDING;
export const PANE_SPLIT_ACTIONS_RESERVED_WIDTH =
  smallIconButtonChromeFrameSize(false) +
  PANE_SPLIT_ACTIONS_HORIZONTAL_PADDING * 2 +
  PANE_SPLIT_ACTIONS_OUTER_MARGIN;
export const PANE_MAXIMIZE_ACTION_RESERVED_WIDTH = smallIconButtonChromeFrameSize(false) + 1;

// Chip geometry. `layoutMetrics` measures tabs from these same numbers, so a chip that changes
// shape without changing them mis-measures and drops the row into the overflow-scroll fallback at
// the wrong width. Keep them together.
// Tabs and the adjacent New Tab trigger are one control family. Keep their outer box and corner
// token identical; only their horizontal sizing differs (content-width chip versus square icon).
export const TAB_CHIP_HORIZONTAL_PADDING = 8;
export const TAB_CHIP_GAP = 4;
export const TAB_ROW_PADDING_HORIZONTAL = 4;
export const TAB_ICON_WIDTH = 14;
export const TAB_CONTENT_GAP = 4;
export const TAB_DROP_INDICATOR_WIDTH = 4;
export const TAB_MODIFIED_DOT_SIZE = 8;
export const TAB_MIN_WIDTH = 64;
export const TAB_MAX_WIDTH = 160;
export const TAB_CLOSE_BUTTON_RESERVED_WIDTH = 0;
export const TAB_LABEL_LAYOUT_ALLOWANCE = 4;
export const AGENT_TOOLTIP_TITLE_MAX_LENGTH = 80;

export function updateMeasuredWidth(
  setWidth: Dispatch<SetStateAction<number>>,
  event: LayoutChangeEvent,
) {
  const nextWidth = Math.round(event.nativeEvent.layout.width);
  setWidth((current) => retainWorkspaceTabMeasuredWidth(current, nextWidth));
}
