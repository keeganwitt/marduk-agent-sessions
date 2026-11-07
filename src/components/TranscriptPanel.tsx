import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/context";
import type { MessageKey } from "../i18n/messages";
import { loadTranscript } from "../lib/ipc";
import type {
  Label,
  SessionRow,
  Transcript,
  TranscriptMessage,
} from "../lib/types";
import { LabelChips } from "./LabelChip";
import { Markdown } from "./Markdown";

interface Props {
  session: SessionRow;
  labelsById?: ReadonlyMap<number, Label>;
  onClose: () => void;
}

const ROLE_LABELS: Record<TranscriptMessage["role"], MessageKey> = {
  user: "transcript.role.user",
  assistant: "transcript.role.assistant",
  tool: "transcript.role.tool",
  context: "transcript.role.context",
};

function MessageView({
  message,
  id,
}: {
  message: TranscriptMessage;
  id: string;
}) {
  const { t, dateTime } = useI18n();
  const timestamp = message.timestamp ? dateTime(message.timestamp) : null;
  if (message.tool) {
    return (
      <details className="message tool" id={id}>
        <summary>
          <span className="tool-name">{message.tool.name}</span>
          {timestamp && <span className="when">{timestamp}</span>}
        </summary>
        {message.text && <pre>{message.text}</pre>}
        {message.tool.output !== null && (
          <>
            <div className="tool-output-label">{t("transcript.output")}</div>
            <pre>{message.tool.output}</pre>
          </>
        )}
      </details>
    );
  }
  if (message.role === "context") {
    return (
      <details className="message context" id={id}>
        <summary>{t(ROLE_LABELS.context)}</summary>
        <pre>{message.text}</pre>
      </details>
    );
  }
  return (
    <article className={`message ${message.role}`} id={id}>
      <header>
        <span className="role">{t(ROLE_LABELS[message.role])}</span>
        {timestamp && <span className="when">{timestamp}</span>}
      </header>
      <Markdown text={message.text} />
    </article>
  );
}

function promptLabel(text: string): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > 80 ? `${flat.slice(0, 79)}…` : flat;
}

export function TranscriptPanel({
  session,
  labelsById = new Map(),
  onClose,
}: Props) {
  const { t } = useI18n();
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showTools, setShowTools] = useState(true);
  const [showContext, setShowContext] = useState(false);
  const body = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    loadTranscript(session.uid).then(
      (loaded) => !cancelled && setTranscript(loaded),
      (caught: unknown) =>
        !cancelled &&
        setError(caught instanceof Error ? caught.message : String(caught)),
    );
    return () => {
      cancelled = true;
    };
  }, [session.uid]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const visible = (message: TranscriptMessage) =>
    (showTools || message.role !== "tool") &&
    (showContext || message.role !== "context");
  const sections = transcript
    ? [
        { key: "head", messages: transcript.head },
        { key: "tail", messages: transcript.tail },
      ]
    : [];
  const prompts = sections.flatMap((section) =>
    section.messages.flatMap((message, index) =>
      message.role === "user"
        ? [{ id: `${section.key}-${index}`, label: promptLabel(message.text) }]
        : [],
    ),
  );

  const jumpTo = (id: string) => {
    body.current
      ?.querySelector(`#${CSS.escape(id)}`)
      ?.scrollIntoView?.({ block: "start" });
  };

  return (
    <aside
      className="transcript"
      role="dialog"
      aria-modal="false"
      aria-labelledby="transcript-title"
    >
      <header className="transcript-header">
        <div className="transcript-heading">
          <h2 id="transcript-title">{session.title ?? session.id}</h2>
          <LabelChips ids={session.labelIds} labelsById={labelsById} />
          <p className="muted">
            {session.agentName}
            {session.projectDir && ` · ${session.projectDir}`}
          </p>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label={t("transcript.closeLabel")}
        >
          {t("transcript.close")}
        </button>
      </header>
      <div className="transcript-controls">
        <label>
          <input
            type="checkbox"
            checked={showTools}
            onChange={(event) => setShowTools(event.target.checked)}
          />
          {t("transcript.toolCalls")}
        </label>
        <label>
          <input
            type="checkbox"
            checked={showContext}
            onChange={(event) => setShowContext(event.target.checked)}
          />
          {t("transcript.role.context")}
        </label>
        {prompts.length > 1 && (
          <select
            aria-label={t("transcript.jumpLabel")}
            value=""
            onChange={(event) => jumpTo(event.target.value)}
          >
            <option value="" disabled>
              {t("transcript.jumpCount", { count: prompts.length })}
            </option>
            {prompts.map((prompt) => (
              <option key={prompt.id} value={prompt.id}>
                {prompt.label}
              </option>
            ))}
          </select>
        )}
      </div>
      <div className="transcript-body" ref={body}>
        {error && (
          <div className="banner error" role="alert">
            {error}
          </div>
        )}
        {!transcript && !error && (
          <p className="muted">{t("transcript.loading")}</p>
        )}
        {transcript &&
          transcript.head.length + transcript.tail.length === 0 &&
          !transcript.note && (
            <p className="muted">{t("transcript.noMessages")}</p>
          )}
        {transcript &&
          transcript.head.length + transcript.tail.length > 0 &&
          !transcript.head.concat(transcript.tail).some(visible) && (
            <p className="muted">{t("transcript.allHidden")}</p>
          )}
        {transcript?.note && <p className="muted note">{transcript.note}</p>}
        {session.running && (
          <p className="muted note">{t("transcript.running")}</p>
        )}
        {transcript?.shortened && (
          <p className="muted note">{t("transcript.shortened")}</p>
        )}
        {transcript?.head.map(
          (message, index) =>
            visible(message) && (
              <MessageView
                key={`head-${index}`}
                id={`head-${index}`}
                message={message}
              />
            ),
        )}
        {transcript && transcript.omitted > 0 && (
          <p className="omitted">
            {t("transcript.omitted", { count: transcript.omitted })}
          </p>
        )}
        {transcript?.tail.map(
          (message, index) =>
            visible(message) && (
              <MessageView
                key={`tail-${index}`}
                id={`tail-${index}`}
                message={message}
              />
            ),
        )}
      </div>
    </aside>
  );
}
