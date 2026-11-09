import { useCallback, type ReactElement } from "react";
import { View } from "react-native";
import { Bot, Layers, Sparkles } from "lucide-react-native";
import { StyleSheet } from "react-native-unistyles";
import { Button } from "@/components/ui/button";

/** Which session list the screen is showing. */
export type SessionViewMode = "indexed" | "kronos" | "omp";

export interface SessionViewModeTabsProps {
  activeViewMode: SessionViewMode;
  onSelectViewMode: (mode: SessionViewMode) => void;
}

/** The switcher between Universal Agent Session History and Kronos / OMP views. */
export function SessionViewModeTabs({
  activeViewMode,
  onSelectViewMode,
}: SessionViewModeTabsProps): ReactElement {
  const handleSelectIndexed = useCallback(() => onSelectViewMode("indexed"), [onSelectViewMode]);
  const handleSelectKronos = useCallback(() => onSelectViewMode("kronos"), [onSelectViewMode]);
  const handleSelectOmp = useCallback(() => onSelectViewMode("omp"), [onSelectViewMode]);

  return (
    <View style={styles.viewModeTabs}>
      <Button
        variant={activeViewMode === "indexed" ? "secondary" : "ghost"}
        size="xs"
        leftIcon={Sparkles}
        onPress={handleSelectIndexed}
      >
        All Agent History
      </Button>
      <Button
        variant={activeViewMode === "kronos" ? "secondary" : "ghost"}
        size="xs"
        leftIcon={Layers}
        onPress={handleSelectKronos}
      >
        Kronos Sessions
      </Button>
      <Button
        variant={activeViewMode === "omp" ? "secondary" : "ghost"}
        size="xs"
        leftIcon={Bot}
        onPress={handleSelectOmp}
      >
        OMP CLI Sessions
      </Button>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  viewModeTabs: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
    paddingHorizontal: theme.spacing[4],
    paddingBottom: theme.spacing[2],
  },
}));
