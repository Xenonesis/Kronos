import { useCallback, useMemo } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { Pressable, Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { MoreVertical, Plus } from "lucide-react-native";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { SettingsGroup } from "@/components/settings/headings/settings-group";
import { settingsStyles } from "@/styles/settings";
import type { ProjectScriptDraft } from "@/utils/project-config-form";

const ICON_SIZE = 14;

interface ProjectScriptsSectionProps {
  scripts: ProjectScriptDraft[];
  onAdd: () => void;
  onEdit: (script: ProjectScriptDraft) => void;
  onRemove: (script: ProjectScriptDraft) => void;
}

export function ProjectScriptsSection({
  scripts,
  onAdd,
  onEdit,
  onRemove,
}: ProjectScriptsSectionProps) {
  const { t } = useTranslation();
  const scriptsTrailing = useMemo(
    () => (
      <Pressable
        onPress={onAdd}
        hitSlop={8}
        style={settingsStyles.sectionHeaderLink}
        accessibilityRole="button"
        accessibilityLabel={t("settings.project.scripts.actions.add")}
        testID="scripts-add-button"
      >
        <Plus size={ICON_SIZE} color={styles.iconColor.color} />
      </Pressable>
    ),
    [onAdd, t],
  );

  return (
    <SettingsGroup
      title={t("settings.project.scripts.title")}
      info={t("settings.project.scripts.info")}
      trailing={scriptsTrailing}
      testID="scripts-group"
    >
      <View style={settingsStyles.card} testID="scripts-list">
        {scripts.length === 0 ? (
          <View style={settingsStyles.row}>
            <Text style={styles.emptyScripts}>{t("settings.project.scripts.empty")}</Text>
          </View>
        ) : (
          scripts.map((script, index) => (
            <ScriptRow
              key={script.id}
              script={script}
              isFirst={index === 0}
              onEdit={onEdit}
              onRemove={onRemove}
            />
          ))
        )}
      </View>
    </SettingsGroup>
  );
}

interface ScriptRowProps {
  script: ProjectScriptDraft;
  isFirst: boolean;
  onEdit: (script: ProjectScriptDraft) => void;
  onRemove: (script: ProjectScriptDraft) => void;
}

function ScriptRow({ script, isFirst, onEdit, onRemove }: ScriptRowProps) {
  const { t } = useTranslation();
  const handleEdit = useCallback(() => onEdit(script), [onEdit, script]);
  const handleRemove = useCallback(() => onRemove(script), [onRemove, script]);
  const rowStyle = isFirst ? styles.scriptRow : styles.scriptRowWithBorder;

  return (
    <View style={rowStyle} testID={`script-row-${script.id}`}>
      <Pressable style={styles.scriptRowMain} onPress={handleEdit}>
        <Text style={settingsStyles.rowTitle} numberOfLines={1}>
          {script.name || t("settings.project.scripts.untitled")}
        </Text>
        <Text style={settingsStyles.rowHint} numberOfLines={1}>
          {scriptHint(script, t)}
        </Text>
      </Pressable>
      <DropdownMenu>
        <DropdownMenuTrigger
          accessibilityLabel={t("settings.project.scripts.menuAccessibility")}
          testID={`script-row-menu-${script.id}`}
          style={styles.scriptKebab}
        >
          <MoreVertical size={ICON_SIZE} color={styles.chevronColor.color} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" minWidth={160}>
          <DropdownMenuItem testID={`script-action-${script.id}-edit`} onSelect={handleEdit}>
            {t("settings.project.scripts.actions.edit")}
          </DropdownMenuItem>
          <DropdownMenuItem
            testID={`script-action-${script.id}-remove`}
            destructive
            onSelect={handleRemove}
          >
            {t("settings.project.scripts.actions.remove")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </View>
  );
}

function scriptHint(script: ProjectScriptDraft, t: TFunction): string {
  const pieces: string[] = [];
  if (script.type) pieces.push(script.type);
  if (script.portText) pieces.push(t("settings.project.scripts.port", { port: script.portText }));
  if (script.commandText) pieces.push(script.commandText.split("\n")[0] ?? "");
  return pieces.join(" · ");
}

const styles = StyleSheet.create((theme) => ({
  iconColor: {
    color: theme.colors.foregroundMuted,
  },
  emptyScripts: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
  scriptRow: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingVertical: theme.spacing[4],
    paddingHorizontal: theme.spacing[4],
  },
  scriptRowWithBorder: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingVertical: theme.spacing[4],
    paddingHorizontal: theme.spacing[4],
    borderTopWidth: 1,
    borderTopColor: theme.colors.border,
  },
  scriptRowMain: {
    flex: 1,
    minWidth: 0,
    gap: theme.spacing[1],
  },
  scriptKebab: {
    padding: theme.spacing[1],
  },
  chevronColor: {
    color: theme.colors.foregroundMuted,
  },
}));
