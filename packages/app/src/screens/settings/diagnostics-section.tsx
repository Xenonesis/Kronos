import { useCallback } from "react";
import { Text, View } from "react-native";
import { useTranslation } from "react-i18next";
import { SettingsSection } from "@/components/settings/headings/settings-section";
import { Button } from "@/components/ui/button";
import { useAppDiagnosticStore } from "@/diagnostics/store";
import { settingsStyles } from "@/styles/settings";

interface DiagnosticsSectionProps {
  canTestPlayback: boolean;
  isPlaybackTestRunning: boolean;
  playbackTestResult: string | null;
  onPlaybackTest: () => Promise<void>;
}

export function DiagnosticsSection({
  canTestPlayback,
  isPlaybackTestRunning,
  playbackTestResult,
  onPlaybackTest,
}: DiagnosticsSectionProps) {
  const { t } = useTranslation();
  const openAppDiagnostic = useAppDiagnosticStore((state) => state.open);
  const handlePlayPress = useCallback(() => {
    void onPlaybackTest();
  }, [onPlaybackTest]);
  return (
    <SettingsSection title={t("settings.diagnostics.title")}>
      <View style={settingsStyles.card}>
        <View style={settingsStyles.row} testID="app-diagnostic-row">
          <View style={settingsStyles.rowContent}>
            <Text style={settingsStyles.rowTitle}>{t("settings.diagnostics.app.rowTitle")}</Text>
            <Text style={settingsStyles.rowHint}>{t("settings.diagnostics.app.rowHint")}</Text>
          </View>
          <Button variant="secondary" size="sm" onPress={openAppDiagnostic}>
            {t("settings.diagnostics.app.run")}
          </Button>
        </View>
        <View style={settingsStyles.row}>
          <View style={settingsStyles.rowContent}>
            <Text style={settingsStyles.rowTitle}>{t("settings.diagnostics.testAudio")}</Text>
            {playbackTestResult ? (
              <Text style={settingsStyles.rowHint}>{playbackTestResult}</Text>
            ) : null}
          </View>
          <Button
            variant="secondary"
            size="sm"
            onPress={handlePlayPress}
            disabled={!canTestPlayback || isPlaybackTestRunning}
          >
            {isPlaybackTestRunning
              ? t("settings.diagnostics.playing")
              : t("settings.diagnostics.playTest")}
          </Button>
        </View>
      </View>
    </SettingsSection>
  );
}
