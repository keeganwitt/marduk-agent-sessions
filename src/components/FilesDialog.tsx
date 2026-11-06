import { useEffect } from "react";
import type { SessionRow } from "../lib/types";

interface Props {
  session: SessionRow;
  onReveal: (path: string) => void;
  onClose: () => void;
}

export function FilesDialog({ session, onReveal, onClose }: Props) {
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
        <ul className="files">
          {session.files.map((file) => (
            <li key={file}>
              <span className="path">{file}</span>
              <button type="button" className="link" onClick={() => onReveal(file)}>
                Show in folder
              </button>
            </li>
          ))}
        </ul>
        <div className="actions">
          <button type="button" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
