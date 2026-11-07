import {
  ALL_AGENTS,
  type ArchiveFilter,
  type LabelFilter,
  type ProjectOption,
  type SessionFilter,
} from "../lib/filter";
import { ProjectFilter } from "./ProjectFilter";
import type { AgentSummary, Label } from "../lib/types";

interface Props {
  agents: AgentSummary[];
  labels: Label[];
  projects: ProjectOption[];
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
  homeDir,
  filter,
  onChange,
}: Props) {
  return (
    <div className="filters" role="search">
      <label>
        Agent
        <select
          value={filter.agent}
          onChange={(event) =>
            onChange({ ...filter, agent: event.target.value })
          }
        >
          <option value={ALL_AGENTS}>All agents</option>
          {agents.map((agent) => (
            <option key={agent.id} value={agent.id}>
              {agent.name} ({agent.sessionCount})
            </option>
          ))}
        </select>
      </label>
      <label>
        Status
        <select
          value={filter.archive}
          onChange={(event) =>
            onChange({
              ...filter,
              archive: event.target.value as ArchiveFilter,
            })
          }
        >
          <option value="all">Active and archived</option>
          <option value="active">Active</option>
          <option value="archived">Archived</option>
        </select>
      </label>
      <label>
        Label
        <select
          value={String(filter.label)}
          onChange={(event) =>
            onChange({ ...filter, label: parseLabelFilter(event.target.value) })
          }
        >
          <option value="any">Any label</option>
          <option value="none">No label</option>
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
        Older than (days)
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
        Min size (MB)
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
        Search
        <input
          type="search"
          placeholder="Title or project folder"
          value={filter.query}
          onChange={(event) =>
            onChange({ ...filter, query: event.target.value })
          }
        />
      </label>
    </div>
  );
}
