import { useEffect, useState, type KeyboardEvent, type ReactNode } from "react";

import { useI18n } from "../i18n/context";
import type { MessageKey } from "../i18n/messages";
import { tildePath } from "../lib/format";
import { agentHelp, appVersion } from "../lib/ipc";
import type { AgentHelp } from "../lib/types";

interface Props {
  homeDir: string;
  onClose: () => void;
}

const TABS = [
  { id: "deleting", label: "help.tab.deleting" },
  { id: "archiving", label: "help.tab.archiving" },
  { id: "safety", label: "help.tab.safety" },
  { id: "subagents", label: "help.tab.subagents" },
  { id: "agents", label: "help.tab.agents" },
] as const satisfies readonly { id: string; label: MessageKey }[];

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

const RICH_TAGS = { strong: "strong", em: "em", code: "code" } as const;

/** A message with `<strong>`, `<em>` and `<code>` tags, rendered as that markup. */
function Rich({ text }: { text: string }) {
  const parts = text.split(/(<(strong|em|code)>[\s\S]*?<\/\2>)/g);
  const nodes: ReactNode[] = [];
  for (let index = 0; index < parts.length; index++) {
    const part = parts[index];
    const match = /^<(strong|em|code)>([\s\S]*)<\/\1>$/.exec(part);
    if (match) {
      const Tag = RICH_TAGS[match[1] as keyof typeof RICH_TAGS];
      nodes.push(<Tag key={index}>{match[2]}</Tag>);
      index += 1;
    } else if (part) {
      nodes.push(part);
    }
  }
  return <>{nodes}</>;
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
  const { t } = useI18n();
  return (
    <>
      <Section title={t("help.deleting.before.title")}>
        <p>
          <Rich text={t("help.deleting.before.body")} />
        </p>
      </Section>
      <Section title={t("help.deleting.trash.title")}>
        <p>{t("help.deleting.trash.body")}</p>
      </Section>
      <Section title={t("help.deleting.permanent.title")}>
        <p>
          <Rich text={t("help.deleting.permanent.body")} />
        </p>
      </Section>
      <p className="muted">
        <Rich text={t("help.deleting.seeAgents")} />
      </p>
    </>
  );
}

function ArchivingTab() {
  const { t } = useI18n();
  return (
    <>
      <Section title={t("help.archiving.agent.title")}>
        <p>
          <Rich text={t("help.archiving.agent.body")} />
        </p>
      </Section>
      <Section title={t("help.archiving.none.title")}>
        <p>{t("help.archiving.none.body")}</p>
      </Section>
    </>
  );
}

function SafetyTab() {
  const { t } = useI18n();
  return (
    <Section title={t("help.safety.title")}>
      <ul>
        <li>
          <Rich text={t("help.safety.running")} />
        </li>
        <li>{t("help.safety.changed")}</li>
        <li>{t("help.safety.outside")}</li>
        <li>{t("help.safety.database")}</li>
        <li>
          <Rich text={t("help.safety.install")} />
        </li>
      </ul>
    </Section>
  );
}

function SubagentsTab() {
  const { t } = useI18n();
  return (
    <>
      <Section title={t("help.subagents.title")}>
        <p>
          <Rich text={t("help.subagents.body")} />
        </p>
      </Section>
      <Section title={t("help.labels.title")}>
        <p>{t("help.labels.body")}</p>
      </Section>
    </>
  );
}

function AgentsTab({ homeDir }: { homeDir: string }) {
  const { t } = useI18n();
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
  if (!agents) return <p className="muted">{t("help.agents.loading")}</p>;
  if (agents.length === 0)
    return <p className="muted">{t("help.agents.none")}</p>;

  const ids = agents.map((agent) => agent.id);
  const current = agents.find((agent) => agent.id === selected) ?? agents[0];
  return (
    <div className="agent-help">
      <div
        className="agent-list"
        role="tablist"
        aria-label={t("help.agents.list")}
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
        <Section title={t("help.agents.delete")}>
          <p>
            <WithCode text={current.delete} />
          </p>
        </Section>
        <Section title={t("help.agents.archive")}>
          <p>
            <WithCode text={current.archive} />
          </p>
        </Section>
        <Section title={t("help.agents.reads")}>
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
  const { t } = useI18n();
  const [tab, setTab] = useState<TabId>("deleting");
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    appVersion().then(setVersion, () => {});
  }, []);

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
        <h2 id="help-title">{t("help.title")}</h2>
        <div
          className="tabs"
          role="tablist"
          aria-label={t("help.topics")}
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
              {t(item.label)}
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
          {version && <span className="help-version">Marduk {version}</span>}
          <button type="button" onClick={onClose}>
            {t("help.done")}
          </button>
        </div>
      </div>
    </div>
  );
}
