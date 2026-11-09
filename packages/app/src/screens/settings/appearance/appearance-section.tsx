import { useCallback, useEffect, useMemo, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { View } from "react-native";
import { StyleSheet } from "react-native-unistyles";
import {
  MAX_CODE_FONT_SIZE,
  MAX_CONTENT_FONT_SIZE,
  MAX_UI_BASE_FONT_SIZE,
  MIN_CODE_FONT_SIZE,
  MIN_CONTENT_FONT_SIZE,
  MIN_UI_BASE_FONT_SIZE,
  parseClampedFontSize,
  sanitizeFontFamily,
  useAppSettings,
  type AppSettings,
} from "@/hooks/use-settings";
import { DEFAULT_MONO_FONT_STACK, DEFAULT_UI_FONT_STACK } from "@/styles/theme";
import { SettingsSection } from "@/components/settings/headings/settings-section";
import { useContributedThemes } from "@/appearance/provider";
import { isNative } from "@/constants/platform";
import { settingsStyles } from "@/styles/settings";
import { AppearancePreview } from "./appearance-preview";
import type { PluginThemeOption } from "@/plugins/themes";
import type { SyntaxThemeId } from "@getpaseo/highlight";
import { ThemeRow, type BuiltInThemePreference } from "./theme-row";
import { FontFamilyRow, FontSizeRow } from "./typography-rows";
import { ContentWidthRow } from "./content-width-row";
import { SyntaxRow } from "./syntax-row";

const BARE_DEFAULT_STACKS: ReadonlySet<string> = new Set(["normal", "monospace"]);

function resolveDefaultStackPlaceholder(t: TFunction, stack: string): string {
  return BARE_DEFAULT_STACKS.has(stack) ? t("settings.appearance.fonts.systemDefault") : stack;
}

// Local size string (digits only) -> preview override number. Empty/invalid
// yields undefined so the preview falls back to the committed theme value.
function sizeDraftToOverride(value: string): number | undefined {
  if (value.length === 0) return undefined;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : undefined;
}

export function AppearanceSection() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useAppSettings();
  const {
    options: pluginThemes,
    selected: selectedPluginTheme,
    select: selectPluginTheme,
  } = useContributedThemes();
  const showInterfaceFontFamilyRow = !isNative;
  const uiFontPlaceholder = resolveDefaultStackPlaceholder(t, DEFAULT_UI_FONT_STACK);
  const monoFontPlaceholder = resolveDefaultStackPlaceholder(t, DEFAULT_MONO_FONT_STACK);

  const [uiFontDraft, setUiFontDraft] = useState(settings.uiFontFamily);
  const [monoFontDraft, setMonoFontDraft] = useState(settings.monoFontFamily);
  const [uiBaseSizeDraft, setUiBaseSizeDraft] = useState(String(settings.uiBaseFontSize));
  const [contentSizeDraft, setContentSizeDraft] = useState(String(settings.contentFontSize));
  const [codeSizeDraft, setCodeSizeDraft] = useState(String(settings.codeFontSize));

  // Resync numeric drafts when the committed value changes elsewhere.
  useEffect(() => {
    setUiBaseSizeDraft(String(settings.uiBaseFontSize));
  }, [settings.uiBaseFontSize]);
  useEffect(() => {
    setContentSizeDraft(String(settings.contentFontSize));
  }, [settings.contentFontSize]);
  useEffect(() => {
    setCodeSizeDraft(String(settings.codeFontSize));
  }, [settings.codeFontSize]);

  const handleThemeChange = useCallback(
    (theme: BuiltInThemePreference) => {
      void updateSettings({ theme });
    },
    [updateSettings],
  );

  const handlePluginThemeChange = useCallback(
    (option: PluginThemeOption) => {
      selectPluginTheme(option);
    },
    [selectPluginTheme],
  );

  const handleContentMaxWidthChange = useCallback(
    (contentMaxWidth: AppSettings["contentMaxWidth"]) => {
      void updateSettings({ contentMaxWidth });
    },
    [updateSettings],
  );

  const handleSyntaxThemeChange = useCallback(
    (syntaxTheme: SyntaxThemeId) => {
      void updateSettings({ syntaxTheme });
    },
    [updateSettings],
  );

  const commitUiFontFamily = useCallback(
    (value: string) => {
      const sanitized = sanitizeFontFamily(value);
      if (sanitized === null) {
        setUiFontDraft(settings.uiFontFamily);
        return settings.uiFontFamily;
      }
      setUiFontDraft(sanitized);
      if (sanitized !== settings.uiFontFamily) {
        void updateSettings({ uiFontFamily: sanitized });
      }
      return sanitized;
    },
    [settings.uiFontFamily, updateSettings],
  );

  const commitMonoFontFamily = useCallback(
    (value: string) => {
      const sanitized = sanitizeFontFamily(value);
      if (sanitized === null) {
        setMonoFontDraft(settings.monoFontFamily);
        return settings.monoFontFamily;
      }
      setMonoFontDraft(sanitized);
      if (sanitized !== settings.monoFontFamily) {
        void updateSettings({ monoFontFamily: sanitized });
      }
      return sanitized;
    },
    [settings.monoFontFamily, updateSettings],
  );

  const handleUiBaseSizeChange = useCallback((value: string) => {
    setUiBaseSizeDraft(value.replace(/[^\d]/g, ""));
  }, []);

  const handleCodeSizeChange = useCallback((value: string) => {
    setCodeSizeDraft(value.replace(/[^\d]/g, ""));
  }, []);

  const handleContentSizeChange = useCallback((value: string) => {
    setContentSizeDraft(value.replace(/[^\d]/g, ""));
  }, []);

  const commitUiBaseSize = useCallback(() => {
    const parsed = parseClampedFontSize(uiBaseSizeDraft, {
      min: MIN_UI_BASE_FONT_SIZE,
      max: MAX_UI_BASE_FONT_SIZE,
    });
    const next = parsed ?? settings.uiBaseFontSize;
    setUiBaseSizeDraft(String(next));
    if (next !== settings.uiBaseFontSize) {
      void updateSettings({ uiBaseFontSize: next });
    }
  }, [settings.uiBaseFontSize, uiBaseSizeDraft, updateSettings]);

  const commitCodeSize = useCallback(() => {
    const parsed = parseClampedFontSize(codeSizeDraft, {
      min: MIN_CODE_FONT_SIZE,
      max: MAX_CODE_FONT_SIZE,
    });
    const next = parsed ?? settings.codeFontSize;
    setCodeSizeDraft(String(next));
    if (next !== settings.codeFontSize) {
      void updateSettings({ codeFontSize: next });
    }
  }, [codeSizeDraft, settings.codeFontSize, updateSettings]);

  const commitContentSize = useCallback(() => {
    const parsed = parseClampedFontSize(contentSizeDraft, {
      min: MIN_CONTENT_FONT_SIZE,
      max: MAX_CONTENT_FONT_SIZE,
    });
    const next = parsed ?? settings.contentFontSize;
    setContentSizeDraft(String(next));
    if (next !== settings.contentFontSize) {
      void updateSettings({ contentFontSize: next });
    }
  }, [contentSizeDraft, settings.contentFontSize, updateSettings]);

  // Live-while-typing: the in-progress drafts drive the preview without
  // committing to the global theme. Empty/invalid fields fall back to the
  // theme value inside the preview.
  const previewOverrides = useMemo(
    () => ({
      contentFontSize: sizeDraftToOverride(contentSizeDraft),
      monoFontFamily: monoFontDraft,
      codeFontSize: sizeDraftToOverride(codeSizeDraft),
    }),
    [codeSizeDraft, contentSizeDraft, monoFontDraft],
  );

  return (
    <View>
      <SettingsSection title={t("settings.appearance.theme.title")}>
        <View style={settingsStyles.card}>
          <ThemeRow
            value={settings.theme}
            pluginThemes={pluginThemes}
            selectedPluginTheme={selectedPluginTheme}
            onChange={handleThemeChange}
            onSelectPluginTheme={handlePluginThemeChange}
          />
        </View>
      </SettingsSection>
      <SettingsSection title={t("settings.appearance.fonts.title")}>
        <View style={settingsStyles.card}>
          {showInterfaceFontFamilyRow ? (
            <FontFamilyRow
              title={t("settings.appearance.fonts.interfaceFont")}
              hint={t("settings.appearance.fonts.interfaceFontHint")}
              accessibilityLabel={t("settings.appearance.fonts.interfaceFontAccessibility")}
              placeholder={uiFontPlaceholder}
              value={settings.uiFontFamily}
              draft={uiFontDraft}
              withBorder={false}
              onChangeDraft={setUiFontDraft}
              onCommit={commitUiFontFamily}
            />
          ) : null}
          <FontSizeRow
            title={t("settings.appearance.fonts.interfaceSize")}
            hint={t("settings.appearance.fonts.interfaceSizeHint")}
            accessibilityLabel={t("settings.appearance.fonts.interfaceSizeAccessibility")}
            draft={uiBaseSizeDraft}
            withBorder={showInterfaceFontFamilyRow}
            onChangeDraft={handleUiBaseSizeChange}
            onCommit={commitUiBaseSize}
          />
          <FontSizeRow
            title={t("settings.appearance.fonts.contentSize")}
            hint={t("settings.appearance.fonts.contentSizeHint")}
            accessibilityLabel={t("settings.appearance.fonts.contentSizeAccessibility")}
            draft={contentSizeDraft}
            onChangeDraft={handleContentSizeChange}
            onCommit={commitContentSize}
          />
          <FontFamilyRow
            title={t("settings.appearance.fonts.codeFont")}
            hint={t("settings.appearance.fonts.codeFontHint")}
            accessibilityLabel={t("settings.appearance.fonts.codeFontAccessibility")}
            placeholder={monoFontPlaceholder}
            value={settings.monoFontFamily}
            draft={monoFontDraft}
            withBorder
            onChangeDraft={setMonoFontDraft}
            onCommit={commitMonoFontFamily}
          />
          <FontSizeRow
            title={t("settings.appearance.fonts.codeSize")}
            hint={t("settings.appearance.fonts.codeSizeHint")}
            accessibilityLabel={t("settings.appearance.fonts.codeSizeAccessibility")}
            draft={codeSizeDraft}
            onChangeDraft={handleCodeSizeChange}
            onCommit={commitCodeSize}
          />
        </View>
      </SettingsSection>
      <SettingsSection title={t("settings.appearance.layout.title")}>
        <View style={settingsStyles.card}>
          <ContentWidthRow
            value={settings.contentMaxWidth}
            onChange={handleContentMaxWidthChange}
          />
        </View>
      </SettingsSection>
      <SettingsSection title={t("settings.appearance.syntax.title")}>
        <View style={settingsStyles.card}>
          <SyntaxRow value={settings.syntaxTheme} onChange={handleSyntaxThemeChange} />
        </View>
        <View style={styles.preview}>
          <AppearancePreview overrides={previewOverrides} />
        </View>
      </SettingsSection>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  preview: {
    marginTop: theme.spacing[4],
  },
}));
