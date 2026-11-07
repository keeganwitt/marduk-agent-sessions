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
          {t("settings.cli.missing", { agent: status.agentName })}{" "}
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
      {t("language.label")}
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
        <h3>{t("settings.appearance")}</h3>
        <div
          className="segmented"
          role="radiogroup"
          aria-label={t("settings.theme")}
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
        <LanguageSetting />
        <h3>{t("settings.labels")}</h3>
        <LabelManager onChanged={() => setChanged(true)} onError={setError} />
        <h3>{t("settings.cli.title")}</h3>
        <p className="muted">{t("settings.cli.intro")}</p>
        {error && (
          <div className="banner error" role="alert">
            {error}
          </div>
        )}
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
        <div className="actions">
          <button type="button" onClick={close}>
            {t("settings.done")}
          </button>
        </div>
      </div>
    </div>
  );
}
