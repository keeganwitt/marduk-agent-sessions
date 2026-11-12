import { useEffect, useMemo, useState, type ReactNode } from "react";

import { getLanguage, saveLanguage } from "../lib/ipc";
import { I18nContext, makeI18n } from "./context";
import {
  cacheLanguage,
  cachedLanguage,
  isLanguagePref,
  resolveLocale,
  systemLanguages,
  type LanguagePref,
} from "./locales";

/** Holds the language setting: cached for the first paint, then from the settings file. */
export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, setLanguageState] = useState<LanguagePref>(cachedLanguage);

  useEffect(() => {
    getLanguage().then(
      (saved) => {
        if (!isLanguagePref(saved)) return;
        setLanguageState(saved);
        cacheLanguage(saved);
      },
      () => {},
    );
  }, []);

  const value = useMemo(() => {
    const setLanguage = (next: LanguagePref) => {
      setLanguageState(next);
      cacheLanguage(next);
      void saveLanguage(next).catch(() => {});
    };
    return makeI18n(
      resolveLocale(language, systemLanguages()),
      language,
      setLanguage,
    );
  }, [language]);

  useEffect(() => {
    document.documentElement.lang = value.locale;
  }, [value.locale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}
