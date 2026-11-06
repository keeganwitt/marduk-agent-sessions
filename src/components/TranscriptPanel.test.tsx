import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../lib/ipc";
import type { Transcript, TranscriptMessage } from "../lib/types";
import { makeSession } from "../test/factories";
import { TranscriptPanel } from "./TranscriptPanel";

vi.mock("../lib/ipc", () => ({ loadTranscript: vi.fn() }));
const loadTranscript = vi.mocked(ipc.loadTranscript);

function message(
  role: TranscriptMessage["role"],
  text: string,
  tool: TranscriptMessage["tool"] = null,
): TranscriptMessage {
  return { role, text, timestamp: null, tool };
}

function transcriptOf(
  head: TranscriptMessage[],
  extra: Partial<Transcript> = {},
): Transcript {
  return { head, omitted: 0, tail: [], shortened: false, note: null, ...extra };
}

describe("TranscriptPanel", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("renders markdown, tool calls with their output, and hides injected context by default", async () => {
    const user = userEvent.setup();
    loadTranscript.mockResolvedValue(
      transcriptOf([
        message("context", "# AGENTS.md instructions"),
        message(
          "user",
          "What does **src** hold? See [docs](https://example.com/docs)",
        ),
        message("tool", "ls src", { name: "Bash", output: "main.rs" }),
        message("assistant", "Just `main.rs`."),
      ]),
    );

    render(
      <TranscriptPanel
        session={makeSession({ uid: "u1", title: "Explore src" })}
        onClose={() => {}}
      />,
    );

    expect(
      await screen.findByText("src", { selector: "strong" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "docs" })).toHaveAttribute(
      "href",
      "https://example.com/docs",
    );
    expect(
      screen.getByText("main.rs", { selector: "pre" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Bash")).toBeInTheDocument();
    expect(
      screen.queryByText("# AGENTS.md instructions"),
    ).not.toBeInTheDocument();
    expect(loadTranscript).toHaveBeenCalledWith("u1");

    await user.click(screen.getByLabelText("Injected context"));
    expect(screen.getByText("# AGENTS.md instructions")).toBeInTheDocument();

    await user.click(screen.getByLabelText("Tool calls"));
    expect(screen.queryByText("Bash")).not.toBeInTheDocument();
  });

  it("marks omitted middle messages and running sessions", async () => {
    loadTranscript.mockResolvedValue(
      transcriptOf([message("user", "start")], {
        omitted: 42,
        tail: [message("assistant", "end")],
      }),
    );

    render(
      <TranscriptPanel
        session={makeSession({ running: true })}
        onClose={() => {}}
      />,
    );

    expect(
      await screen.findByText(/42 messages in the middle/),
    ).toBeInTheDocument();
    expect(screen.getByText("end")).toBeInTheDocument();
    expect(screen.getByText(/still running/)).toBeInTheDocument();
  });

  it("shows how the transcript was read when it wasn't the usual way", async () => {
    loadTranscript.mockResolvedValue(
      transcriptOf([message("user", "hi")], {
        note: "Read through `opencode session export`, because…",
      }),
    );

    render(<TranscriptPanel session={makeSession()} onClose={() => {}} />);

    expect(
      await screen.findByText(/Read through `opencode session export`/),
    ).toBeInTheDocument();
  });

  it("explains when everything in the session is hidden by the toggles", async () => {
    loadTranscript.mockResolvedValue(
      transcriptOf([message("context", "# AGENTS.md")]),
    );

    render(<TranscriptPanel session={makeSession()} onClose={() => {}} />);

    expect(
      await screen.findByText(/hidden by the filters above/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/no messages to show/)).not.toBeInTheDocument();
  });

  it("shows only the note, not 'no messages', for sessions with nothing readable", async () => {
    loadTranscript.mockResolvedValue(
      transcriptOf([], {
        note: "This session only contains an encrypted context record",
      }),
    );

    render(<TranscriptPanel session={makeSession()} onClose={() => {}} />);

    expect(
      await screen.findByText(/encrypted context record/),
    ).toBeInTheDocument();
    expect(screen.queryByText(/no messages to show/)).not.toBeInTheDocument();
  });

  it("lists prompts to jump to", async () => {
    loadTranscript.mockResolvedValue(
      transcriptOf([
        message("user", "first question"),
        message("assistant", "a"),
        message("user", "second\nquestion"),
      ]),
    );

    render(<TranscriptPanel session={makeSession()} onClose={() => {}} />);

    const jump = await screen.findByRole("combobox", {
      name: "Jump to prompt",
    });
    expect(
      within(jump).getByRole("option", { name: "second question" }),
    ).toBeInTheDocument();
  });

  it("shows load errors and closes on Escape", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    loadTranscript.mockRejectedValue("Couldn't read /x: permission denied");

    render(<TranscriptPanel session={makeSession()} onClose={onClose} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "permission denied",
    );
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });
});
