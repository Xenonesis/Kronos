import { memo, useCallback } from "react";
import { Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Compass, Check } from "lucide-react-native";
import { PressHighlight } from "@/components/ui/press-highlight";

const MutedCompassIcon = withUnistyles(Compass, (theme) => ({
  color: theme.colors.foregroundMuted,
}));

export interface ArchitectModeToggleProps {
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
}

export const ArchitectModeToggle = memo(function ArchitectModeToggle({
  enabled,
  onToggle,
}: ArchitectModeToggleProps) {
  const handlePress = useCallback(() => {
    onToggle(!enabled);
  }, [enabled, onToggle]);

  return (
    <PressHighlight
      onPress={handlePress}
      style={[styles.toggleBtn, enabled && styles.toggleBtnActive]}
      accessibilityLabel="Toggle Two-Stage Architect Mode"
    >
      {enabled ? (
        <Compass size={13} color="#ffffff" />
      ) : (
        <MutedCompassIcon size={13} />
      )}
      <Text style={[styles.btnText, enabled && styles.btnTextActive]}>Architect Mode</Text>
      {enabled && <Check size={11} color="#ffffff" />}
    </PressHighlight>
  );
});

const styles = StyleSheet.create((theme) => ({
  toggleBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
    backgroundColor: theme.colors.surface2,
    borderWidth: 1,
    borderColor: theme.colors.border,
    paddingHorizontal: theme.spacing[2],
    paddingVertical: 3,
    borderRadius: theme.borderRadius.sm,
  },
  toggleBtnActive: {
    backgroundColor: theme.colors.accent,
    borderColor: theme.colors.accent,
  },
  btnText: {
    fontSize: theme.fontSize.code,
    fontWeight: "500",
    color: theme.colors.foregroundMuted,
  },
  btnTextActive: {
    color: "#ffffff",
    fontWeight: "600",
  },
}));
