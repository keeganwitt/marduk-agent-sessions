import { useEffect, useState } from "react";
import { formatBytes, shortenPath } from "../lib/format";
import { fileDetails } from "../lib/ipc";
import { sharedFolder, splitPath } from "../lib/paths";
import type { FileDetail } from "../lib/types";

interface Props {
  paths: string[];
  homeDir: string;
  onReveal: (path: string) => void;
}

function FileIcon({ folder }: { folder: boolean }) {
  return folder ? (
    <svg className="file-icon" viewBox="0 0 16 16" aria-hidden="true">
      <path
        d="M1.5 3.5h4.2l1.5 1.5h7.3v8h-13z"
        fill="currentColor"
        opacity="0.85"
      />
    </svg>
  ) : (
    <svg className="file-icon" viewBox="0 0 16 16" aria-hidden="true">
      <path
        d="M3.5 1.5h6l3 3v10h-9z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
      />
      <path
        d="M9.5 1.5v3h3"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
      />
    </svg>
  );
}

/**
 * Session files as one row each: icon, name, size and "Show in folder". The folder they
 * share is shown once above them, shortened; each row's full path is in its tooltip.
 */
export function FileList({ paths, homeDir, onReveal }: Props) {
  const [details, setDetails] = useState<Map<string, FileDetail>>(new Map());
  const pathsKey = paths.join("\n");

  useEffect(() => {
    let cancelled = false;
    fileDetails(pathsKey === "" ? [] : pathsKey.split("\n")).then(
      (found) =>
        !cancelled &&
        setDetails(new Map(found.map((detail) => [detail.path, detail]))),
      () => {},
    );
    return () => {
      cancelled = true;
    };
  }, [pathsKey]);

  if (paths.length === 0) return null;
  const folder = sharedFolder(paths);

  return (
    <div className="file-list">
      {folder !== null && (
        <div className="file-folder" title={folder}>
          {shortenPath(folder, homeDir)}
        </div>
      )}
      <ul>
        {paths.map((path) => {
          const { dir, name } = splitPath(path);
          const detail = details.get(path);
          return (
            <li key={path} title={path}>
              <FileIcon folder={detail?.isDir ?? false} />
              <span className="file-text">
                <span className="file-name">
                  {name}
                  {detail?.isDir && "/"}
                </span>
                {folder === null && (
                  <span className="file-dir">{shortenPath(dir, homeDir)}</span>
                )}
              </span>
              <span className="file-size">
                {detail
                  ? detail.exists
                    ? formatBytes(detail.sizeBytes)
                    : "missing"
                  : ""}
              </span>
              <button
                type="button"
                className="link"
                onClick={() => onReveal(path)}
              >
                Show in folder
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
