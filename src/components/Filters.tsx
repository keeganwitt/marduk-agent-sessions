import { ALL_AGENTS, type SessionFilter } from "../lib/filter";
import type { AgentSummary } from "../lib/types";

interface Props {
  agents: AgentSummary[];
  filter: SessionFilter;
  onChange: (filter: SessionFilter) => void;
}

function parseOptionalNumber(value: string): number | null {
  if (value.trim() === "") return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

export function Filters({ agents, filter, onChange }: Props) {
  return (
    <div className="filters" role="search">
      <label>
        Agent
        <select value={filter.agent} onChange={(event) => onChange({ ...filter, agent: event.target.value })}>
          <option value={ALL_AGENTS}>All agents</option>
          {agents.map((agent) => (
            <option key={agent.id} value={agent.id}>
              {agent.name} ({agent.sessionCount})
            </option>
          ))}
        </select>
      </label>
      <label>
        Older than (days)
        <input
          type="number"
          min={0}
          inputMode="numeric"
          value={filter.olderThanDays ?? ""}
          onChange={(event) => onChange({ ...filter, olderThanDays: parseOptionalNumber(event.target.value) })}
        />
      </label>
      <label>
        Min size (MB)
        <input
          type="number"
          min={0}
          step="any"
          value={filter.minSizeMb ?? ""}
          onChange={(event) => onChange({ ...filter, minSizeMb: parseOptionalNumber(event.target.value) })}
        />
      </label>
      <label className="grow">
        Search
        <input
          type="search"
          placeholder="Title or project folder"
          value={filter.query}
          onChange={(event) => onChange({ ...filter, query: event.target.value })}
        />
      </label>
    </div>
  );
}
