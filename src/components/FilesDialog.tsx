import { useEffect } from "react";
import type { SessionRow } from "../lib/types";
import { FileList } from "./FileList";

interface Props {
  session: SessionRow;
  homeDir: string;
  onReveal: (path: string) => void;
  onClose: () => void;
}

export function FilesDialog({ session, homeDir, onReveal, onClose }: Props) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="overlay">
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="files-title">
        <h2 id="files-title">{session.title ?? session.id}</h2>
        <p className="muted">
          {session.agentName} · {session.id}
        </p>
        {session.database && (
          <div className="database-note">
            <p>
              {session.agentName} keeps all sessions in one shared database, so this session has no files of its own.
              Deleting it removes its rows through {session.agentName}'s own command; the database file stays.
            </p>
            <FileList paths={[session.database]} homeDir={homeDir} onReveal={onReveal} />
          </div>
        )}
        <FileList paths={session.files} homeDir={homeDir} onReveal={onReveal} />
        <div className="actions">
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
