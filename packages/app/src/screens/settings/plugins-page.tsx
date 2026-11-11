import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { useMutation } from "@tanstack/react-query";
import type { PluginListItem } from "@getpaseo/protocol/messages";
import { SettingsCard } from "@/components/settings";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ExternalLink } from "@/components/ui/external-link";
import { Field, FormTextInput } from "@/components/ui/form-field";
import { Switch } from "@/components/ui/switch";
import { useDaemonConfig } from "@/hooks/use-daemon-config";
import { useHostFeature } from "@/runtime/host-features";
import { useHostRuntimeClient, useHostRuntimeIsConnected } from "@/runtime/host-runtime";
import { SettingsSection } from "@/components/settings/headings/settings-section";
import { resolvePluginPageState } from "@/screens/settings/plugins-page-state";
import { openPluginInstallForm } from "@/screens/settings/plugin-install-form-model";
import { pluginRegistry, useInstalledPlugins } from "@/plugins/registry";
import { pluginDisplayName, usePluginList } from "@/plugins/plugin-list";
import { settingsStyles } from "@/styles/settings";
import { confirmDialog } from "@/utils/confirm-dialog";
import { type PluginRowAction, pluginRowAction } from "./plugins/plugin-types";
import { PluginRow } from "./plugins/plugin-row";
import { PluginLogsSheet } from "./plugins/plugin-logs-sheet";

