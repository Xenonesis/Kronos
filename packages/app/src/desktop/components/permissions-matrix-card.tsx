import { memo, useCallback, useMemo } from "react";
import { View, Text } from "react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import { Shield, Zap, Volume2 } from "lucide-react-native";
import { Switch } from "@/components/ui/switch";
import { SegmentedControl, type SegmentedControlOption } from "@/components/ui/segmented-control";
import { Button } from "@/components/ui/button";
import { useAppSettings } from "@/hooks/use-settings";
import { playTaskSuccessSound } from "@/audio/audio-suite";

const ThemedShield = withUnistyles(Shield);
const ThemedZap = withUnistyles(Zap);
const ThemedVolume = withUnistyles(Volume2);

interface PermissionToggleRowProps {
  label: string;
  description: string;
  value: boolean;
  onValueChange: (val: boolean) => void;
  disabled?: boolean;
}

const PermissionToggleRow = memo(function PermissionToggleRow({
  label,
  description,
  value,
  onValueChange,
  disabled,
}: PermissionToggleRowProps) {
  return (
    <View style={styles.toggleRow}>
      <View style={styles.toggleTextCol}>
        <Text style={styles.toggleLabel}>{label}</Text>
        <Text style={styles.toggleDesc}>{description}</Text>
      </View>
      <Switch
        value={value}
        onValueChange={onValueChange}
        disabled={disabled}
        accessibilityLabel={label}
      />
    </View>
  );
});

export const PermissionsMatrixCard = memo(function PermissionsMatrixCard() {
  const { settings, updateSettings } = useAppSettings();

  const executionMode = settings.executionMode ?? "guarded";
  const isYolo = executionMode === "yolo";

  const handleModeChange = useCallback(
    (mode: "guarded" | "yolo") => {
      void updateSettings({ executionMode: mode });
    },
    [updateSettings],
  );

  const handleToggleReadFiles = useCallback(
    (val: boolean) => {
      void updateSettings({ autoApproveReadFiles: val });
    },
    [updateSettings],
  );

  const handleToggleTerminalSafe = useCallback(
    (val: boolean) => {
      void updateSettings({ autoApproveTerminalSafe: val });
    },
    [updateSettings],
  );

  const handleToggleFileEdits = useCallback(
    (val: boolean) => {
      void updateSettings({ autoApproveFileEdits: val });
    },
    [updateSettings],
  );

  const handleToggleWebSearch = useCallback(
    (val: boolean) => {
      void updateSettings({ autoApproveWebSearch: val });
    },
    [updateSettings],
  );

  const handleToggleMcp = useCallback(
    (val: boolean) => {
      void updateSettings({ autoApproveMcp: val });
    },
    [updateSettings],
  );

  const handleToggleAudio = useCallback(
    (val: boolean) => {
      void updateSettings({ audioNotificationsEnabled: val });
    },
    [updateSettings],
  );

  const handleTestChime = useCallback(() => {
    playTaskSuccessSound({ muted: false, volume: 0.3 });
  }, []);

  const modeOptions: SegmentedControlOption<"guarded" | "yolo">[] = useMemo(
    () => [
      {
        value: "guarded",
        label: "Guarded (Safe)",
      },
      {
        value: "yolo",
        label: "Yolo (Autonomous)",
      },
    ],
    [],
  );

  return (
    <View style={styles.card}>
      <View style={styles.header}>
        <View style={styles.headerTitleRow}>
          {isYolo ? (
            <ThemedZap size={18} color="#f59e0b" />
          ) : (
            <ThemedShield size={18} color="#10b981" />
          )}
          <Text style={styles.title}>Agent Permissions & Execution Mode</Text>
        </View>
        <Text style={styles.subtitle}>
          Configure autonomous capability matrix and auto-approval policies.
        </Text>
      </View>

      <View style={styles.modeSelectorRow}>
        <Text style={styles.sectionHeading}>Master Execution Mode</Text>
        <SegmentedControl<"guarded" | "yolo">
          options={modeOptions}
          value={executionMode}
          onValueChange={handleModeChange}
        />
        {isYolo && (
          <View style={styles.yoloWarning}>
            <Text style={styles.yoloWarningText}>
              ⚡ Yolo Mode enabled: Tool calls will execute at maximum autonomous speed.
            </Text>
          </View>
        )}
      </View>

      <View style={styles.matrixContainer}>
        <Text style={styles.sectionHeading}>Tool Auto-Approval Matrix</Text>

        <PermissionToggleRow
          label="Auto-Approve File Reads & Search"
          description="Read repository files, outline symbols, and search code without prompting."
          value={settings.autoApproveReadFiles ?? true}
          onValueChange={handleToggleReadFiles}
        />

        <PermissionToggleRow
          label="Auto-Approve Safe Terminal Commands"
          description="Allow non-destructive diagnostic commands (cargo check, npm test, git status)."
          value={settings.autoApproveTerminalSafe ?? true}
          onValueChange={handleToggleTerminalSafe}
        />

        <PermissionToggleRow
          label="Auto-Approve File Modifications"
          description="Directly apply code edits and write files without per-turn confirmation."
          value={settings.autoApproveFileEdits ?? false}
          onValueChange={handleToggleFileEdits}
        />

        <PermissionToggleRow
          label="Auto-Approve Live Web Search"
          description="Perform real-time web searches and documentation grounding queries."
          value={settings.autoApproveWebSearch ?? true}
          onValueChange={handleToggleWebSearch}
        />

        <PermissionToggleRow
          label="Auto-Approve MCP Tool Invocations"
          description="Execute Model Context Protocol extensions and server methods."
          value={settings.autoApproveMcp ?? false}
          onValueChange={handleToggleMcp}
        />
      </View>

      <View style={styles.audioSection}>
        <View style={styles.audioHeader}>
          <View style={styles.audioTitleRow}>
            <ThemedVolume size={16} color="#6366f1" />
            <Text style={styles.sectionHeading}>Audio Notification Suite</Text>
          </View>
          <Button variant="ghost" size="sm" onPress={handleTestChime}>
            Test Chime
          </Button>
        </View>
        <PermissionToggleRow
          label="Chime on Task Completion & Attention"
          description="Play pleasant audio chimes when long-running agent tasks finish or need attention."
          value={settings.audioNotificationsEnabled ?? true}
          onValueChange={handleToggleAudio}
        />
      </View>
    </View>
  );
});

