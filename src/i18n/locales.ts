/** Languages Marduk is translated into, each named in its own language. */
export const LOCALES = [
  { id: "en", name: "English" },
  { id: "es", name: "Español" },
  { id: "pt-BR", name: "Português (Brasil)" },
  { id: "fr", name: "Français" },
  { id: "de", name: "Deutsch" },
  { id: "ja", name: "日本語" },
  { id: "ko", name: "한국어" },
  { id: "zh-Hans", name: "简体中文" },
  { id: "ru", name: "Русский" },
] as const;

export type Locale = (typeof LOCALES)[number]["id"];

/** The setting: a language, or `system` to follow the OS's preferred languages. */
export type LanguagePref = "system" | Locale;

export function isLocale(value: unknown): value is Locale {
  return LOCALES.some((locale) => locale.id === value);
}

export function isLanguagePref(value: unknown): value is LanguagePref {
  return value === "system" || isLocale(value);
}

/**
 * The first supported language among `preferred` (BCP 47 tags, most preferred first), matching
 * by language when the exact tag isn't supported: any `pt-*` is Brazilian Portuguese and
 * Simplified Chinese covers `zh`, `zh-CN` and `zh-SG`, but not Traditional (`zh-TW`, `zh-HK`).
 */
export function matchLocale(preferred: readonly string[]): Locale {
  for (const tag of preferred) {
    const lower = tag.toLowerCase();
    const exact = LOCALES.find((locale) => locale.id.toLowerCase() === lower);
    if (exact) return exact.id;
    const [language, ...rest] = lower.split(/[-_]/);
    if (language === "zh") {
      const traditional = rest.some((part) =>
        ["hant", "tw", "hk", "mo"].includes(part),
      );
      if (!traditional) return "zh-Hans";
      continue;
    }
    if (language === "pt") return "pt-BR";
    const base = LOCALES.find((locale) => locale.id === language);
    if (base) return base.id;
  }
  return "en";
}

export function resolveLocale(
  pref: LanguagePref,
  preferred: readonly string[],
): Locale {
  return pref === "system" ? matchLocale(preferred) : pref;
}

const STORAGE_KEY = "marduk.language";

/** The last choice, so the first paint is in the right language before settings load. */
export function cachedLanguage(): LanguagePref {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return isLanguagePref(saved) ? saved : "system";
  } catch {
    return "system";
  }
}

export function cacheLanguage(pref: LanguagePref): void {
  try {
    localStorage.setItem(STORAGE_KEY, pref);
  } catch {
    // The setting still applies for this session.
  }
}

export function systemLanguages(): readonly string[] {
  if (typeof navigator === "undefined") return [];
  return navigator.languages?.length
    ? navigator.languages
    : [navigator.language];
}
