import { useCallback, useMemo } from "react";
import { View } from "react-native";
import { useTranslation } from "react-i18next";
import {
  SettingsCard,
  SettingsSection,
  SettingsSelect,
  SettingsSwitch,
} from "@/components/settings";
import { isNative } from "@/constants/platform";
import { useAppSettings, type ChatDisplay } from "@/hooks/use-settings";

const CHAT_DISPLAYS: readonly ChatDisplay[] = ["full", "collapseToolCalls", "collapseTurns"];

export function ChatSection() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useAppSettings();

  const chatDisplayOptions = useMemo(
    () =>
      CHAT_DISPLAYS.map((value) => ({
        value,
        label: t(`settings.general.chatDisplay.options.${value}`),
      })),
    [t],
  );

  const changeChatDisplay = useCallback(
    (chatDisplay: ChatDisplay) => void updateSettings({ chatDisplay }),
    [updateSettings],
  );
  const changeAutoExpandReasoning = useCallback(
    (autoExpandReasoning: boolean) => void updateSettings({ autoExpandReasoning }),
    [updateSettings],
  );
  const changeChatOutline = useCallback(
    (chatOutlineEnabled: boolean) => void updateSettings({ chatOutlineEnabled }),
    [updateSettings],
  );
  const changeBlockDestructiveCommands = useCallback(
    (blockDestructiveCommands: boolean) => void updateSettings({ blockDestructiveCommands }),
    [updateSettings],
  );
  const changeEnableSelfHealing = useCallback(
    (enableSelfHealing: boolean) => void updateSettings({ enableSelfHealing }),
    [updateSettings],
  );

  return (
    <View>
      <SettingsSection title={t("settings.appearance.detailLevel.title")}>
        <SettingsCard>
          <SettingsSwitch
            label={t("settings.general.autoExpandReasoning.label")}
            hint={t("settings.general.autoExpandReasoning.description")}
            value={settings.autoExpandReasoning}
            onValueChange={changeAutoExpandReasoning}
          />
          <SettingsSelect
            label={t("settings.general.chatDisplay.label")}
            hint={t("settings.general.chatDisplay.description")}
            value={settings.chatDisplay}
            options={chatDisplayOptions}
            onValueChange={changeChatDisplay}
          />
          {isNative ? null : (
            <SettingsSwitch
              label={t("settings.appearance.chatOutline.title")}
              hint={t("settings.appearance.chatOutline.description")}
              value={settings.chatOutlineEnabled}
              onValueChange={changeChatOutline}
            />
          )}
        </SettingsCard>
      </SettingsSection>

      <SettingsSection title="Safety Guardrails & Autonomous Execution">
        <SettingsCard>
          <SettingsSwitch
            label="Block Destructive Commands"
            hint="Automatically block commands that can permanently wipe files, databases, or git history (rm -rf, del /s, git reset --hard)"
            value={settings.blockDestructiveCommands ?? true}
            onValueChange={changeBlockDestructiveCommands}
          />
          <SettingsSwitch
            label="Autonomous Self-Healing Loop"
            hint="Automatically run compile & typecheck probes after agent file edits and auto-repair syntax errors before concluding turns"
            value={settings.enableSelfHealing ?? true}
            onValueChange={changeEnableSelfHealing}
          />
        </SettingsCard>
      </SettingsSection>
    </View>
  );
}
