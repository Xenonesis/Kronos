import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Alert, Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { RotateCw } from "lucide-react-native";
import { Button } from "@/components/ui/button";
import { useToast } from "@/contexts/toast-api-context";
import { settingsStyles } from "@/styles/settings";
import { useHostRuntimeIsConnected } from "@/runtime/host-runtime";
import { selectHostFeature, useHostFeature } from "@/runtime/host-features";
import { useSessionStore } from "@/stores/session-store";
import { useProvidersSnapshot } from "@/hooks/use-providers-snapshot";
import { useDaemonConfig } from "@/hooks/use-daemon-config";
import { buildProviderDefinitions } from "@/utils/provider-definitions";
import { filterSelectableModels } from "@/provider-selection/model-catalog";
import type { ProviderSnapshotEntry } from "@getpaseo/protocol/agent-types";
import type { AgentProviderDefinition } from "@getpaseo/protocol/provider-manifest";
import {
  buildAcpProviderConfigPatch,
  getAcpProviderCatalog,
  type AcpProviderCatalogItem,
} from "@/hooks/use-acp-provider-catalog";
import { ProviderCatalogList } from "@/components/provider-catalog-list";
import { SettingsSection } from "@/components/settings/headings/settings-section";
import { useProviderSettingsStore } from "@/stores/provider-settings-store";
import { confirmDialog } from "@/utils/confirm-dialog";
import { ProviderRow } from "./providers/provider-row";
import { ProviderRowsSkeleton } from "./providers/provider-skeleton";

export interface ProvidersSectionProps {
  serverId: string;
}

function isProviderInstalled(entry: ProviderSnapshotEntry | undefined): boolean {
  if (!entry) return false;
  if (entry.status === "ready") return true;
  return (filterSelectableModels(entry.models ?? null)?.length ?? 0) > 0;
}

function compareProviderOrder(orderA: number | undefined, orderB: number | undefined): number {
  if (typeof orderA === "number" && typeof orderB === "number") return orderA - orderB;
  if (typeof orderA === "number") return -1;
  if (typeof orderB === "number") return 1;
  return 0;
}

function compareProviderDefinitions(
  a: AgentProviderDefinition,
  b: AgentProviderDefinition,
  entries: ProviderSnapshotEntry[] | undefined,
  configProviders: Record<string, unknown> | undefined,
): number {
  const entryA = entries?.find((candidate) => candidate.provider === a.id);
  const entryB = entries?.find((candidate) => candidate.provider === b.id);

  const aInstalled = isProviderInstalled(entryA);
  const bInstalled = isProviderInstalled(entryB);
  if (aInstalled !== bInstalled) return aInstalled ? -1 : 1;

  const orderA = (configProviders?.[a.id] as { order?: number } | undefined)?.order;
  const orderB = (configProviders?.[b.id] as { order?: number } | undefined)?.order;
  const orderDiff = compareProviderOrder(orderA, orderB);
  if (orderDiff !== 0) return orderDiff;

  const aEnabled = entryA?.enabled ?? true;
  const bEnabled = entryB?.enabled ?? true;
  if (aEnabled !== bEnabled) return aEnabled ? -1 : 1;

  return 0;
}

