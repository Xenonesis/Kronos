import * as Localization from "expo-localization";
import { type ReactNode, useEffect, useMemo, useState } from "react";
import { View } from "react-native";
import { I18nextProvider } from "react-i18next";
import { isWeb } from "@/constants/platform";
import { useAppSettings } from "@/hooks/use-settings";
import { i18n } from "./i18next";
import { resolveSupportedLocale } from "./locales";
import { ensureI18nLanguageForRender, reportI18nError } from "./sync-language";

interface I18nProviderProps {
  children: ReactNode;
}

function getSystemLocales(): string[] {
  if (isWeb && typeof navigator !== "undefined" && navigator.languages.length > 0) {
    return [...navigator.languages];
  }

  return Localization.getLocales().map((locale) => locale.languageTag);
}

export function I18nProvider({ children }: I18nProviderProps) {
  const { settings } = useAppSettings();
  const systemLocales = useMemo(() => getSystemLocales(), []);
  const locale = resolveSupportedLocale(settings.language, systemLocales);

  const [ready, setReady] = useState(
    () => locale === "en" || i18n.hasResourceBundle(locale, "translation"),
  );

  useEffect(() => {
    if (
      i18n.language === locale &&
      (locale === "en" || i18n.hasResourceBundle(locale, "translation"))
    ) {
      setReady(true);
      return;
    }
    setReady(false);
    i18n
      .changeLanguage(locale)
      .then(() => setReady(true))
      .catch((error) => {
        reportI18nError("[i18n] Failed to load locale " + locale, error);
        setReady(true);
      });
  }, [locale]);

  if (!ready) {
    return <View style={{ flex: 1, backgroundColor: "#0c0d0e" }} />;
  }

  ensureI18nLanguageForRender(locale, i18n);

  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>;
}
