import { useTranslation } from "react-i18next";
import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import type { ProjectConfigRpcError } from "@getpaseo/protocol/messages";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

interface ProjectConfigSaveActionsProps {
  writeError: ProjectConfigRpcError | null;
  isPending: boolean;
  hasInvalidScripts: boolean;
  onSave: () => void;
  onReload: () => void;
}

export function ProjectConfigSaveActions({
  writeError,
  isPending,
  hasInvalidScripts,
  onSave,
  onReload,
}: ProjectConfigSaveActionsProps) {
  const { t } = useTranslation();
  const isStale = writeError?.code === "stale_project_config";
  const isWriteFailed = writeError?.code === "write_failed";
  const saveDisabled = isPending || isStale || hasInvalidScripts;

  return (
    <>
      {isStale ? (
        <View style={styles.calloutWrap}>
          <Alert
            size="sm"
            testID="stale-callout"
            variant="error"
            title={t("settings.project.writeFailures.staleTitle")}
            description={t("settings.project.writeFailures.staleDescription")}
          >
            <Button testID="stale-callout-action-0" onPress={onReload} variant="outline" size="sm">
              {t("settings.project.actions.reload")}
            </Button>
          </Alert>
        </View>
      ) : null}

      {isWriteFailed ? (
        <View style={styles.calloutWrap}>
          <Alert
            size="sm"
            testID="write-failed-callout"
            variant="error"
            title={t("settings.project.writeFailures.failedTitle")}
            description={t("settings.project.writeFailures.failedDescription")}
          >
            <Button
              testID="write-failed-callout-action-0"
              onPress={onSave}
              variant="outline"
              size="sm"
            >
              {t("settings.project.actions.tryAgain")}
            </Button>
            <Button
              testID="write-failed-callout-action-1"
              onPress={onReload}
              variant="outline"
              size="sm"
            >
              {t("settings.project.actions.reload")}
            </Button>
          </Alert>
        </View>
      ) : null}

      <View style={styles.footer}>
        <Button
          testID="save-button"
          accessibilityLabel={t("settings.project.actions.save")}
          variant="default"
          size="md"
          disabled={saveDisabled}
          loading={isPending}
          onPress={onSave}
        >
          {isPending ? t("settings.project.actions.saving") : t("settings.project.actions.save")}
        </Button>
      </View>
    </>
  );
}

const styles = StyleSheet.create((theme) => ({
  calloutWrap: {
    marginTop: theme.spacing[3],
  },
  footer: {
    marginTop: theme.spacing[4],
    flexDirection: "row",
    justifyContent: "flex-end",
  },
}));
