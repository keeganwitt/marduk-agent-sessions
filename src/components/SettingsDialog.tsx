import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { cliStatuses, saveTheme, setCliPath } from "../lib/ipc";
import { applyTheme, cachedTheme, THEMES, type Theme } from "../lib/theme";
import { useI18n } from "../i18n/context";
import {
  isLanguagePref,
  LOCALES,
  matchLocale,
  systemLanguages,
} from "../i18n/locales";
import type { MessageKey } from "../i18n/messages";
import { LabelManager } from "./LabelManager";
import { panelId, tabId } from "../lib/tabs";
import { Tabs } from "./Tabs";

type SettingsTab = "appearance" | "labels" | "cli";
import type { CliStatus } from "../lib/types";

interface Props {
  onClose: (changed: boolean) => void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const THEME_LABELS = {
  system: "settings.theme.system",
  light: "settings.theme.light",
  dark: "settings.theme.dark",
} as const satisfies Record<Theme, MessageKey>;

function CliRow({
  status,
  busy,
  onChoose,
  onReset,
}: {
  status: CliStatus;
  busy: boolean;
  onChoose: () => void;
  onReset: () => void;
}) {
  const { t } = useI18n();
  return (
    <li className="cli-row">
      <div className="cli-heading">
        <strong>{status.agentName}</strong> <code>{status.program}</code>
        {status.overridden ? (
          <span className="badge">{t("settings.cli.custom")}</span>
        ) : (
          status.path && (
            <span className="badge muted">{t("settings.cli.automatic")}</span>
          )
        )}
      </div>
      {status.path ? (
        <div className="cli-detail">
          <span className="path">{status.path}</span>
          {status.version && <span className="muted"> · {status.version}</span>}
        </div>
      ) : (
        <div className="cli-detail missing">
          {status.optional
            ? t("settings.cli.missingOptional", { agent: status.agentName })
            : t("settings.cli.missing", { agent: status.agentName })}{" "}
          <a href={status.installUrl}>
            {t("settings.cli.install", { program: status.program })}
          </a>
        </div>
      )}
      {status.problem && (
        <div className="cli-detail problem">{status.problem}</div>
      )}
      <div className="cli-actions">
        <button type="button" disabled={busy} onClick={onChoose}>
          {t("settings.cli.choose")}
        </button>
        {status.overridden && (
          <button type="button" disabled={busy} onClick={onReset}>
            {t("settings.cli.useAutomatic")}
          </button>
        )}
      </div>
    </li>
  );
}

function LanguageSetting() {
  const { language, setLanguage, t } = useI18n();
  const system = matchLocale(systemLanguages());
  const systemName = LOCALES.find((locale) => locale.id === system)!.name;
  return (
    <label className="setting-row">
      <span>{t("language.label")}</span>
      <select
        value={language}
        onChange={(event) => {
          if (isLanguagePref(event.target.value))
            setLanguage(event.target.value);
        }}
      >
        <option value="system">
          {t("language.system", { language: systemName })}
        </option>
        {LOCALES.map((locale) => (
          <option key={locale.id} value={locale.id} lang={locale.id}>
            {locale.name}
          </option>
        ))}
      </select>
    </label>
  );
}

export function SettingsDialog({ onClose }: Props) {
  const { t } = useI18n();
  const [statuses, setStatuses] = useState<CliStatus[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [changed, setChanged] = useState(false);
  const [theme, setTheme] = useState<Theme>(cachedTheme);
  const [tab, setTab] = useState<SettingsTab>("appearance");

  useEffect(() => {
    cliStatuses().then(setStatuses, (caught: unknown) =>
      setError(errorMessage(caught)),
    );
  }, []);

  const close = useCallback(() => onClose(changed), [onClose, changed]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  const save = async (agent: string, path: string | null) => {
    setBusy(true);
    setError(null);
    try {
      setStatuses(await setCliPath(agent, path));
      setChanged(true);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(false);
    }
  };

  const chooseTheme = (next: Theme) => {
    setTheme(next);
    applyTheme(next);
    saveTheme(next).catch((caught: unknown) => setError(errorMessage(caught)));
  };

  const choose = async (status: CliStatus) => {
    const picked = await open({
      multiple: false,
      directory: false,
      title: t("settings.cli.pickTitle", { program: status.program }),
    });
    if (typeof picked === "string") await save(status.agent, picked);
  };

  return (
    <div className="overlay">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
      >
        <h2 id="settings-title">{t("settings.title")}</h2>
        <Tabs
          label={t("settings.title")}
          idPrefix="settings"
          tabs={[
            { id: "appearance", label: t("settings.appearance") },
            { id: "labels", label: t("settings.labels") },
            { id: "cli", label: t("settings.cli.title") },
          ]}
          selected={tab}
          onSelect={setTab}
        />
        {error && (
          <div className="banner error" role="alert">
            {error}
          </div>
        )}
        <div
          className="settings-panel"
          role="tabpanel"
          id={panelId("settings")}
          aria-labelledby={tabId("settings", tab)}
        >
          {tab === "appearance" && (
            <>
              <div className="setting-row">
                <span id="settings-theme-label">{t("settings.theme")}</span>
                <div
                  className="segmented"
                  role="radiogroup"
                  aria-labelledby="settings-theme-label"
                >
                  {THEMES.map((option) => (
                    <label
                      key={option}
                      className={option === theme ? "active" : undefined}
                    >
                      <input
                        type="radio"
                        name="theme"
                        value={option}
                        checked={option === theme}
                        onChange={() => chooseTheme(option)}
                      />
                      {t(THEME_LABELS[option])}
                    </label>
                  ))}
                </div>
              </div>
              <LanguageSetting />
            </>
          )}
          {tab === "labels" && (
            <LabelManager
              onChanged={() => setChanged(true)}
              onError={setError}
            />
          )}
          {tab === "cli" && (
            <>
              <p className="muted">{t("settings.cli.intro")}</p>
              {!statuses && !error && (
                <p className="muted">{t("settings.cli.checking")}</p>
              )}
              {statuses && (
                <ul className="cli-list">
                  {statuses.map((status) => (
                    <CliRow
                      key={status.agent}
                      status={status}
                      busy={busy}
                      onChoose={() => void choose(status)}
                      onReset={() => void save(status.agent, null)}
                    />
                  ))}
                </ul>
              )}
            </>
          )}
        </div>
        <div className="actions">
          <button type="button" onClick={close}>
            {t("settings.done")}
          </button>
        </div>
      </div>
    </div>
  );
}
