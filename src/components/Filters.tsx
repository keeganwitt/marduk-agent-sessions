import {
  ALL_AGENTS,
  ALL_MODELS,
  type ArchiveFilter,
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
  homeDir: string;
  filter: SessionFilter;
  onChange: (filter: SessionFilter) => void;
}

function parseOptionalNumber(value: string): number | null {
  if (value.trim() === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function parseLabelFilter(value: string): LabelFilter {
  return value === "any" || value === "none" ? value : Number(value);
}

export function Filters({
  agents,
  labels,
  projects,
  models,
  homeDir,
  filter,
  onChange,
}: Props) {
  const { t } = useI18n();
  return (
    <div className="filters" role="search">
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
      {models.length > 0 && (
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
      )}
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
      <ProjectFilter
        options={projects}
        value={filter.project}
        homeDir={homeDir}
        onChange={(project) => onChange({ ...filter, project })}
      />
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
      <label className="grow">
        {t("table.filter.search")}
        <input
          type="search"
          placeholder={t("table.filter.searchPlaceholder")}
          value={filter.query}
          onChange={(event) =>
            onChange({ ...filter, query: event.target.value })
          }
        />
      </label>
    </div>
  );
}
