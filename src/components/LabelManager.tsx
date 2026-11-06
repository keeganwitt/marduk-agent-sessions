import { useCallback, useEffect, useState } from "react";
import { deleteLabel, listLabels, updateLabel } from "../lib/ipc";
import { COLOR_NAMES, LABEL_COLORS, labelStyle } from "../lib/labels";
import type { Label } from "../lib/types";

interface Props {
  onChanged: () => void;
  onError: (message: string) => void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function LabelRow({
  label,
  onSave,
  onDelete,
}: {
  label: Label;
  onSave: (name: string, color: string) => Promise<void>;
  onDelete: () => Promise<void>;
}) {
  const [name, setName] = useState(label.name);
  const [confirming, setConfirming] = useState(false);

  const commitName = () => {
    if (name.trim() && name.trim() !== label.name)
      void onSave(name, label.color);
    else setName(label.name);
  };

  return (
    <li className="label-row">
      <span className="label-chip" style={labelStyle(label.color)}>
        {label.name}
      </span>
      <input
        aria-label={`Name of ${label.name}`}
        value={name}
        onChange={(event) => setName(event.target.value)}
        onBlur={commitName}
        onKeyDown={(event) => {
          if (event.key === "Enter") commitName();
        }}
      />
      <span
        className="swatches"
        role="radiogroup"
        aria-label={`Color of ${label.name}`}
      >
        {COLOR_NAMES.map((color) => (
          <button
            key={color}
            type="button"
            role="radio"
            aria-checked={color === label.color}
            aria-label={LABEL_COLORS[color].name}
            className={
              color === label.color ? "swatch-button active" : "swatch-button"
            }
            style={{ background: LABEL_COLORS[color].fill }}
            onClick={() => void onSave(label.name, color)}
          />
        ))}
      </span>
      <span className="muted count">{label.count}</span>
      {confirming ? (
        <span className="confirm">
          <button
            type="button"
            className="danger"
            onClick={() => void onDelete()}
          >
            Delete from {label.count}{" "}
            {label.count === 1 ? "session" : "sessions"}
          </button>
          <button type="button" onClick={() => setConfirming(false)}>
            Keep
          </button>
        </span>
      ) : (
        <button
          type="button"
          onClick={() => setConfirming(true)}
          aria-label={`Delete label ${label.name}`}
        >
          Delete
        </button>
      )}
    </li>
  );
}

/** Rename, recolor and delete labels. Labels are created from the Labels menu. */
export function LabelManager({ onChanged, onError }: Props) {
  const [labels, setLabels] = useState<Label[] | null>(null);

  const reload = useCallback(
    () =>
      listLabels().then(setLabels, (caught: unknown) =>
        onError(errorMessage(caught)),
      ),
    [onError],
  );

  useEffect(() => {
    void reload();
  }, [reload]);

  const run = async (action: () => Promise<void>) => {
    try {
      await action();
      onChanged();
    } catch (caught) {
      onError(errorMessage(caught));
    }
    await reload();
  };

  if (!labels) return <p className="muted">Loading…</p>;
  if (labels.length === 0) {
    return (
      <p className="muted">
        No labels yet. Select sessions and use Labels ▾ to create one.
      </p>
    );
  }
  return (
    <ul className="label-list">
      {labels.map((label) => (
        <LabelRow
          key={`${label.id}:${label.name}`}
          label={label}
          onSave={(name, color) =>
            run(() => updateLabel(label.id, name, color))
          }
          onDelete={() => run(() => deleteLabel(label.id))}
        />
      ))}
    </ul>
  );
}
