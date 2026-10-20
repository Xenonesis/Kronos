import React from "react";
import { View, StyleSheet } from "react-native";
import { Columns } from "lucide-react-native";
import { ToolbarButton } from "@/components/ui/pane-content-toolbar";

/** The share of the split the primary pane takes. */
const DEFAULT_SPLIT_RATIO = 0.5;

export interface SplitLayoutContainerProps {
  primary: React.ReactNode;
  secondary: React.ReactNode;
  isSplit: boolean;
  onToggleSplit?: () => void;
  testID?: string;
}

export function SplitLayoutContainer({
  primary,
  secondary,
  isSplit,
  onToggleSplit,
  testID = "split-layout-container",
}: SplitLayoutContainerProps) {
  const splitRatio = DEFAULT_SPLIT_RATIO;

  if (!isSplit) {
    return (
      <View style={styles.container} testID={testID}>
        {primary}
      </View>
    );
  }

  return (
    <View style={styles.splitContainer} testID={testID}>
      {/* Left Primary Pane */}
      <View style={[styles.pane, { flex: splitRatio }]}>{primary}</View>

      {/* Resize Divider */}
      <View style={styles.divider}>
        {onToggleSplit ? (
          <ToolbarButton
            testID="split-layout-toggle-button"
            label="Toggle Single/Split View"
            onPress={onToggleSplit}
            compact
          >
            <Columns size={12} color="#94a3b8" />
          </ToolbarButton>
        ) : null}
      </View>

      {/* Right Secondary Pane (Browser / Preview) */}
      <View style={[styles.pane, { flex: 1 - splitRatio }]}>{secondary}</View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    width: "100%",
    height: "100%",
  },
  splitContainer: {
    flex: 1,
    flexDirection: "row",
    width: "100%",
    height: "100%",
  },
  pane: {
    height: "100%",
    overflow: "hidden",
  },
  divider: {
    width: 6,
    backgroundColor: "#1e293b",
    borderLeftWidth: 1,
    borderRightWidth: 1,
    borderColor: "#334155",
    justifyContent: "center",
    alignItems: "center",
    cursor: "col-resize" as unknown as undefined,
  },
});
