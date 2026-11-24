import { useEffect, useState } from "react";
import { useI18n } from "../i18n/context";
import { terminalSettings, setTerminal } from "../lib/ipc";
import type {
  TerminalApp,
  TerminalPreference,
  TerminalSettings,
} from "../lib/types";

const TERMINAL_NAMES: Record<TerminalApp, string> = {
  macTerminal: "Terminal.app",
  iTerm2: "iTerm2",
  cmux: "cmux",
  windowsTerminal: "Windows Terminal",
  powerShell: "Windows PowerShell",
  gnomeTerminal: "GNOME Terminal",
  konsole: "Konsole",
  xfceTerminal: "Xfce Terminal",
  kitty: "kitty",
  alacritty: "Alacritty",
  xterm: "xterm",
  ghostty: "Ghostty",
  wezTerm: "WezTerm",
  mintty: "mintty (Cygwin)",
};

export function TerminalSetting({
  onError,
}: {
  onError: (message: string | null) => void;
}) {
  const { t } = useI18n();
  const [settings, setSettings] = useState<TerminalSettings | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let mounted = true;
    terminalSettings().then(
      (result) => {
        if (mounted) setSettings(result);
      },
      (caught: unknown) => {
        if (mounted) onError(String(caught));
      },
    );
    return () => {
      mounted = false;
    };
  }, [onError]);

  const save = async (terminal: TerminalPreference) => {
    setBusy(true);
    onError(null);
    try {
      await setTerminal(terminal);
      setSettings((current) => current && { ...current, selected: terminal });
    } catch (caught) {
      onError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      setBusy(false);
    }
  };

  if (!settings) return null;

  const automatic = settings.automatic;
  return (
    <label className="setting-row">
      <span>{t("resume.terminalSetting")}</span>
      <select
        value={settings.selected}
        disabled={busy}
        onChange={(event) => {
          const next = event.target.value;
          const option = settings.options.find(
            (candidate) => candidate === next,
          );
          if (next === "automatic" || option) void save(option ?? "automatic");
        }}
      >
        <option value="automatic">
          {automatic
            ? t("resume.automaticNamed", {
                terminal: TERMINAL_NAMES[automatic],
              })
            : t("resume.automatic")}
        </option>
        {settings.options.map((terminal) => (
          <option key={terminal} value={terminal}>
            {TERMINAL_NAMES[terminal]}
          </option>
        ))}
      </select>
    </label>
  );
}
