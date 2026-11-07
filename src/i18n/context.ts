import { createContext, useContext } from "react";

import { ageParts, formatBytes } from "../lib/format";
import { formatMessage, type Params } from "./catalog";
import type { LanguagePref, Locale } from "./locales";
import { messagesFor, type MessageKey } from "./messages";

export interface I18n {
  locale: Locale;
  /** The setting, which may be `system`. */
  language: LanguagePref;
  setLanguage: (language: LanguagePref) => void;
  t: (key: MessageKey, params?: Params) => string;
  /** "3h ago", in the current language. */
  age: (isoTimestamp: string, now: Date) => string;
  bytes: (bytes: number) => string;
  dateTime: (isoTimestamp: string) => string;
}

export function makeI18n(
  locale: Locale,
  language: LanguagePref = locale,
  setLanguage: (language: LanguagePref) => void = () => {},
): I18n {
  const messages = messagesFor(locale);
  const t = (key: MessageKey, params?: Params) =>
    formatMessage(messages[key], params, locale);
  return {
    locale,
    language,
    setLanguage,
    t,
    age: (isoTimestamp, now) => {
      const { unit, count } = ageParts(isoTimestamp, now);
      return t(`age.${unit}`, { count });
    },
    bytes: (bytes) => formatBytes(bytes, locale),
    dateTime: (isoTimestamp) => new Date(isoTimestamp).toLocaleString(locale),
  };
}

export const I18nContext = createContext<I18n>(makeI18n("en"));

export function useI18n(): I18n {
  return useContext(I18nContext);
}
