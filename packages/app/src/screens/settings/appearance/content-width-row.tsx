import { useCallback, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import { Button } from "@/components/ui/button";
import {
  EditingTextInput as TextInput,
  type EditingTextInputHandle,
} from "@/components/ui/text-input";
import { settingsStyles } from "@/styles/settings";
import {
  parseContentMaxWidth,
  resolveContentMaxWidth,
  type AppSettings,
} from "@/hooks/use-settings";

export interface ContentWidthRowProps {
  value: AppSettings["contentMaxWidth"];
  onChange: (value: AppSettings["contentMaxWidth"]) => void;
}

export function ContentWidthRow({ value, onChange }: ContentWidthRowProps) {
  const { t } = useTranslation();
  const width = resolveContentMaxWidth({ contentMaxWidth: value });
  // The field is uncontrolled, so a saved or reset width is written into it directly.
  const input = useRef<EditingTextInputHandle>(null);

  useEffect(() => {
    input.current?.replaceText(String(width));
  }, [width]);

  const commit = useCallback(() => {
    const next = parseContentMaxWidth(input.current?.getText()) ?? width;
    input.current?.replaceText(String(next));
    // Typing the width already in effect keeps following the default.
    if (next !== width) {
      onChange(next);
    }
  }, [onChange, width]);

  const reset = useCallback(() => {
    onChange(null);
  }, [onChange]);

  return (
    <View style={settingsStyles.row}>
      <View style={settingsStyles.rowContent}>
        <Text style={settingsStyles.rowTitle}>{t("settings.appearance.layout.contentWidth")}</Text>
        <Text style={settingsStyles.rowHint}>
          {t("settings.appearance.layout.contentWidthHint")}
        </Text>
      </View>
      <View style={styles.sizeField}>
        {value === null ? null : (
          <Button
            variant="ghost"
            size="sm"
            onPress={reset}
            accessibilityLabel={t("settings.appearance.layout.resetAccessibility")}
          >
            {t("settings.appearance.layout.reset")}
          </Button>
        )}
        <TextInput
          ref={input}
          initialValue={String(width)}
          onBlur={commit}
          onSubmitEditing={commit}
          keyboardType="number-pad"
          inputMode="numeric"
          selectTextOnFocus
          style={styles.widthInput}
          accessibilityLabel={t("settings.appearance.layout.contentWidthAccessibility")}
        />
        <Text style={styles.unit}>px</Text>
      </View>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  sizeField: {
    flexDirection: "row",
    alignItems: "center",
    gap: theme.spacing[2],
  },
  widthInput: {
    width: 80,
    minHeight: 36,
    paddingVertical: theme.spacing[2],
    paddingHorizontal: theme.spacing[3],
    borderRadius: theme.borderRadius.md,
    borderWidth: theme.borderWidth[1],
    borderColor: theme.colors.border,
    backgroundColor: theme.colors.surface2,
    color: theme.colors.foreground,
    fontSize: theme.fontSize.base,
    textAlign: "right",
  },
  unit: {
    color: theme.colors.foregroundMuted,
    fontSize: theme.fontSize.base,
  },
}));
