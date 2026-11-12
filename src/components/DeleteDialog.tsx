import { Fragment, useState, type ReactNode } from "react";

import { useI18n } from "../i18n/context";
import { progressPercent, tildePath } from "../lib/format";
import type {
  MemoryStore,
  PreviewItem,
  ProjectPurge,
  TaskProgress,
} from "../lib/types";
import { FileList } from "./FileList";

interface Props {
  items: PreviewItem[];
  /** Memory no session would use after this delete, offered for the Trash too. */
  memory?: MemoryStore[];
  /** Projects this delete empties, which the agent's own command could purge afterwards. */
  purges?: ProjectPurge[];
  busy: boolean;
  /** How far the confirmed delete has got, while it runs. */
  progress?: TaskProgress | null;
  onConfirm: (
    uids: string[],
    memoryFolders: string[],
    purges: ProjectPurge[],
  ) => void;
  onCancel: () => void;
  onReveal: (path: string) => void;
  homeDir: string;
}

/** A translated sentence with `{name}` placeholders filled by React nodes, in its own order. */
function withNodes(
  text: string,
  nodes: Record<string, ReactNode>,
): ReactNode[] {
  return text.split(/(\{\w+\})/).map((part, index) => {
    const name = /^\{(\w+)\}$/.exec(part)?.[1];
    return name !== undefined && name in nodes ? (
      <Fragment key={index}>{nodes[name]}</Fragment>
    ) : (
      part
    );
  });
}

export function DeleteDialog({
  items,
  memory = [],
  purges = [],
  busy,
  progress = null,
  onConfirm,
  onCancel,
  onReveal,
  homeDir,
}: Props) {
  const { t } = useI18n();
  const percent = progressPercent(progress);
  const [memoryChosen, setMemoryChosen] = useState<ReadonlySet<string>>(
    new Set(),
  );
  const toggleMemory = (folder: string, chosen: boolean) =>
    setMemoryChosen((current) => {
      const next = new Set(current);
      if (chosen) next.add(folder);
      else next.delete(folder);
      return next;
    });
  const [purgesChosen, setPurgesChosen] = useState<ReadonlySet<string>>(
    new Set(),
  );
  const togglePurge = (folder: string, chosen: boolean) =>
    setPurgesChosen((current) => {
      const next = new Set(current);
      if (chosen) next.add(folder);
      else next.delete(folder);
      return next;
    });
  const ready = items.filter((item) => item.refusedReason === null);
  const refused = items.filter((item) => item.refusedReason !== null);
  const toTrash = ready.filter((item) => !item.permanent);
  const permanent = ready.filter((item) => item.permanent);
  const title =
    permanent.length === 0
      ? t("dialogs.delete.titleTrash", { count: ready.length })
      : t("dialogs.delete.titleDelete", { count: ready.length });

  return (
    <div className="overlay">
      <div
        className="dialog dialog-scrollable"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-title"
      >
        <h2 id="delete-title">{title}</h2>
        <div className="dialog-body">
          {toTrash.length > 0 && (
            <p>{t("dialogs.delete.trashNote", { count: toTrash.length })}</p>
          )}
          {permanent.length > 0 && (
            <p className="permanent-warning" role="alert">
              {withNodes(
                t("dialogs.delete.permanentNote", { count: permanent.length }),
                {
                  permanently: (
                    <strong>{t("dialogs.delete.permanently")}</strong>
                  ),
                },
              )}
            </p>
          )}
          {ready.length > 0 && (
            <ul className="preview-list">
              {ready.map((item) => (
                <li key={item.uid}>
                  <div className="preview-title">
                    <strong>{item.title ?? item.uid}</strong>
                    {item.agentName && (
                      <span className="muted"> · {item.agentName}</span>
                    )}
                    {item.permanent && (
                      <span className="badge danger">
                        {t("dialogs.delete.permanentBadge")}
                      </span>
                    )}
                  </div>
                  {item.command && (
                    <code className="command">{item.command}</code>
                  )}
                  {item.warning && (
                    <p className="item-warning">{item.warning}</p>
                  )}
                  <FileList
                    paths={item.files}
                    homeDir={homeDir}
                    onReveal={onReveal}
                  />
                </li>
              ))}
            </ul>
          )}
          {memory.length > 0 && ready.length > 0 && (
            <div className="memory-offers">
              <h3>{t("dialogs.delete.memoryHeading")}</h3>
              {memory.map((store) => (
                <div key={store.folder} className="memory-offer">
                  <label>
                    <input
                      type="checkbox"
                      checked={memoryChosen.has(store.folder)}
                      onChange={(event) =>
                        toggleMemory(store.folder, event.target.checked)
                      }
                    />
                    {withNodes(
                      t("dialogs.delete.memoryOffer", {
                        agent: store.agentName,
                        count: store.files.length,
                      }),
                      {
                        path: (
                          <code>
                            {store.projectDir
                              ? tildePath(store.projectDir, homeDir)
                              : tildePath(store.folder, homeDir)}
                          </code>
                        ),
                      },
                    )}
                  </label>
                  <p className="muted">
                    {store.projectExists === false
                      ? t("dialogs.delete.memoryProjectGone")
                      : t("dialogs.delete.memoryProjectExists", {
                          agent: store.agentName,
                        })}
                  </p>
                </div>
              ))}
            </div>
          )}
          {purges.length > 0 && ready.length > 0 && (
            <div className="memory-offers">
              <h3>{t("dialogs.delete.purgeHeading")}</h3>
              {purges.map((purge) => (
                <div key={purge.folder} className="memory-offer">
                  <label>
                    <input
                      type="checkbox"
                      checked={purgesChosen.has(purge.folder)}
                      onChange={(event) =>
                        togglePurge(purge.folder, event.target.checked)
                      }
                    />
                    {withNodes(
                      t("dialogs.delete.purgeOffer", {
                        agent: purge.agentName,
                      }),
                      {
                        path: (
                          <code>{tildePath(purge.projectDir, homeDir)}</code>
                        ),
                      },
                    )}
                    <span className="badge danger">
                      {t("dialogs.delete.permanentBadge")}
                    </span>
                  </label>
                  <code className="command">{purge.command}</code>
                  <p className="muted">
                    {t("dialogs.delete.purgeNote", { agent: purge.agentName })}
                  </p>
                </div>
              ))}
            </div>
          )}
          {refused.length > 0 && (
            <div className="refused">
              <h3>{t("dialogs.delete.skipped", { count: refused.length })}</h3>
              <ul>
                {refused.map((item) => (
                  <li key={item.uid}>
                    <strong>{item.title ?? item.uid}</strong>:{" "}
                    {item.refusedReason}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
        <div className="actions">
          {busy && progress && percent !== null && (
            <>
              <progress
                className="task-progress"
                max={progress.total}
                value={progress.done}
                aria-label={t("dialogs.delete.progress")}
              />
              <span className="task-percent">{percent}%</span>
            </>
          )}
          <button type="button" onClick={onCancel} disabled={busy}>
            {t("dialogs.cancel")}
          </button>
          <button
            type="button"
            className="danger"
            onClick={() =>
              onConfirm(
                ready.map((item) => item.uid),
                memory
                  .map((store) => store.folder)
                  .filter((folder) => memoryChosen.has(folder)),
                purges.filter((purge) => purgesChosen.has(purge.folder)),
              )
            }
            disabled={busy || ready.length === 0}
          >
            {busy
              ? t("dialogs.working")
              : permanent.length > 0
                ? t("dialogs.delete.confirmDelete")
                : t("dialogs.delete.confirmTrash")}
          </button>
        </div>
      </div>
    </div>
  );
}
