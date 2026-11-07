import { useEffect, useState, type KeyboardEvent, type ReactNode } from "react";

import { tildePath } from "../lib/format";
import { agentHelp } from "../lib/ipc";
import type { AgentHelp } from "../lib/types";

interface Props {
  homeDir: string;
  onClose: () => void;
}

const TABS = [
  { id: "deleting", label: "Deleting" },
  { id: "archiving", label: "Archiving" },
  { id: "safety", label: "Safety" },
  { id: "subagents", label: "Subagents & labels" },
  { id: "agents", label: "Agents" },
] as const;

type TabId = (typeof TABS)[number]["id"];

/** Agent descriptions mark commands with backticks, as in Markdown. */
function WithCode({ text }: { text: string }) {
  return (
    <>
      {text
        .split("`")
        .map((part, index) =>
          index % 2 === 1 ? <code key={index}>{part}</code> : part,
        )}
    </>
  );
}

/** Left and right arrows move between tabs, as in native tab bars. */
function arrowStep<T>(
  event: KeyboardEvent,
  items: readonly T[],
  current: T,
  select: (item: T) => void,
  keys: [string, string] = ["ArrowLeft", "ArrowRight"],
) {
  const step = event.key === keys[0] ? -1 : event.key === keys[1] ? 1 : 0;
  if (step === 0) return;
  event.preventDefault();
  const index = items.indexOf(current);
  const next = items[(index + step + items.length) % items.length];
  select(next);
  const target = (
    event.currentTarget as HTMLElement
  ).querySelector<HTMLElement>(`[data-key="${String(next)}"]`);
  target?.focus();
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="help-section">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

function DeletingTab() {
  return (
    <>
      <Section title="Before anything happens">
        <p>
          <strong>Delete…</strong> lists every file that will be moved and every
          command that will run. Nothing changes until you confirm.
        </p>
      </Section>
      <Section title="Trash">
        <p>
          Agents without a delete command of their own have their files moved to
          the system Trash (the Recycle Bin on Windows), where you can restore
          them. If a file can't be moved to the Trash, Marduk reports the error
          rather than deleting it permanently.
        </p>
      </Section>
      <Section title="Permanent">
        <p>
          When an agent has its own delete command, Marduk runs it, so the
          agent's own indexes stay consistent. These deletes can't be undone.
          The confirmation marks them <em>Permanent</em> and shows the exact
          command.
        </p>
      </Section>
      <p className="muted">
        The <strong>Agents</strong> tab says which applies to each agent.
      </p>
    </>
  );
}

function ArchivingTab() {
  return (
    <>
      <Section title="Only through the agent">
        <p>
          <strong>Archive</strong> is offered only for agents with their own
          archive command, and runs that command. The session stays in the
          agent, which can still open it.
        </p>
      </Section>
      <Section title="No invented archives">
        <p>
          Marduk never makes archive folders that an agent doesn't know about.
          For agents without an archive, delete or keep the session instead.
        </p>
      </Section>
    </>
  );
}

function SafetyTab() {
  return (
    <Section title="What Marduk refuses to do">
      <ul>
        <li>
          Change a <em>Running</em> session, one written to in the last 5
          minutes.
        </li>
        <li>
          Change a session that changed after the last scan. Rescan, then try
          again.
        </li>
        <li>Touch files outside the agent's own session folders.</li>
        <li>
          Write to an agent's database. Database-backed agents are changed only
          through their own command-line tool, and are read-only without it.
        </li>
        <li>
          Install or update an agent's command-line tool. Choose where yours is
          in <strong>Settings</strong>.
        </li>
      </ul>
    </Section>
  );
}

function SubagentsTab() {
  return (
    <>
      <Section title="Subagents">
        <p>
          Sessions an agent started on its own are nested under the session that
          started them; expand the parent to see them. Some agents delete them
          along with the parent, and the confirmation says when that happens. A
          subagent whose parent is gone shows <em>Parent deleted</em>.
        </p>
      </Section>
      <Section title="Labels">
        <p>
          Labels are kept by Marduk, not the agent, so they survive archiving.
          Deleting a session in Marduk removes its labels. Labels of sessions
          deleted some other way are removed once the session has been missing
          for 7 days.
        </p>
      </Section>
    </>
  );
}

function AgentsTab({ homeDir }: { homeDir: string }) {
  const [agents, setAgents] = useState<AgentHelp[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    agentHelp().then(setAgents, (caught: unknown) => setError(String(caught)));
  }, []);

  if (error) {
    return (
      <div className="banner error" role="alert">
        {error}
      </div>
    );
  }
  if (!agents) return <p className="muted">Loading…</p>;
  if (agents.length === 0) return <p className="muted">No agents.</p>;

  const ids = agents.map((agent) => agent.id);
  const current = agents.find((agent) => agent.id === selected) ?? agents[0];
  return (
    <div className="agent-help">
      <div
        className="agent-list"
        role="tablist"
        aria-label="Agent"
        aria-orientation="vertical"
        onKeyDown={(event) =>
          arrowStep(event, ids, current.id, setSelected, [
            "ArrowUp",
            "ArrowDown",
          ])
        }
      >
        {agents.map((agent) => (
          <button
            key={agent.id}
            type="button"
            role="tab"
            data-key={agent.id}
            id={`agent-tab-${agent.id}`}
            aria-selected={agent.id === current.id}
            aria-controls="agent-panel"
            tabIndex={agent.id === current.id ? 0 : -1}
            onClick={() => setSelected(agent.id)}
          >
            {agent.name}
          </button>
        ))}
      </div>
      <div
        className="agent-detail"
        role="tabpanel"
        id="agent-panel"
        aria-labelledby={`agent-tab-${current.id}`}
      >
        <Section title="Delete">
          <p>
            <WithCode text={current.delete} />
          </p>
        </Section>
        <Section title="Archive">
          <p>
            <WithCode text={current.archive} />
          </p>
        </Section>
        <Section title="Reads sessions from">
          <ul className="paths">
            {current.roots.map((root) => (
              <li key={root}>
                <code title={root}>{tildePath(root, homeDir)}</code>
              </li>
            ))}
          </ul>
        </Section>
      </div>
    </div>
  );
}

export function HelpDialog({ homeDir, onClose }: Props) {
  const [tab, setTab] = useState<TabId>("deleting");

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="overlay">
      <div
        className="dialog help-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="help-title"
      >
        <h2 id="help-title">How Marduk changes sessions</h2>
        <div
          className="tabs"
          role="tablist"
          aria-label="Help topics"
          onKeyDown={(event) =>
            arrowStep(
              event,
              TABS.map((item) => item.id),
              tab,
              setTab,
            )
          }
        >
          {TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              data-key={item.id}
              id={`help-tab-${item.id}`}
              aria-selected={item.id === tab}
              aria-controls="help-panel"
              tabIndex={item.id === tab ? 0 : -1}
              onClick={() => setTab(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>
        <div
          className="help-panel"
          role="tabpanel"
          id="help-panel"
          aria-labelledby={`help-tab-${tab}`}
        >
          {tab === "deleting" && <DeletingTab />}
          {tab === "archiving" && <ArchivingTab />}
          {tab === "safety" && <SafetyTab />}
          {tab === "subagents" && <SubagentsTab />}
          {tab === "agents" && <AgentsTab homeDir={homeDir} />}
        </div>
        <div className="actions">
          <button type="button" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