const PLUGIN_SOURCE_DOCS_URL = "https://paseo.sh/docs/plugins/reference#plugin-sources";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function HostPluginsPage({ serverId }: { serverId: string }) {
  const { t } = useTranslation();
  const client = useHostRuntimeClient(serverId);
  const connected = useHostRuntimeIsConnected(serverId);
  const supported = useHostFeature(serverId, "pluginManagement");
  // COMPAT(pluginSourceInstallation): added in v0.8.0; remove gate after 2027-03-16 once daemon floor supports source identifiers.
  const sourceInstallSupported = useHostFeature(serverId, "pluginSourceInstallation");
  // COMPAT(pluginLogs): added in v0.4.0, remove gate after 2027-08-16.
  const logsSupported = useHostFeature(serverId, "pluginLogs");
  const { config, patchConfig } = useDaemonConfig(serverId);
  const refreshQueue = useRef(Promise.resolve());
  useInstalledPlugins();
  const [installForm] = useState(openPluginInstallForm);
  const installState = useSyncExternalStore(
    installForm.subscribe,
    installForm.getState,
    installForm.getState,
  );
  const sourceDocsLink = useMemo(
    () => (
      <ExternalLink
        href={PLUGIN_SOURCE_DOCS_URL}
        label={t("settings.plugins.docs")}
        testID="plugin-source-docs-link"
      />
    ),
    [t],
  );
  const [logsPlugin, setLogsPlugin] = useState<PluginListItem | null>(null);
  const [feedback, setFeedback] = useState<{ kind: "success" | "error"; message: string } | null>(
    null,
  );
  const plugins = usePluginList(serverId);
  const refetchPlugins = plugins.refetch;
  const refresh = useCallback(async () => {
    if (!client) return;
    const refetch = async () => {
      await refetchPlugins();
    };
    refreshQueue.current = refreshQueue.current.then(refetch, refetch);
    await refreshQueue.current;
  }, [client, refetchPlugins]);

  useEffect(() => {
    return () => installForm.close();
  }, [installForm]);

  useEffect(() => {
    if (!client || !connected || !supported) return;
    const observation = client.observeEvents(["status.plugin_catalog_changed"]);
    observation.subscribe({
      snapshot: () => {},
      update: (message) => {
        if (message.type === "status" && message.payload.status === "plugin_catalog_changed")
          void refresh();
      },
    });
    return () => {
      void observation
        .release()
        .catch((error) => console.warn("[Plugins] Failed to release settings feed", error));
    };
  }, [client, connected, supported, refresh]);

  const mutation = useMutation({
    mutationFn: async (operation: {
      action: string;
      pluginId?: string;
      run: () => Promise<string>;
    }) => operation.run(),
    onSuccess: async (message) => {
      setFeedback({ kind: "success", message });
      await refresh();
    },
    onError: (error) => setFeedback({ kind: "error", message: errorMessage(error) }),
  });

  const install = useCallback(() => {
    if (!client || !sourceInstallSupported || !installState.canSubmit) return;
    const input = installForm.getSubmission();
    setFeedback(null);
    mutation.mutate({
      action: "install",
      run: async () => {
        const installed = await client.installPluginSource(input);
        installForm.reset();
        return t("settings.plugins.feedback.installed", { name: pluginDisplayName(installed) });
      },
    });
  }, [client, installForm, installState.canSubmit, mutation, sourceInstallSupported, t]);

  const action = useCallback(
    (name: PluginRowAction, plugin: PluginListItem) => {
      if (!client) return;
      const pluginName = pluginDisplayName(plugin);
      const run = async () => {
        if (name === "remove") {
          const confirmed = await confirmDialog({
            title: t("settings.plugins.removeConfirmTitle", { name: pluginName }),
            message: t("settings.plugins.removeConfirmMessage"),
            confirmLabel: t("settings.plugins.actions.remove"),
            destructive: true,
          });
          if (!confirmed) return t("settings.plugins.feedback.kept", { name: pluginName });
          await client.removePlugin(plugin.id);
        } else {
          await client[`${name}Plugin`](plugin.id);
        }
        return t(`settings.plugins.feedback.${name}`, { name: pluginName });
      };
      setFeedback(null);
      mutation.mutate({ action: name, pluginId: plugin.id, run });
    },
    [client, mutation, t],
  );

  const toggleGlobal = useCallback(
    (enabled: boolean) => {
      setFeedback(null);
      mutation.mutate({
        action: enabled ? "global-enable" : "global-disable",
        run: async () => {
          await patchConfig({ pluginsEnabled: enabled });
          return enabled
            ? t("settings.plugins.feedback.globalEnabled")
            : t("settings.plugins.feedback.globalDisabled");
        },
      });
    },
    [mutation, patchConfig, t],
  );

  const pageState = resolvePluginPageState({
    connected,
    supported,
    loading: plugins.isLoading,
    error: plugins.isError,
    pluginCount: plugins.data?.length ?? 0,
  });

  const retry = useCallback(() => {
    void refetchPlugins();
  }, [refetchPlugins]);

  const closeLogs = useCallback(() => setLogsPlugin(null), []);

  if (pageState === "offline") {
    return (
      <Alert
        size="md"
        variant="warning"
        title={t("settings.plugins.states.offlineTitle")}
        description={t("settings.plugins.states.offlineDescription")}
      />
    );
  }
  if (pageState === "unsupported") {
    return <Alert size="md" variant="warning" title={t("settings.plugins.states.updateTitle")} />;
  }

  let catalogContent = <Alert size="sm" title={t("settings.plugins.states.loading")} />;
  if (pageState === "error") {
    catalogContent = (
      <Alert
        size="sm"
        variant="error"
        title={t("settings.plugins.states.errorTitle")}
        description={errorMessage(plugins.error)}
      >
        <Button variant="outline" onPress={retry}>
          {t("settings.plugins.states.retry")}
        </Button>
      </Alert>
    );
  } else if (pageState !== "loading") {
    catalogContent = (
      <SettingsCard>
        {plugins.data?.length ? (
          plugins.data.map((plugin) => {
            const clientError = pluginRegistry.getEvaluationError(serverId, plugin.id);
            const pending = mutation.isPending && mutation.variables?.pluginId === plugin.id;
            return (
              <PluginRow
                key={plugin.id}
                serverId={serverId}
                plugin={plugin}
                clientError={clientError}
                pending={mutation.isPending}
                pendingAction={pending ? pluginRowAction(mutation.variables?.action) : undefined}
                onAction={action}
                onOpenLogs={setLogsPlugin}
                supportsLogs={logsSupported}
              />
            );
          })
        ) : (
          <View style={styles.empty}>
            <Text style={settingsStyles.rowHint}>{t("settings.plugins.states.empty")}</Text>
          </View>
        )}
      </SettingsCard>
    );
  }

  return (
    <View>
      <SettingsSection title={t("settings.plugins.title")}>
        <Alert
          size="sm"
          variant="warning"
          title={t("settings.plugins.trustedTitle")}
          description={t("settings.plugins.trustedDescription")}
        />
        <View style={settingsStyles.card}>
          <View style={settingsStyles.row}>
            <View style={settingsStyles.rowContent}>
              <Text style={settingsStyles.rowTitle}>{t("settings.plugins.globalTitle")}</Text>
              <Text style={settingsStyles.rowHint}>{t("settings.plugins.globalHint")}</Text>
            </View>
            <Switch
              value={config?.pluginsEnabled === true}
              onValueChange={toggleGlobal}
              disabled={mutation.isPending}
              accessibilityLabel={t("settings.plugins.globalTitle")}
            />
          </View>
        </View>
        {sourceInstallSupported ? (
          <View style={[settingsStyles.card, styles.install]}>
            <Field label={t("settings.plugins.sourceLabel")} trailing={sourceDocsLink}>
              <FormTextInput
                initialValue=""
                resetKey={installState.resetKey}
                onChangeText={installForm.setSource}
                placeholder={t("settings.plugins.sourcePlaceholder")}
                autoCapitalize="none"
                autoCorrect={false}
                editable={!mutation.isPending}
                accessibilityLabel={t("settings.plugins.sourceLabel")}
              />
            </Field>
            <Button onPress={install} disabled={!installState.canSubmit || mutation.isPending}>
              {mutation.isPending && mutation.variables?.action === "install"
                ? t("settings.plugins.installing")
                : t("settings.plugins.install")}
            </Button>
          </View>
        ) : (
          <Alert
            size="sm"
            variant="warning"
            title={t("settings.plugins.states.sourceUpdateTitle")}
          />
        )}
        {feedback ? (
          <Alert
            size="sm"
            variant={feedback.kind}
            title={feedback.message}
            testID="plugin-management-feedback"
          />
        ) : null}
        {catalogContent}
      </SettingsSection>
      {client && logsPlugin ? (
        <PluginLogsSheet
          client={client}
          plugin={logsPlugin}
          serverId={serverId}
          onClose={closeLogs}
        />
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  install: {
    padding: theme.spacing[4],
    gap: theme.spacing[3],
  },
  empty: {
    padding: theme.spacing[4],
    alignItems: "center",
  },
}));
