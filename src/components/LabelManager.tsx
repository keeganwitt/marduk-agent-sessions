import { useCallback, useEffect, useState } from "react";
import { useI18n } from "../i18n/context";
import type { MessageKey } from "../i18n/messages";
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
  const { t } = useI18n();
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
        aria-label={t("settings.labels.nameOf", { name: label.name })}
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
        aria-label={t("settings.labels.colorOf", { name: label.name })}
      >
        {COLOR_NAMES.map((color) => (
          <button
            key={color}
            type="button"
            role="radio"
            aria-checked={color === label.color}
            aria-label={t(`settings.color.${color}` as MessageKey)}
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
            {t("settings.labels.deleteFrom", { count: label.count })}
          </button>
          <button type="button" onClick={() => setConfirming(false)}>
            {t("settings.labels.keep")}
          </button>
        </span>
      ) : (
        <button
          type="button"
          onClick={() => setConfirming(true)}
          aria-label={t("settings.labels.deleteLabel", { name: label.name })}
        >
          {t("settings.labels.delete")}
        </button>
      )}
    </li>
  );
}

/** Rename, recolor and delete labels. Labels are created from the Labels menu. */
export function LabelManager({ onChanged, onError }: Props) {
  const { t } = useI18n();
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

  if (!labels) return <p className="muted">{t("settings.labels.loading")}</p>;
  if (labels.length === 0) {
    return <p className="muted">{t("settings.labels.empty")}</p>;
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
