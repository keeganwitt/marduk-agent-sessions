import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { cliStatuses, setCliPath } from "../lib/ipc";
import type { CliStatus } from "../lib/types";

interface Props {
  onClose: (changed: boolean) => void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function CliRow({ status, busy, onChoose, onReset }: {
  status: CliStatus;
  busy: boolean;
  onChoose: () => void;
  onReset: () => void;
}) {
  return (
    <li className="cli-row">
      <div className="cli-heading">
        <strong>{status.agentName}</strong> <code>{status.program}</code>
        {status.overridden ? <span className="badge">Custom location</span> : status.path && <span className="badge muted">Found automatically</span>}
      </div>
      {status.path ? (
        <div className="cli-detail">
          <span className="path">{status.path}</span>
          {status.version && <span className="muted"> · {status.version}</span>}
        </div>
      ) : (
        <div className="cli-detail missing">
          Not found. {status.agentName} sessions are read-only until it's installed or located.{" "}
          <a href={status.installUrl}>How to install {status.program}</a>
        </div>
      )}
      {status.problem && <div className="cli-detail problem">{status.problem}</div>}
      <div className="cli-actions">
        <button type="button" disabled={busy} onClick={onChoose}>
          Choose…
        </button>
        {status.overridden && (
          <button type="button" disabled={busy} onClick={onReset}>
            Use automatic
          </button>
        )}
      </div>
    </li>
  );
}

export function SettingsDialog({ onClose }: Props) {
  const [statuses, setStatuses] = useState<CliStatus[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [changed, setChanged] = useState(false);

  useEffect(() => {
    cliStatuses().then(setStatuses, (caught: unknown) => setError(errorMessage(caught)));
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

  const choose = async (status: CliStatus) => {
    const picked = await open({ multiple: false, directory: false, title: `Choose the ${status.program} CLI` });
    if (typeof picked === "string") await save(status.agent, picked);
  };

  return (
    <div className="overlay">
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <h2 id="settings-title">Settings</h2>
        <h3>Agent CLIs</h3>
        <p className="muted">
          Some agents can only be changed through their own command-line tool. Marduk finds them automatically; choose
          a location here if yours is installed somewhere else.
        </p>
        {error && (
          <div className="banner error" role="alert">
            {error}
          </div>
        )}
        {!statuses && !error && <p className="muted">Checking…</p>}
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
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