export function ProvidersSection({ serverId }: ProvidersSectionProps) {
  const { t } = useTranslation();
  const toast = useToast();
  const isConnected = useHostRuntimeIsConnected(serverId);
  const supportsProviderRemoval = useHostFeature(serverId, "providerRemoval");
  const { entries, isLoading, isRefreshing, refresh } = useProvidersSnapshot(serverId);
  const { config, patchConfig } = useDaemonConfig(serverId);
  const openProviderSettings = useProviderSettingsStore((state) => state.open);
  const [pendingProviderId, setPendingProviderId] = useState<string | null>(null);
  const [removingProviderId, setRemovingProviderId] = useState<string | null>(null);
  const removingProviderIdRef = useRef<string | null>(null);
  const [installingProviderId, setInstallingProviderId] = useState<string | null>(null);

  const hasServer = serverId.length > 0;

  // Auto-find installed providers on mount when connected
  useEffect(() => {
    if (hasServer && isConnected) {
      void refresh();
    }
  }, [hasServer, isConnected, refresh]);

  const handleScanPress = useCallback(() => {
    void refresh();
  }, [refresh]);

  // Sort providers: Installed & Available providers appear on TOP
  const providerDefinitions = useMemo(() => {
    const rawDefs = buildProviderDefinitions(entries);
    if (!rawDefs.length) return [];

    return [...rawDefs].sort((a, b) =>
      compareProviderDefinitions(a, b, entries, config?.providers),
    );
  }, [entries, config?.providers]);

  const trailingHeaderAction = useMemo(() => {
    if (!hasServer || !isConnected) return null;
    return (
      <Button
        size="sm"
        variant="ghost"
        leftIcon={RotateCw}
        loading={isRefreshing}
        disabled={isRefreshing}
        onPress={handleScanPress}
        testID="providers-auto-detect-btn"
        accessibilityLabel={t("settings.providers.autoDetect.button")}
      >
        {isRefreshing
          ? t("settings.providers.autoDetect.scanning")
          : t("settings.providers.autoDetect.button")}
      </Button>
    );
  }, [hasServer, isConnected, isRefreshing, handleScanPress, t]);

  const isConfigOwnedProvider = useCallback(
    (providerId: string) =>
      Boolean(
        config?.providers?.[providerId] &&
        (config.providers[providerId].extends ||
          getAcpProviderCatalog().some(
            (entry) => entry.id === providerId && entry.bundledProvider,
          )),
      ),
    [config],
  );

  const handleOpenProviderSettings = useCallback(
    (providerId: string) => {
      openProviderSettings({ serverId, provider: providerId });
    },
    [openProviderSettings, serverId],
  );

  const handleToggleEnabled = useCallback(
    async (providerId: string, enabled: boolean) => {
      setPendingProviderId(providerId);
      try {
        await patchConfig({ providers: { [providerId]: { enabled } } });
      } catch (error) {
        Alert.alert(
          t("settings.providers.updateErrorTitle"),
          error instanceof Error ? error.message : String(error),
        );
      } finally {
        setPendingProviderId((current) => (current === providerId ? null : current));
      }
    },
    [patchConfig, t],
  );

  const handleRemoveProvider = useCallback(
    async (providerId: string, providerLabel: string) => {
      if (removingProviderIdRef.current) return;
      removingProviderIdRef.current = providerId;
      setRemovingProviderId(providerId);
      try {
        const confirmed = await confirmDialog({
          title: t("settings.providers.remove.confirmTitle", { name: providerLabel }),
          message: t("settings.providers.remove.confirmMessage"),
          confirmLabel: t("settings.providers.remove.confirm"),
          destructive: true,
        });
        if (!confirmed) {
          return;
        }

        await patchConfig({ removeProviders: [providerId] });
      } catch (error) {
        Alert.alert(
          t("settings.providers.remove.errorTitle"),
          error instanceof Error ? error.message : String(error),
        );
      } finally {
        if (removingProviderIdRef.current === providerId) {
          removingProviderIdRef.current = null;
        }
        setRemovingProviderId((current) => (current === providerId ? null : current));
      }
    },
    [patchConfig, t],
  );

  const handleInstall = useCallback(
    async (entry: AcpProviderCatalogItem) => {
      if (installingProviderId) return;
      setInstallingProviderId(entry.id);
      try {
        await patchConfig(
          buildAcpProviderConfigPatch(entry, (feature) =>
            selectHostFeature(useSessionStore.getState(), serverId, feature),
          ),
        );
        await refresh([entry.id]);
      } catch (error) {
        toast.error(error instanceof Error ? error.message : String(error));
      } finally {
        setInstallingProviderId((current) => (current === entry.id ? null : current));
      }
    },
    [installingProviderId, patchConfig, refresh, serverId, toast],
  );

  return (
    <>
      <SettingsSection
        title={t("settings.providers.title")}
        testID="host-page-providers-card"
        style={styles.sectionSpacing}
        trailing={trailingHeaderAction}
      >
        {!hasServer || !isConnected ? (
          <View style={[settingsStyles.card, styles.emptyCard]}>
            <Text style={styles.emptyText}>{t("settings.providers.unavailable")}</Text>
          </View>
        ) : null}
        {hasServer && isConnected && isLoading ? <ProviderRowsSkeleton /> : null}
        {hasServer && isConnected && !isLoading && providerDefinitions.length > 0 ? (
          <View style={settingsStyles.card}>
            {providerDefinitions.map((def, index) => {
              const entry = entries?.find((candidate) => candidate.provider === def.id);
              if (!entry) return null;
              return (
                <ProviderRow
                  key={def.id}
                  serverId={serverId}
                  def={def}
                  entry={entry}
                  enabled={entry.enabled ?? true}
                  isToggling={pendingProviderId === def.id}
                  isRemoving={removingProviderId === def.id}
                  canRemove={
                    supportsProviderRemoval &&
                    entry.source === "custom" &&
                    isConfigOwnedProvider(def.id)
                  }
                  isFirst={index === 0}
                  onPress={handleOpenProviderSettings}
                  onToggleEnabled={handleToggleEnabled}
                  onRemove={handleRemoveProvider}
                />
              );
            })}
          </View>
        ) : null}
      </SettingsSection>

      {hasServer && isConnected ? (
        <SettingsSection
          title={t("settings.providers.addProvider")}
          testID="host-page-add-provider-card"
          style={styles.addProviderSection}
        >
          <ProviderCatalogList
            serverId={serverId}
            installingProviderId={installingProviderId}
            onInstall={handleInstall}
          />
        </SettingsSection>
      ) : null}
    </>
  );
}

const styles = StyleSheet.create((theme) => ({
  sectionSpacing: {
    marginBottom: theme.spacing[4],
  },
  addProviderSection: {
    marginTop: theme.spacing[4],
  },
  emptyCard: {
    padding: theme.spacing[4],
    alignItems: "center",
  },
  emptyText: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
}));
