import { useEffect } from "react";
import { useI18n } from "../i18n/context";
import type { SessionRow } from "../lib/types";
import { FileList } from "./FileList";

interface Props {
  session: SessionRow;
  homeDir: string;
  onReveal: (path: string) => void;
  onClose: () => void;
}

export function FilesDialog({ session, homeDir, onReveal, onClose }: Props) {
  const { t } = useI18n();
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="overlay">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="files-title"
      >
        <h2 id="files-title">{session.title ?? session.id}</h2>
        <p className="muted">
          {session.agentName} · {session.id}
        </p>
        {session.database && (
          <div className="database-note">
            <p>
              {t("dialogs.files.databaseNote", { agent: session.agentName })}
            </p>
            <FileList
              paths={[session.database]}
              homeDir={homeDir}
              onReveal={onReveal}
            />
          </div>
        )}
        <FileList paths={session.files} homeDir={homeDir} onReveal={onReveal} />
        <div className="actions">
          <button type="button" onClick={onClose}>
            {t("dialogs.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
