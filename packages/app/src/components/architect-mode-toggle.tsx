import { memo, useCallback } from "react";
import { View, Text, Pressable } from "react-native";
import { StyleSheet, useUnistyles } from "react-native-unistyles";
import { Compass, Check } from "lucide-react-native";

export interface ArchitectModeToggleProps {
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
}

export const ArchitectModeToggle = memo(function ArchitectModeToggle({
  enabled,
  onToggle,
}: ArchitectModeToggleProps) {
  const { theme } = useUnistyles();

  const handlePress = useCallback(() => {
    onToggle(!enabled);
  }, [enabled, onToggle]);

  return (
    <Pressable
      onPress={handlePress}
      style={[styles.toggleBtn, enabled && styles.toggleBtnActive]}
      accessibilityLabel="Toggle Two-Stage Architect Mode"
    >
      <Compass size={13} color={enabled ? "#ffffff" : theme.colors.foregroundMuted} />
      <Text style={[styles.btnText, enabled && styles.btnTextActive]}>Architect Mode</Text>
      {enabled && <Check size={11} color="#ffffff" />}
    </Pressable>
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
