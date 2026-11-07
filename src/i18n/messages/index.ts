import type { Catalog, Message, Messages } from "../catalog";
import type { Locale } from "../locales";
import { app } from "./app";
import { common } from "./common";
import { dialogs } from "./dialogs";
import { help } from "./help";
import { memory } from "./memory";
import { settings } from "./settings";
import { table } from "./table";
import { transcript } from "./transcript";

export const CATALOGS = {
  common,
  app,
  table,
  help,
  settings,
  memory,
  dialogs,
  transcript,
} satisfies Record<string, Catalog<Messages>>;

export const EN = {
  ...common.en,
  ...app.en,
  ...table.en,
  ...help.en,
  ...settings.en,
  ...memory.en,
  ...dialogs.en,
  ...transcript.en,
};

export type MessageKey = keyof typeof EN;

/** Every message in `locale`, falling back to English for any that are missing. */
export function messagesFor(locale: Locale): Record<MessageKey, Message> {
  if (locale === "en") return EN;
  const translated = Object.values(CATALOGS).reduce<Messages>(
    (all, catalog) => ({
      ...all,
      ...(catalog.translations[locale] as Messages),
    }),
    {},
  );
  return { ...EN, ...translated } as Record<MessageKey, Message>;
}
