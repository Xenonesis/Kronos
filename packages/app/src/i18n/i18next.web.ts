import { createInstance, type BackendModule } from "i18next";
import { initReactI18next } from "react-i18next";
import { observeI18nInit } from "./init";
import { en } from "./resources/en";

// On web, lazy dynamic import splits non-English locales into on-demand chunks,
// while English stays in the main bundle for immediate offline first paint.
const bundledTranslations: BackendModule = {
  type: "backend",
  init() {},
  read(language, _namespace, callback) {
    if (language === "en") {
      callback(null, en);
      return;
    }
    switch (language) {
      case "ar":
        import("./resources/ar")
          .then((m) => callback(null, m.ar))
          .catch((err) => callback(err, null));
        return;
      case "es":
        import("./resources/es")
          .then((m) => callback(null, m.es))
          .catch((err) => callback(err, null));
        return;
      case "fr":
        import("./resources/fr")
          .then((m) => callback(null, m.fr))
          .catch((err) => callback(err, null));
        return;
      case "ja":
        import("./resources/ja")
          .then((m) => callback(null, m.ja))
          .catch((err) => callback(err, null));
        return;
      case "ko":
        import("./resources/ko")
          .then((m) => callback(null, m.ko))
          .catch((err) => callback(err, null));
        return;
      case "pt-BR":
        import("./resources/pt-BR")
          .then((m) => callback(null, m.ptBR))
          .catch((err) => callback(err, null));
        return;
      case "ru":
        import("./resources/ru")
          .then((m) => callback(null, m.ru))
          .catch((err) => callback(err, null));
        return;
      case "zh-CN":
        import("./resources/zh-CN")
          .then((m) => callback(null, m.zhCN))
          .catch((err) => callback(err, null));
        return;
      default:
        callback(null, undefined);
    }
  },
};

const i18n = createInstance();

observeI18nInit(
  i18n
    .use(bundledTranslations)
    .use(initReactI18next)
    .init({
      compatibilityJSON: "v4",
      fallbackLng: "en",
      lng: "en",
      partialBundledLanguages: true,
      resources: {
        en: { translation: en },
      },
      interpolation: {
        escapeValue: false,
      },
      react: {
        useSuspense: false,
      },
    }),
);

export { i18n };
