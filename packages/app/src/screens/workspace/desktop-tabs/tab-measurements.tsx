import React, { useCallback } from "react";
import { Text, type LayoutChangeEvent } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import type { ResolvedWorkspaceDesktopTabRowItem } from "./tab-chip-types";
import { TAB_CONTENT_GAP, TAB_MODIFIED_DOT_SIZE, TAB_LABEL_LAYOUT_ALLOWANCE } from "./tab-geometry";

export interface WorkspaceTabLabel {
  key: string;
  label: string;
  modified: boolean;
}

export interface WorkspaceTabLabelMeasurement {
  label: string;
  width: number;
}

export interface WorkspaceTabTrackSnapshot {
  signature: string;
  tabs: ResolvedWorkspaceDesktopTabRowItem[];
  labels: WorkspaceTabLabel[];
  labelWidths: number[];
}

export function workspaceTabLabelSignature(labels: WorkspaceTabLabel[]): string {
  return JSON.stringify(labels);
}

export function completeWorkspaceTabLabelWidths(
  labels: WorkspaceTabLabel[],
  measurements: Map<string, WorkspaceTabLabelMeasurement>,
): number[] | null {
  const widths: number[] = [];
  for (const { key, label, modified } of labels) {
    const measurement = measurements.get(key);
    if (!measurement || measurement.label !== label || measurement.width <= 0) {
      return null;
    }
    // The modified dot sits in the content row, so a modified tab needs that much more width
    // before its label starts truncating.
    const modifiedAllowance = modified ? TAB_CONTENT_GAP + TAB_MODIFIED_DOT_SIZE : 0;
    widths.push(measurement.width + TAB_LABEL_LAYOUT_ALLOWANCE + modifiedAllowance);
  }
  return widths;
}

export function sameWidths(left: number[], right: number[]): boolean {
  return left.length === right.length && left.every((width, index) => width === right[index]);
}

export function TabLabelMeasurement({
  tabKey,
  label,
  onMeasure,
}: {
  tabKey: string;
  label: string;
  onMeasure: (tabKey: string, label: string, event: LayoutChangeEvent) => void;
}) {
  const handleLayout = useCallback(
    (event: LayoutChangeEvent) => onMeasure(tabKey, label, event),
    [label, onMeasure, tabKey],
  );

  return (
    <Text
      style={[styles.tabLabel, styles.tabLabelMeasurement]}
      numberOfLines={1}
      onLayout={handleLayout}
    >
      {label}
    </Text>
  );
}

const styles = StyleSheet.create((theme) => ({
  tabLabel: {
    flexShrink: 1,
    minWidth: 0,
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
    fontWeight: theme.fontWeight.normal,
    userSelect: "none",
  },
  tabLabelMeasurement: {
    flexShrink: 0,
  },
}));
