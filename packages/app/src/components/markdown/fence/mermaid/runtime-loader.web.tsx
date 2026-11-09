import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { Pressable, Text, View } from "react-native";
import { useTranslation } from "react-i18next";
import { StyleSheet } from "react-native-unistyles";

export function useMermaidRuntimeHtml() {
  return useQuery({
    queryKey: ["mermaidRuntimeHtml"],
    // Keep the generated HTML behind a literal import: Expo emits a packaged,
    // same-origin chunk instead of putting the entire renderer in the entry JS.
    queryFn: async () => (await import("./runtime/html.gen")).mermaidRuntimeHtml,
    // Desktop assets remain reachable when browser connectivity reports offline.
    networkMode: "always",
    staleTime: Infinity,
    gcTime: Infinity,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
}

export function MermaidRuntimeStatus({ runtime }: { runtime: UseQueryResult<string, Error> }) {
  const { t } = useTranslation();
  if (runtime.data !== undefined) return null;
  if (runtime.isPending || runtime.isFetching) {
    return (
      <Text role="status" style={styles.text} testID="mermaid-runtime-loading">
        {t("common.loading")}
      </Text>
    );
  }
  return (
    <View accessibilityRole="alert" style={styles.error} testID="mermaid-runtime-error">
      <Text style={styles.text}>
        {t("common.errors.error")}: {runtime.error?.message}
      </Text>
      <Pressable
        accessibilityRole="button"
        onPress={() => void runtime.refetch()}
        testID="mermaid-runtime-retry"
      >
        <Text style={styles.retry}>{t("common.actions.retry")}</Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  text: { color: theme.colors.foregroundMuted, fontSize: theme.fontSize.sm },
  error: { gap: theme.spacing[2] },
  retry: { color: theme.colors.foreground, fontSize: theme.fontSize.sm },
}));
