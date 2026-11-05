import { useCallback, type ReactElement } from "react";
import { View } from "react-native";
import { Bot, Layers } from "lucide-react-native";
import { StyleSheet } from "react-native-unistyles";
import { Button } from "@/components/ui/button";

/** Which session list the screen is showing. */
export type SessionViewMode = "paseo" | "omp";

export interface SessionViewModeTabsProps {
  activeViewMode: SessionViewMode;
  onSelectViewMode: (mode: SessionViewMode) => void;
}

/** The switcher between the Paseo app's own sessions and the OMP CLI's sessions. */
export function SessionViewModeTabs({
  activeViewMode,
  onSelectViewMode,
}: SessionViewModeTabsProps): ReactElement {
  const handleSelectPaseo = useCallback(() => onSelectViewMode("paseo"), [onSelectViewMode]);
  const handleSelectOmp = useCallback(() => onSelectViewMode("omp"), [onSelectViewMode]);

  return (
    <View style={styles.viewModeTabs}>
      <Button
        variant={activeViewMode === "paseo" ? "secondary" : "ghost"}
        size="xs"
        leftIcon={Layers}
        onPress={handleSelectPaseo}
      >
        Paseo Sessions
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
