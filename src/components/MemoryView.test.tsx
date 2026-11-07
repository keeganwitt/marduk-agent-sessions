import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../lib/ipc";
import type { MemoryFile, MemoryStore } from "../lib/types";
import { makeSession } from "../test/factories";
import { MemoryView } from "./MemoryView";

vi.mock("../lib/ipc", () => ({
  listMemory: vi.fn(),
  readMemoryFile: vi.fn(),
  revealMemory: vi.fn(),
  trashMemory: vi.fn(),
}));

const listMemory = vi.mocked(ipc.listMemory);

function file(overrides: Partial<MemoryFile>): MemoryFile {
  return {
    path: "/c/projects/-w/memory/git.md",
    title: "git",
    description: null,
    kind: "feedback",
    sessionId: null,
    modified: null,
    size: 1,
    ...overrides,
  };
}

const claude: MemoryStore = {
  agent: "claude",
  agentName: "Claude Code",
  folder: "/c/projects/-w/memory",
  projectDir: "/Users/me/w",
  projectExists: true,
  sessions: 2,
  files: [
    file({
      title: "git",
      description: "Use the personal email",
      sessionId: "s1",
    }),
    file({
      path: "/c/projects/-w/memory/old.md",
      title: "old",
      sessionId: "gone",
    }),
  ],
  problems: ["Not listed in MEMORY.md, so Claude Code won't find it: old.md."],
  note: null,
  canTrash: true,
};

const codex: MemoryStore = {
  agent: "codex",
  agentName: "Codex",
  folder: "/Users/me/.codex/memories",
  projectDir: null,
  projectExists: null,
  sessions: 0,
  files: [
    file({
      path: "/Users/me/.codex/memories/memory_summary.md",
      title: "memory_summary.md",
      kind: "loaded",
    }),
  ],
  problems: [],
  note: "Codex treats these files as generated state.",
  canTrash: false,
};

function renderView(onOpenSession = vi.fn()) {
  const source = makeSession({
    uid: "u1",
    id: "s1",
    title: "Where it was learned",
  });
  render(
    <MemoryView
      sessions={[source]}
      homeDir="/Users/me"
      now={new Date("2026-11-06T12:00:00Z")}
      reloadKey={1}
      onOpenSession={onOpenSession}
    />,
  );
  return source;
}

describe("MemoryView", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    listMemory.mockResolvedValue([claude, codex]);
  });

  it("shows a project's memory, its warnings and the sessions it came from", async () => {
    const user = userEvent.setup();
    const onOpenSession = vi.fn();
    const source = renderView(onOpenSession);

    const detail = await screen.findByRole("region", {
      name: "Memory details",
    });
    expect(
      within(detail).getByRole("heading", { name: "~/w" }),
    ).toBeInTheDocument();
    expect(detail).toHaveTextContent("2 sessions in this project");
    expect(detail).toHaveTextContent("won't find it: old.md");
    expect(within(detail).getByText("Deleted session")).toBeInTheDocument();

    await user.click(
      within(detail).getByRole("button", { name: "Where it was learned" }),
    );
    expect(onOpenSession).toHaveBeenCalledWith(source);
  });

  it("shows Codex memory read-only, with how Codex resets it", async () => {
    const user = userEvent.setup();
    renderView();

    await user.click(
      await screen.findByRole("button", { name: /Codex \(all projects\)/ }),
    );

    const detail = screen.getByRole("region", { name: "Memory details" });
    expect(detail).toHaveTextContent("generated state");
    expect(
      within(detail).queryByRole("button", { name: "Move to Trash…" }),
    ).toBeNull();
  });

  it("reads a memory file in a side panel", async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.readMemoryFile).mockResolvedValue(
      "# Git\n\nUse the **personal** email.",
    );
    renderView();

    await user.click(await screen.findByRole("button", { name: "git" }));

    const panel = await screen.findByRole("dialog", { name: "git" });
    expect(ipc.readMemoryFile).toHaveBeenCalledWith(
      "/c/projects/-w/memory/git.md",
    );
    expect(await within(panel).findByText("personal")).toBeInTheDocument();
  });

  it("moves a project's memory to the Trash after confirming, then reloads", async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.trashMemory).mockResolvedValue();
    renderView();

    await user.click(
      await screen.findByRole("button", { name: "Move to Trash…" }),
    );
    const confirm = screen.getByRole("dialog", {
      name: /Move Claude Code memory for ~\/w/,
    });
    expect(confirm).toHaveTextContent("start its memory over");
    listMemory.mockResolvedValue([codex]);
    await user.click(
      within(confirm).getByRole("button", { name: "Move to Trash" }),
    );

    expect(ipc.trashMemory).toHaveBeenCalledWith(
      "/c/projects/-w/memory",
      false,
    );
    expect(
      await screen.findByText("Moved Claude Code memory for ~/w to the Trash."),
    ).toBeInTheDocument();
    expect(listMemory).toHaveBeenCalledTimes(2);
  });
});
