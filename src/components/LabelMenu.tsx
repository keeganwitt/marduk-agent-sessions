import { useEffect, useMemo, useRef, useState } from "react";
import { useI18n } from "../i18n/context";
import { labelState, labelStyle, type LabelState } from "../lib/labels";
import type { Label } from "../lib/types";

interface Props {
  labels: Label[];
  selected: { labelIds: number[] }[];
  onApply: (add: number[], remove: number[]) => void;
  onCreate: (name: string) => Promise<Label | null>;
  onClose: () => void;
}

/**
 * Gmail-style label menu for the selected sessions: each label's checkbox starts checked,
 * unchecked or partly checked, and only labels the user changes are applied.
 */
export function LabelMenu({
  labels,
  selected,
  onApply,
  onCreate,
  onClose,
}: Props) {
  const { t } = useI18n();
  const initial = useMemo(
    () =>
      new Map(
        labels.map((label) => [label.id, labelState(label.id, selected)]),
      ),
    [labels, selected],
  );
  /** Only the labels the user changed; everything else keeps its initial state. */
  const [changes, setChanges] = useState<Map<number, LabelState>>(new Map());
  const stateOf = (id: number): LabelState =>
    changes.get(id) ?? initial.get(id) ?? "none";
  const [query, setQuery] = useState("");
  const search = useRef<HTMLInputElement>(null);

  useEffect(() => {
    search.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const trimmed = query.trim();
  const shown = labels.filter((label) =>
    label.name.toLowerCase().includes(trimmed.toLowerCase()),
  );
  const exists = labels.some(
    (label) => label.name.toLowerCase() === trimmed.toLowerCase(),
  );
  const changed = labels.filter(
    (label) => stateOf(label.id) !== (initial.get(label.id) ?? "none"),
  );

  const toggle = (id: number) =>
    setChanges((current) =>
      new Map(current).set(id, stateOf(id) === "all" ? "none" : "all"),
    );

  const create = async () => {
    const label = await onCreate(trimmed);
    if (label) {
      setChanges((current) => new Map(current).set(label.id, "all"));
      setQuery("");
    }
  };

  const apply = () => {
    const add = changed
      .filter((label) => stateOf(label.id) === "all")
      .map((label) => label.id);
    const remove = changed
      .filter((label) => stateOf(label.id) === "none")
      .map((label) => label.id);
    onApply(add, remove);
  };

  return (
    <div
      className="label-menu"
      role="dialog"
      aria-label={t("settings.labelMenu.title")}
    >
      <input
        ref={search}
        type="search"
        placeholder={t("settings.labelMenu.placeholder")}
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && trimmed && !exists) void create();
        }}
      />
      <ul>
        {shown.map((label) => {
          const state = stateOf(label.id);
          return (
            <li key={label.id}>
              <label>
                <input
                  type="checkbox"
                  checked={state === "all"}
                  ref={(element) => {
                    if (element) element.indeterminate = state === "some";
                  }}
                  onChange={() => toggle(label.id)}
                />
                <span
                  className="swatch"
                  style={labelStyle(label.color)}
                  aria-hidden="true"
                />
                {label.name}
              </label>
            </li>
          );
        })}
        {shown.length === 0 && !trimmed && (
          <li className="muted">{t("settings.labelMenu.empty")}</li>
        )}
      </ul>
      {trimmed && !exists && (
        <button
          type="button"
          className="link create"
          onClick={() => void create()}
        >
          {t("settings.labelMenu.create", { name: trimmed })}
        </button>
      )}
      <div className="actions">
        <button type="button" onClick={onClose}>
          {t("settings.labelMenu.cancel")}
        </button>
        <button type="button" disabled={changed.length === 0} onClick={apply}>
          {t("settings.labelMenu.apply")}
        </button>
      </div>
    </div>
  );
}
