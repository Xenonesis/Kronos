import { memo, useCallback, useState, type ReactElement } from "react";
import { Pressable, Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { composerPillStyles } from "@/composer/pill-styles";
import { useCascadeFlowStore } from "@/stores/cascade-flow-store";

export const CascadeFlowPill = memo(function CascadeFlowPill(): ReactElement | null {
  const isFlowActive = useCascadeFlowStore((state) => state.isFlowActive);
  const activeFile = useCascadeFlowStore((state) => state.activeFile);
  const cursorLine = useCascadeFlowStore((state) => state.cursorLine);
  const toggleFlow = useCascadeFlowStore((state) => state.toggleFlow);

  const [isHovered, setIsHovered] = useState(false);

  const handleToggle = useCallback(() => {
    toggleFlow();
  }, [toggleFlow]);

  if (!activeFile) {
    return null;
  }

  const shortName = activeFile.split("/").pop()?.split("\\").pop() ?? activeFile;

  return (
    <View style={styles.container}>
      <Pressable
        testID="cascade-flow-pill"
        accessibilityRole="button"
        onPress={handleToggle}
        onHoverIn={() => setIsHovered(true)}
        onHoverOut={() => setIsHovered(false)}
        style={[
          composerPillStyles.body,
          styles.pill,
          isHovered && styles.pillHover,
          !isFlowActive && styles.pillDisabled,
        ]}
      >
        <View style={[styles.dot, isFlowActive ? styles.dotActive : styles.dotInactive]} />
        <Text style={styles.label}>
          Flow: {shortName}:{cursorLine}
        </Text>
        <Text style={styles.statusBadge}>{isFlowActive ? "SYNC" : "PAUSED"}</Text>
      </Pressable>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  container: {
    flexDirection: "row",
    alignItems: "center",
  },
  pill: {
    flexDirection: "row",
    alignItems: "center",
    backgroundColor: "rgba(34, 197, 94, 0.08)",
    borderColor: "rgba(34, 197, 94, 0.25)",
    borderWidth: 1,
    borderRadius: 6,
    paddingHorizontal: 8,
    paddingVertical: 3,
    gap: 6,
  },
  pillHover: {
    backgroundColor: "rgba(34, 197, 94, 0.15)",
  },
  pillDisabled: {
    backgroundColor: "rgba(113, 113, 122, 0.1)",
    borderColor: "rgba(113, 113, 122, 0.2)",
  },
  dot: {
    width: 6,
    height: 6,
    borderRadius: 3,
  },
  dotActive: {
    backgroundColor: "#22c55e",
  },
  dotInactive: {
    backgroundColor: "#71717a",
  },
  label: {
    fontSize: 11,
    fontFamily: theme.fontFamily.mono,
    color: theme.colors.foreground,
  },
  statusBadge: {
    fontSize: 9,
    fontWeight: "700",
    color: "#a1a1aa",
    letterSpacing: 0.5,
  },
}));
