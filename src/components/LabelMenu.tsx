import { useEffect, useMemo, useRef, useState } from "react";
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
export function LabelMenu({ labels, selected, onApply, onCreate, onClose }: Props) {
  const initial = useMemo(
    () => new Map(labels.map((label) => [label.id, labelState(label.id, selected)])),
    [labels, selected],
  );
  const [states, setStates] = useState<Map<number, LabelState>>(initial);
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

  useEffect(() => {
    setStates((current) => {
      const next = new Map(initial);
      for (const [id, state] of current) if (next.has(id)) next.set(id, state);
      return next;
    });
  }, [initial]);

  const trimmed = query.trim();
  const shown = labels.filter((label) => label.name.toLowerCase().includes(trimmed.toLowerCase()));
  const exists = labels.some((label) => label.name.toLowerCase() === trimmed.toLowerCase());
  const changed = labels.filter((label) => states.get(label.id) !== initial.get(label.id));

  const toggle = (id: number) =>
    setStates((current) => new Map(current).set(id, current.get(id) === "all" ? "none" : "all"));

  const create = async () => {
    const label = await onCreate(trimmed);
    if (label) {
      setStates((current) => new Map(current).set(label.id, "all"));
      setQuery("");
    }
  };

  const apply = () => {
    const add = changed.filter((label) => states.get(label.id) === "all").map((label) => label.id);
    const remove = changed.filter((label) => states.get(label.id) === "none").map((label) => label.id);
    onApply(add, remove);
  };

  return (
    <div className="label-menu" role="dialog" aria-label="Labels">
      <input
        ref={search}
        type="search"
        placeholder="Find or create a label"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && trimmed && !exists) void create();
        }}
      />
      <ul>
        {shown.map((label) => {
          const state = states.get(label.id) ?? "none";
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
                <span className="swatch" style={labelStyle(label.color)} aria-hidden="true" />
                {label.name}
              </label>
            </li>
          );
        })}
        {shown.length === 0 && !trimmed && <li className="muted">No labels yet. Type a name to create one.</li>}
      </ul>
      {trimmed && !exists && (
        <button type="button" className="link create" onClick={() => void create()}>
          Create “{trimmed}”
        </button>
      )}
      <div className="actions">
        <button type="button" onClick={onClose}>
          Cancel
        </button>
        <button type="button" disabled={changed.length === 0} onClick={apply}>
          Apply
        </button>
      </div>
    </div>
  );
}
