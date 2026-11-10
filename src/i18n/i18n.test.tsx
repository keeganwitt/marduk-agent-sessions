import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../lib/ipc";
import { formatMessage, type Message } from "./catalog";
import { makeI18n, useI18n } from "./context";
import { I18nProvider } from "./I18n";
import { LOCALES, matchLocale, resolveLocale } from "./locales";
import { CATALOGS, EN } from "./messages";

vi.mock("../lib/ipc", () => ({
  getLanguage: vi.fn(),
  saveLanguage: vi.fn(),
}));

function placeholders(message: Message): string[] {
  const texts =
    typeof message === "string" ? [message] : Object.values(message);
  return [
    ...new Set(texts.flatMap((text) => text.match(/\{\w+\}/g) ?? [])),
  ].sort();
}

describe("locales", () => {
  it("matches the system's preferred languages, falling back to English", () => {
    expect(matchLocale(["de-AT", "en-US"])).toBe("de");
    expect(matchLocale(["pt-PT"])).toBe("pt-BR");
    expect(matchLocale(["zh-CN"])).toBe("zh-Hans");
    expect(matchLocale(["zh-Hans-SG"])).toBe("zh-Hans");
    expect(matchLocale(["zh-TW", "ja-JP"])).toBe("ja");
    expect(matchLocale(["nl-NL"])).toBe("en");
    expect(matchLocale([])).toBe("en");
    expect(resolveLocale("system", ["ko-KR"])).toBe("ko");
    expect(resolveLocale("fr", ["ko-KR"])).toBe("fr");
  });
});

describe("formatMessage", () => {
  it("fills placeholders and picks plural forms for the language", () => {
    const sessions = { one: "{count} session", other: "{count} sessions" };
    expect(formatMessage(sessions, { count: 1 }, "en")).toBe("1 session");
    expect(formatMessage(sessions, { count: 3 }, "en")).toBe("3 sessions");
    const russian = {
      one: "{count} сессия",
      few: "{count} сессии",
      many: "{count} сессий",
      other: "{count} сессии",
    };
    expect(formatMessage(russian, { count: 2 }, "ru")).toBe("2 сессии");
    expect(formatMessage(russian, { count: 5 }, "ru")).toBe("5 сессий");
    expect(formatMessage("Hi {name}", { name: "Ada" }, "en")).toBe("Hi Ada");
    expect(formatMessage("Hi {name}", undefined, "en")).toBe("Hi {name}");
  });
});

describe("catalogs", () => {
  const keys = Object.values(CATALOGS).flatMap((catalog) =>
    Object.keys(catalog.en),
  );

  it("never define the same key twice", () => {
    expect(keys.length).toBe(new Set(keys).size);
  });

  it("translate every message with the same placeholders as English", () => {
    for (const catalog of Object.values(CATALOGS)) {
      for (const locale of LOCALES.filter((locale) => locale.id !== "en")) {
        const translated = catalog.translations[
          locale.id as Exclude<typeof locale.id, "en">
        ] as Record<string, Message>;
        expect(Object.keys(translated).sort()).toEqual(
          Object.keys(catalog.en).sort(),
        );
        for (const [key, english] of Object.entries(catalog.en)) {
          const message = translated[key];
          expect(placeholders(message), `${locale.id} ${key}`).toEqual(
            placeholders(english as Message),
          );
          const texts =
            typeof message === "string" ? [message] : Object.values(message);
          expect(
            texts.every((text) => text.trim() !== ""),
            `${locale.id} ${key}`,
          ).toBe(true);
        }
      }
    }
  });

  it("formats ages and sizes in the chosen language", () => {
    const now = new Date("2026-11-06T12:00:00Z");
    expect(makeI18n("en").age("2026-11-06T09:00:00Z", now)).toBe("3h ago");
    expect(makeI18n("ja").age("2026-11-06T09:00:00Z", now)).toBe("3時間前");
    expect(makeI18n("de").bytes(1.5 * 1024 * 1024)).toBe("1,5 MB");
    expect(Object.keys(EN)).toContain("age.now");
  });
});

function Probe() {
  const { t, setLanguage, locale } = useI18n();
  return (
    <button type="button" lang={locale} onClick={() => setLanguage("de")}>
      {t("language.label")}
    </button>
  );
}

describe("I18nProvider", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.clear();
    vi.mocked(ipc.saveLanguage).mockResolvedValue();
  });

  it("uses the saved language and saves a new choice", async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.getLanguage).mockResolvedValue("fr");
    render(
      <I18nProvider>
        <Probe />
      </I18nProvider>,
    );

    expect(
      await screen.findByRole("button", { name: "Langue" }, { timeout: 5000 }),
    ).toBeInTheDocument();
    await waitFor(() => expect(document.documentElement.lang).toBe("fr"));

    await user.click(screen.getByRole("button"));

    expect(screen.getByRole("button", { name: "Sprache" })).toBeInTheDocument();
    expect(ipc.saveLanguage).toHaveBeenCalledWith("de");
    expect(localStorage.getItem("marduk.language")).toBe("de");
  });
});
