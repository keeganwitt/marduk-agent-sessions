import { useEffect, useRef, useState, type ReactNode } from "react";

import {
  ALL_AGENT_TYPES,
  ALL_AGENTS,
  ALL_MODELS,
  DEFAULT_FILTERS,
  FILTERS,
  EMPTY_FILTER,
  isDefaultFilterSet,
  isFiltering,
  shownFilters,
  type ArchiveFilter,
  type FilterId,
  type LabelFilter,
  type ProjectOption,
  type SessionFilter,
} from "../lib/filter";
import { useI18n } from "../i18n/context";
import { ProjectFilter } from "./ProjectFilter";
import type { AgentSummary, Label } from "../lib/types";

interface Props {
  agents: AgentSummary[];
  labels: Label[];
  projects: ProjectOption[];
  models: { model: string; count: number }[];
  agentTypes: { agentType: string; count: number }[];
  homeDir: string;
  filter: SessionFilter;
  onChange: (filter: SessionFilter) => void;
  /** Filters chosen for the bar; removed ones come back through "+ Filter". */
  chosen: FilterId[];
  onChosenChange: (chosen: FilterId[]) => void;
}

function parseOptionalNumber(value: string): number | null {
  if (value.trim() === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function parseLabelFilter(value: string): LabelFilter {
  return value === "any" || value === "none" ? value : Number(value);
}

function AddFilterMenu({
  choices,
  onAdd,
  onReset,
}: {
  choices: { id: FilterId; label: string }[];
  onAdd: (id: FilterId) => void;
  /** Absent when the bar already shows the default filters. */
  onReset?: () => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointer = (event: PointerEvent) => {
      if (!anchor.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  if (choices.length === 0 && !onReset) return null;
  return (
    <div className="menu-anchor add-filter" ref={anchor}>
      <button
        type="button"
        className="ghost"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        {t("table.filter.add")}
      </button>
      {open && (
        <div className="filter-menu" role="menu">
          {choices.map((choice) => (
            <button
              key={choice.id}
              type="button"
              role="menuitem"
              onClick={() => {
                onAdd(choice.id);
                setOpen(false);
              }}
            >
              {choice.label}
            </button>
          ))}
          {onReset && (
            <>
              {choices.length > 0 && <hr />}
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  onReset();
                  setOpen(false);
                }}
              >
                {t("table.filter.restoreDefaults")}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}

/** A filter with a button that removes it from the bar and stops it narrowing the list. */
function Removable({
  label,
  onRemove,
  children,
}: {
  label: string;
  onRemove: () => void;
  children: ReactNode;
}) {
  const { t } = useI18n();
  return (
    <div className="removable-filter">
      {children}
      <button
        type="button"
        className="remove-filter"
        aria-label={t("table.filter.remove", { name: label })}
        title={t("table.filter.remove", { name: label })}
        onClick={onRemove}
      >
        ×
      </button>
    </div>
  );
}

export function Filters({
  agents,
  labels,
  projects,
  models,
  agentTypes,
  homeDir,
  filter,
  onChange,
  chosen,
  onChosenChange,
}: Props) {
  const { t } = useI18n();
  const filterNames: Record<FilterId, string> = {
    agent: t("table.filter.agent"),
    olderThanDays: t("table.filter.olderThan"),
    minSizeMb: t("table.filter.minSize"),
    label: t("table.filter.label"),
    project: t("table.filter.project"),
    archive: t("table.filter.status"),
    model: t("table.filter.model"),
    agentType: t("table.filter.agentType"),
    parentMissing: t("table.badge.parentDeleted"),
  };
  const offered = (id: FilterId) =>
    (id !== "model" || models.length > 0) &&
    (id !== "agentType" || agentTypes.length > 0);
  const shown = shownFilters(chosen, filter);
  const remove = (id: FilterId) => {
    onChosenChange(chosen.filter((other) => other !== id));
    onChange({ ...filter, [id]: EMPTY_FILTER[id] });
  };

  const controls: Record<FilterId, ReactNode> = {
    agent: (
      <label>
        {t("table.filter.agent")}
        <select
          value={filter.agent}
          onChange={(event) =>
            onChange({ ...filter, agent: event.target.value })
          }
        >
          <option value={ALL_AGENTS}>{t("table.filter.allAgents")}</option>
          {agents.map((agent) => (
            <option key={agent.id} value={agent.id}>
              {agent.name} ({agent.sessionCount})
            </option>
          ))}
        </select>
      </label>
    ),
    olderThanDays: (
      <label>
        {t("table.filter.olderThan")}
        <input
          type="number"
          min={0}
          inputMode="numeric"
          value={filter.olderThanDays ?? ""}
          onChange={(event) =>
            onChange({
              ...filter,
              olderThanDays: parseOptionalNumber(event.target.value),
            })
          }
        />
      </label>
    ),
    minSizeMb: (
      <label>
        {t("table.filter.minSize")}
        <input
          type="number"
          min={0}
          step="any"
          value={filter.minSizeMb ?? ""}
          onChange={(event) =>
            onChange({
              ...filter,
              minSizeMb: parseOptionalNumber(event.target.value),
            })
          }
        />
      </label>
    ),
    label: (
      <label>
        {t("table.filter.label")}
        <select
          value={String(filter.label)}
          onChange={(event) =>
            onChange({ ...filter, label: parseLabelFilter(event.target.value) })
          }
        >
          <option value="any">{t("table.filter.anyLabel")}</option>
          <option value="none">{t("table.filter.noLabel")}</option>
          {labels.map((label) => (
            <option key={label.id} value={label.id}>
              {label.name} ({label.count})
            </option>
          ))}
        </select>
      </label>
    ),
    project: (
      <ProjectFilter
        options={projects}
        value={filter.project}
        homeDir={homeDir}
        onChange={(project) => onChange({ ...filter, project })}
      />
    ),
    parentMissing: (
      <label className="filter-toggle">
        <input
          type="checkbox"
          checked={filter.parentMissing}
          onChange={(event) =>
            onChange({ ...filter, parentMissing: event.target.checked })
          }
        />
        {t("table.badge.parentDeleted")}
      </label>
    ),
    archive: (
      <label>
        {t("table.filter.status")}
        <select
          value={filter.archive}
          onChange={(event) =>
            onChange({
              ...filter,
              archive: event.target.value as ArchiveFilter,
            })
          }
        >
          <option value="all">{t("table.filter.statusAll")}</option>
          <option value="active">{t("table.filter.statusActive")}</option>
          <option value="archived">{t("table.filter.statusArchived")}</option>
        </select>
      </label>
    ),
    model: (
      <label>
        {t("table.filter.model")}
        <select
          value={filter.model}
          onChange={(event) =>
            onChange({ ...filter, model: event.target.value })
          }
        >
          <option value={ALL_MODELS}>{t("table.filter.allModels")}</option>
          {models.map(({ model, count }) => (
            <option key={model} value={model}>
              {model} ({count})
            </option>
          ))}
        </select>
      </label>
    ),
    agentType: (
      <label>
        {t("table.filter.agentType")}
        <select
          value={filter.agentType}
          onChange={(event) =>
            onChange({ ...filter, agentType: event.target.value })
          }
        >
          <option value={ALL_AGENT_TYPES}>
            {t("table.filter.allAgentTypes")}
          </option>
          {agentTypes.map(({ agentType, count }) => (
            <option key={agentType} value={agentType}>
              {agentType} ({count})
            </option>
          ))}
        </select>
      </label>
    ),
  };

  return (
    <div className="filters" role="search">
      {shown.map((id) => (
        <Removable key={id} label={filterNames[id]} onRemove={() => remove(id)}>
          {controls[id]}
        </Removable>
      ))}
      <AddFilterMenu
        choices={FILTERS.filter((id) => offered(id) && !shown.includes(id)).map(
          (id) => ({ id, label: filterNames[id] }),
        )}
        onAdd={(id) => {
          onChosenChange([...chosen, id]);
          if (id === "parentMissing")
            onChange({ ...filter, parentMissing: true });
        }}
        onReset={
          isDefaultFilterSet(chosen)
            ? undefined
            : () => onChosenChange([...DEFAULT_FILTERS])
        }
      />
      {isFiltering(filter) && (
        <button
          type="button"
          className="ghost clear-filters"
          onClick={() => onChange({ ...EMPTY_FILTER, query: filter.query })}
        >
          {t("table.filter.clear")}
        </button>
      )}
    </div>
  );
}
