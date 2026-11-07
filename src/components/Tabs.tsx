import type { KeyboardEvent } from "react";

import { panelId, tabId } from "../lib/tabs";

interface Props<T extends string> {
  label: string;
  /** Prefix for element ids, so each tab can name the panel it controls. */
  idPrefix: string;
  tabs: readonly { id: T; label: string }[];
  selected: T;
  onSelect: (id: T) => void;
}

/** A horizontal tab bar; left and right arrows move between tabs, as in native tab bars. */
export function Tabs<T extends string>({
  label,
  idPrefix,
  tabs,
  selected,
  onSelect,
}: Props<T>) {
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step =
      event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const index = tabs.findIndex((tab) => tab.id === selected);
    const next = tabs[(index + step + tabs.length) % tabs.length];
    onSelect(next.id);
    event.currentTarget
      .querySelector<HTMLElement>(`#${CSS.escape(tabId(idPrefix, next.id))}`)
      ?.focus();
  };
  return (
    <div
      className="tabs"
      role="tablist"
      aria-label={label}
      onKeyDown={onKeyDown}
    >
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          id={tabId(idPrefix, tab.id)}
          aria-selected={tab.id === selected}
          aria-controls={panelId(idPrefix)}
          tabIndex={tab.id === selected ? 0 : -1}
          onClick={() => onSelect(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
