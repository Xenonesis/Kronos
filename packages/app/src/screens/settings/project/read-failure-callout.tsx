import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import type { ProjectConfigRpcError } from "@getpaseo/protocol/messages";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

interface ProjectConfigReadFailureCalloutProps {
  kind: "transport" | ProjectConfigRpcError["code"];
  error: unknown;
  onReload: () => void;
}

export function ProjectConfigReadFailureCallout({
  kind,
  error,
  onReload,
}: ProjectConfigReadFailureCalloutProps) {
  const { t } = useTranslation();
  const { testID, title, description } = resolveReadFailureCopy({
    kind,
    error,
    t,
  });
  return (
    <View style={styles.errorBlock}>
      <Alert size="md" testID={testID} variant="error" title={title} description={description}>
        <Button testID={`${testID}-action-0`} onPress={onReload} variant="outline" size="sm">
          {t("settings.project.actions.reload")}
        </Button>
      </Alert>
    </View>
  );
}

interface ReadFailureCopyInput {
  kind: ProjectConfigReadFailureCalloutProps["kind"];
  error: unknown;
  t: TFunction;
}

function resolveReadFailureCopy(input: ReadFailureCopyInput): {
  testID: string;
  title: string;
  description: string;
} {
  if (input.kind === "invalid_project_config") {
    return {
      testID: "invalid-callout",
      title: input.t("settings.project.readFailures.invalidTitle"),
      description: input.t("settings.project.readFailures.invalidDescription"),
    };
  }
  if (input.kind === "project_not_found") {
    return {
      testID: "project-not-found-callout",
      title: input.t("settings.project.readFailures.missingTitle"),
      description: input.t("settings.project.readFailures.missingSingleHost"),
    };
  }
  if (input.kind === "transport") {
    const detail = errorToDetail(input.error);
    return {
      testID: "read-transport-callout",
      title: input.t("settings.project.readFailures.transportTitle"),
      description: detail ?? input.t("settings.project.readFailures.transportFallback"),
    };
  }
  return {
    testID: "read-failed-callout",
    title: input.t("settings.project.readFailures.failedTitle"),
    description: input.t("settings.project.readFailures.failedDescription"),
  };
}

function errorToDetail(error: unknown): string | null {
  if (error instanceof Error && error.message.length > 0) return error.message;
  if (typeof error === "string" && error.length > 0) return error;
  return null;
}

const styles = StyleSheet.create((theme) => ({
  errorBlock: {
    marginTop: theme.spacing[2],
  },
}));
