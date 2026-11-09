import { Fragment, useCallback, useMemo } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { Text, View } from "react-native";
import { Monitor, Moon, Sun } from "lucide-react-native";
import { StyleSheet, withUnistyles } from "react-native-unistyles";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import { DropdownTrigger } from "@/components/ui/dropdown-trigger";
import { DEFAULT_THEME_PREFERENCE, type AppSettings } from "@/hooks/use-settings";
import {
  ICON_SIZE,
  PLUGIN_THEME_PREFERENCE,
  THEME_OPTIONS,
  THEME_SWATCHES,
  type Theme,
} from "@/styles/theme";
import type { PluginThemeOption } from "@/plugins/themes";
import { settingsStyles } from "@/styles/settings";

const ThemedSun = withUnistyles(Sun);
const ThemedMoon = withUnistyles(Moon);
const ThemedMonitor = withUnistyles(Monitor);

const mutedColorMapping = (theme: Theme) => ({ color: theme.colors.foregroundMuted });

export type BuiltInThemePreference = Exclude<AppSettings["theme"], typeof PLUGIN_THEME_PREFERENCE>;

export function getThemeLabel(t: TFunction, value: BuiltInThemePreference): string {
  return t(`settings.appearance.theme.options.${value}`);
}

interface ThemeLeadingProps {
  themeValue: BuiltInThemePreference;
}

function ThemeLeading({ themeValue }: ThemeLeadingProps) {
  switch (themeValue) {
    case "light":
      return <ThemedSun size={ICON_SIZE.md} uniProps={mutedColorMapping} />;
    case "dark":
      return <ThemedMoon size={ICON_SIZE.md} uniProps={mutedColorMapping} />;
    case "auto":
      return <ThemedMonitor size={ICON_SIZE.md} uniProps={mutedColorMapping} />;
    default:
      return <ThemeSwatch color={THEME_SWATCHES[themeValue]} />;
  }
}

interface ThemeSwatchProps {
  color: string;
}

function ThemeSwatch({ color }: ThemeSwatchProps) {
  const swatchStyle = useMemo(() => [styles.swatch, { backgroundColor: color }], [color]);
  return <View style={swatchStyle} />;
}

interface ThemeMenuItemProps {
  themeValue: BuiltInThemePreference;
  selected: boolean;
  onChange: (theme: BuiltInThemePreference) => void;
}

function ThemeMenuItem({ themeValue, selected, onChange }: ThemeMenuItemProps) {
  const { t } = useTranslation();
  const handleSelect = useCallback(() => {
    onChange(themeValue);
  }, [onChange, themeValue]);
  const leading = useMemo(() => <ThemeLeading themeValue={themeValue} />, [themeValue]);
  return (
    <DropdownMenuItem selected={selected} onSelect={handleSelect} leading={leading}>
      {getThemeLabel(t, themeValue)}
    </DropdownMenuItem>
  );
}

interface PluginThemeMenuItemProps {
  option: PluginThemeOption;
  selected: boolean;
  onSelect: (option: PluginThemeOption) => void;
}

function PluginThemeMenuItem({ option, selected, onSelect }: PluginThemeMenuItemProps) {
  const handleSelect = useCallback(() => {
    onSelect(option);
  }, [onSelect, option]);
  const leading = useMemo(() => <ThemeSwatch color={option.swatch} />, [option.swatch]);
  return (
    <DropdownMenuItem selected={selected} onSelect={handleSelect} leading={leading}>
      {option.name}
    </DropdownMenuItem>
  );
}

export interface ThemeRowProps {
  value: AppSettings["theme"];
  pluginThemes: PluginThemeOption[];
  selectedPluginTheme: PluginThemeOption | null;
  onChange: (theme: BuiltInThemePreference) => void;
  onSelectPluginTheme: (option: PluginThemeOption) => void;
}

export function ThemeRow({
  value,
  pluginThemes,
  selectedPluginTheme,
  onChange,
  onSelectPluginTheme,
}: ThemeRowProps) {
  const { t } = useTranslation();
  // A selected contribution that is no longer installed shows the fallback the app renders.
  const builtInValue = value === PLUGIN_THEME_PREFERENCE ? DEFAULT_THEME_PREFERENCE : value;
  const selectedLabel = selectedPluginTheme
    ? selectedPluginTheme.name
    : getThemeLabel(t, builtInValue);
  const leading = useMemo(
    () =>
      selectedPluginTheme ? (
        <ThemeSwatch color={selectedPluginTheme.swatch} />
      ) : (
        <ThemeLeading themeValue={builtInValue} />
      ),
    [builtInValue, selectedPluginTheme],
  );
  return (
    <View style={settingsStyles.row}>
      <View style={settingsStyles.rowContent}>
        <Text style={settingsStyles.rowTitle}>{t("settings.appearance.theme.title")}</Text>
      </View>
      <DropdownMenu>
        <DropdownTrigger
          accessibilityLabel={t("settings.appearance.theme.accessibilityLabel", {
            value: selectedLabel,
          })}
          leading={leading}
        >
          {selectedLabel}
        </DropdownTrigger>
        <DropdownMenuContent side="bottom" align="end" width={200} scrollable>
          {THEME_OPTIONS.map((option, index) => {
            const previousOption = THEME_OPTIONS[index - 1];
            return (
              <Fragment key={option.name}>
                {previousOption && previousOption.group !== option.group ? (
                  <DropdownMenuSeparator />
                ) : null}
                <ThemeMenuItem
                  themeValue={option.name}
                  selected={selectedPluginTheme === null && builtInValue === option.name}
                  onChange={onChange}
                />
              </Fragment>
            );
          })}
          {pluginThemes.length > 0 ? <DropdownMenuSeparator /> : null}
          {pluginThemes.map((option) => (
            <PluginThemeMenuItem
              key={option.id}
              option={option}
              selected={selectedPluginTheme?.id === option.id}
              onSelect={onSelectPluginTheme}
            />
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </View>
  );
}

const styles = StyleSheet.create((theme) => ({
  swatch: {
    width: ICON_SIZE.md,
    height: ICON_SIZE.md,
    borderRadius: ICON_SIZE.md / 2,
    borderWidth: theme.borderWidth[1],
    borderColor: theme.colors.border,
  },
}));
