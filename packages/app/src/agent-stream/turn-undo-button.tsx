import { memo, useState, useCallback, useRef } from "react";
import { View, Text, type StyleProp, type ViewStyle } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Undo2, Check } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";

const ThemedUndo = withUnistyles(Undo2);
const ThemedCheck = withUnistyles(Check);

export interface TurnUndoButtonProps {
  onUndo?: () => Promise<void> | void;
  containerStyle?: StyleProp<ViewStyle>;
  disabled?: boolean;
}

export const TurnUndoButton = memo(function TurnUndoButton({
  onUndo,
  containerStyle,
  disabled,
}: TurnUndoButtonProps) {
  const [undone, setUndone] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handlePress = useCallback(async () => {
    if (disabled || undone || !onUndo) return;
    try {
      await onUndo();
      setUndone(true);
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      timeoutRef.current = setTimeout(() => {
        setUndone(false);
        timeoutRef.current = null;
      }, 3000);
    } catch {
      // Ignore
    }
  }, [disabled, undone, onUndo]);

  return (
    <View style={containerStyle}>
      <PressHighlight
        onPress={handlePress}
        disabled={disabled || undone}
        style={styles.button}
        accessibilityRole="button"
        accessibilityLabel={undone ? "Turn undone" : "Revert this turn"}
      >
        {undone ? (
          <View style={styles.contentRow}>
            <ThemedCheck size={13} color="#10b981" />
            <Text style={styles.successText}>Reverted</Text>
          </View>
        ) : (
          <View style={styles.contentRow}>
            <ThemedUndo size={13} color="#888" />
            <Text style={styles.buttonText}>Revert Turn</Text>
          </View>
        )}
      </PressHighlight>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  button: {
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 2,
    borderRadius: theme.borderRadius.sm,
    backgroundColor: theme.colors.surface2 ?? "transparent",
  },
  contentRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  buttonText: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  successText: {
    fontSize: theme.fontSize.code,
    color: "#10b981",
    fontWeight: "500",
  },
}));
