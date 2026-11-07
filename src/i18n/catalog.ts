import type { Locale } from "./locales";

/**
 * A message: plain text with `{name}` placeholders, or plural forms chosen by `{count}` with
 * `Intl.PluralRules` (`one`, `few`, `many`, … as the language needs; `other` is required).
 */
export type Message =
  string | ({ other: string } & Partial<Record<Intl.LDMLPluralRule, string>>);

export type Messages = Record<string, Message>;

/** Every translation must have exactly the English keys. */
export type Translations<T extends Messages> = Record<
  Exclude<Locale, "en">,
  { [K in keyof T]: Message }
>;

export interface Catalog<T extends Messages> {
  en: T;
  translations: Translations<T>;
}

export function defineMessages<T extends Messages>(
  en: T,
  translations: Translations<T>,
): Catalog<T> {
  return { en, translations };
}

export type Params = Record<string, string | number>;

export function formatMessage(
  message: Message,
  params: Params | undefined,
  locale: Locale,
): string {
  const template =
    typeof message === "string"
      ? message
      : (message[
          new Intl.PluralRules(locale).select(Number(params?.count ?? 0))
        ] ?? message.other);
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    params && name in params ? String(params[name]) : match,
  );
}