const styles = StyleSheet.create((theme) => ({
  card: {
    backgroundColor: theme.colors.surface0,
    borderColor: theme.colors.border,
    borderWidth: 1,
    borderRadius: theme.borderRadius.lg,
    padding: theme.spacing[4],
    gap: theme.spacing[4],
    marginVertical: theme.spacing[3],
  },
  header: {
    gap: theme.spacing[1],
  },
  headerTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  title: {
    fontSize: theme.fontSize.lg,
    fontWeight: "600",
    color: theme.colors.foreground,
  },
  subtitle: {
    fontSize: theme.fontSize.sm,
    color: theme.colors.foregroundMuted,
  },
  modeSelectorRow: {
    gap: theme.spacing[2],
    paddingBottom: theme.spacing[2],
    borderBottomWidth: 1,
    borderBottomColor: theme.colors.border,
  },
  sectionHeading: {
    fontSize: theme.fontSize.sm,
    fontWeight: "600",
    color: theme.colors.foreground,
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  yoloWarning: {
    backgroundColor: "rgba(245, 158, 11, 0.15)",
    padding: theme.spacing[2],
    borderRadius: theme.borderRadius.sm,
    marginTop: theme.spacing[1],
  },
  yoloWarningText: {
    fontSize: theme.fontSize.code,
    color: "#b45309",
    fontWeight: "500",
  },
  matrixContainer: {
    gap: theme.spacing[3],
  },
  toggleRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingVertical: theme.spacing[1],
  },
  toggleTextCol: {
    flex: 1,
    paddingRight: theme.spacing[3],
    gap: 2,
  },
  toggleLabel: {
    fontSize: theme.fontSize.sm,
    fontWeight: "500",
    color: theme.colors.foreground,
  },
  toggleDesc: {
    fontSize: theme.fontSize.code,
    color: theme.colors.foregroundMuted,
  },
  audioSection: {
    paddingTop: theme.spacing[3],
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
    gap: theme.spacing[2],
  },
  audioHeader: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  audioTitleRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
}));
