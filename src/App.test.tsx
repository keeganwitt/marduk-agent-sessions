import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import * as ipc from "./lib/ipc";
import type { PreviewItem, ScanResponse, SessionRow } from "./lib/types";
import { makeSession } from "./test/factories";

vi.mock("./lib/ipc", () => ({
  scanSessions: vi.fn(),
  previewDelete: vi.fn(),
  deleteSessions: vi.fn(),
  toggleArchive: vi.fn(),
  revealSessionFile: vi.fn(),
  openProjectDir: vi.fn(),
  loadTranscript: vi.fn(),
}));

const scanSessions = vi.mocked(ipc.scanSessions);
const previewDelete = vi.mocked(ipc.previewDelete);
const deleteSessions = vi.mocked(ipc.deleteSessions);
const toggleArchive = vi.mocked(ipc.toggleArchive);
const revealSessionFile = vi.mocked(ipc.revealSessionFile);
const openProjectDir = vi.mocked(ipc.openProjectDir);
const loadTranscript = vi.mocked(ipc.loadTranscript);

function rowTitles(): string[] {
  return screen
    .getAllByRole("row")
    .slice(1)
    .map((row) => (within(row).getAllByRole("cell")[1].textContent ?? "").trim());
}

function scanOf(...sessions: SessionRow[]): ScanResponse {
  return {
    agents: [{ id: "claude", name: "Claude Code", complete: true, errors: [], sessionCount: sessions.length }],
    sessions,
    homeDir: "/Users/me",
  };
}

function previewOf(overrides: Partial<PreviewItem> & { uid: string }): PreviewItem {
  return {
    title: overrides.uid,
    agentName: "Claude Code",
    files: [`/p/${overrides.uid}.jsonl`],
    permanent: false,
    command: null,
    warning: null,
    refusedReason: null,
    ...overrides,
  };
}

describe("App", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("previews and trashes selected sessions, then removes them from the list", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "u1", title: "Old work" }), makeSession({ uid: "u2", title: "Other work" })),
    );
    previewDelete.mockResolvedValue([previewOf({ uid: "u1", title: "Old work", files: ["/p/u1.jsonl", "/p/u1"] })]);
    deleteSessions.mockResolvedValue([{ status: "trashed", uid: "u1", files: ["/p/u1.jsonl", "/p/u1"] }]);

    render(<App />);
    await user.click(await screen.findByRole("checkbox", { name: "Select Old work" }));
    await user.click(screen.getByRole("button", { name: /Delete 1/ }));

    const dialog = await screen.findByRole("dialog");
    expect(previewDelete).toHaveBeenCalledWith(["u1"]);
    expect(within(dialog).getByText(/Move 1 session to the Trash\?/)).toBeInTheDocument();
    expect(within(dialog).getByText("/p/u1")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));

    expect(deleteSessions).toHaveBeenCalledWith(["u1"]);
    expect(await screen.findByText(/Moved 1 session to the Trash/)).toBeInTheDocument();
    expect(screen.queryByText("Old work")).not.toBeInTheDocument();
    expect(screen.getByText("Other work")).toBeInTheDocument();
  });

  it("reveals a file from the confirmation dialog", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(scanOf(makeSession({ uid: "u1", title: "Old work" })));
    previewDelete.mockResolvedValue([previewOf({ uid: "u1", files: ["/p/u1.jsonl", "/p/u1"] })]);
    revealSessionFile.mockResolvedValue();

    render(<App />);
    await user.click(await screen.findByRole("checkbox", { name: "Select Old work" }));
    await user.click(screen.getByRole("button", { name: /Delete 1/ }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getAllByRole("button", { name: "Show in folder" })[1]);

    expect(revealSessionFile).toHaveBeenCalledWith("/p/u1");
    expect(deleteSessions).not.toHaveBeenCalled();
  });

  it("warns clearly when a delete is permanent", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "c1", title: "Codex thread", agent: "codex", permanentDelete: true })),
    );
    previewDelete.mockResolvedValue([
      previewOf({ uid: "c1", title: "Codex thread", permanent: true, command: "codex delete --force c1" }),
    ]);
    deleteSessions.mockResolvedValue([
      { status: "deleted", uid: "c1", files: ["/p/c1.jsonl"], command: "codex delete --force c1" },
    ]);

    render(<App />);
    await user.click(await screen.findByRole("checkbox", { name: "Select Codex thread" }));
    await user.click(screen.getByRole("button", { name: /Delete 1/ }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Delete 1 session\?/)).toBeInTheDocument();
    expect(within(dialog).getByRole("alert")).toHaveTextContent(/permanently deleted/);
    expect(within(dialog).getByText("codex delete --force c1")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    expect(await screen.findByText(/Permanently deleted 1 session/)).toBeInTheDocument();
  });

  it("shows OpenCode's child-session warning and rescans after an agent delete", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "o1", title: "Parent", agent: "opencode", permanentDelete: true }),
        makeSession({ uid: "o2", title: "Child", agent: "opencode", parentId: "ses_parent" }),
      ),
    );
    previewDelete.mockResolvedValue([
      previewOf({
        uid: "o1",
        title: "Parent",
        files: [],
        permanent: true,
        command: "opencode session delete ses_parent",
        warning: "OpenCode also deletes this session's 1 child session.",
      }),
    ]);
    deleteSessions.mockResolvedValue([{ status: "deleted", uid: "o1", files: [], command: "opencode session delete ses_parent" }]);

    render(<App />);
    expect(await screen.findByText("Parent")).toBeInTheDocument();
    expect(screen.queryByText("Child")).not.toBeInTheDocument();
    await user.click(screen.getByLabelText("Show subagent sessions (1)"));
    expect(screen.getByText("Subagent")).toBeInTheDocument();
    await user.click(screen.getByRole("checkbox", { name: "Select Parent" }));
    await user.click(screen.getByRole("button", { name: /Delete 1/ }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/also deletes this session's 1 child session/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    expect(await screen.findByText(/Permanently deleted 1 session/)).toBeInTheDocument();
    expect(scanSessions).toHaveBeenCalledTimes(2);
  });

  it("explains database-backed sessions in the files view", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "o1", title: "DB session", agentName: "OpenCode", files: [], database: "/data/opencode.db" })),
    );

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Show files for DB session" }));

    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/one shared database/)).toBeInTheDocument();
    expect(within(dialog).getByText("/data/opencode.db")).toBeInTheDocument();
  });

  it("does not let running sessions be selected", async () => {
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "r", title: "Live", running: true }),
        makeSession({ uid: "ro", title: "Locked", readOnlyReason: "Needs the agent's CLI" }),
      ),
    );

    render(<App />);

    expect(await screen.findByRole("checkbox", { name: "Select Live" })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "Select Locked" })).toBeDisabled();
    expect(screen.getByText("Running")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Select all sessions shown" })).toBeDisabled();
  });

  it("archives and unarchives through the agent, then rescans", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "a", title: "Active", agent: "codex", canArchive: true }),
        makeSession({ uid: "z", title: "Stored", agent: "codex", canArchive: true, archived: true }),
      ),
    );
    toggleArchive.mockResolvedValue([{ status: "done", uid: "a", archived: true, command: "codex archive a" }]);

    render(<App />);
    await user.click(await screen.findByRole("checkbox", { name: "Select all sessions shown" }));
    expect(screen.getByRole("button", { name: "Unarchive 1" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Archive 1" }));

    expect(toggleArchive).toHaveBeenCalledWith(["a"]);
    expect(await screen.findByText("Archived 1 session.")).toBeInTheDocument();
    expect(scanSessions).toHaveBeenCalledTimes(2);
  });

  it("shows sessions the backend refused and only confirms the rest", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(scanOf(makeSession({ uid: "a", title: "A" }), makeSession({ uid: "b", title: "B" })));
    previewDelete.mockResolvedValue([
      previewOf({ uid: "a", title: "A" }),
      previewOf({ uid: "b", title: "B", files: [], refusedReason: "The session changed since it was scanned." }),
    ]);
    deleteSessions.mockResolvedValue([{ status: "trashed", uid: "a", files: ["/a"] }]);

    render(<App />);
    await user.click(await screen.findByRole("checkbox", { name: "Select all sessions shown" }));
    await user.click(screen.getByRole("button", { name: /Delete 2/ }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Move 1 session to the Trash/)).toBeInTheDocument();
    expect(within(dialog).getByText(/changed since it was scanned/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));

    expect(deleteSessions).toHaveBeenCalledWith(["a"]);
  });

  it("sorts by a column, reverses on the second click, and clears on the third", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "1", title: "Beta", sizeBytes: 10 }),
        makeSession({ uid: "2", title: "alpha", sizeBytes: 30 }),
        makeSession({ uid: "3", title: "Gamma", sizeBytes: 20 }),
      ),
    );

    render(<App />);
    const sessionHeader = await screen.findByRole("button", { name: /^Session/ });
    expect(rowTitles()).toEqual(["Beta", "alpha", "Gamma"]);

    await user.click(sessionHeader);
    expect(rowTitles()).toEqual(["alpha", "Beta", "Gamma"]);
    expect(screen.getByRole("columnheader", { name: /Session/ })).toHaveAttribute("aria-sort", "ascending");

    await user.click(sessionHeader);
    expect(rowTitles()).toEqual(["Gamma", "Beta", "alpha"]);

    await user.click(sessionHeader);
    expect(rowTitles()).toEqual(["Beta", "alpha", "Gamma"]);
    expect(screen.getByRole("columnheader", { name: /Session/ })).toHaveAttribute("aria-sort", "none");

    await user.click(screen.getByRole("button", { name: /^Size/ }));
    expect(rowTitles()).toEqual(["alpha", "Gamma", "Beta"]);
  });

  it("shows a session's files without starting a delete", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "u1", title: "Old work", files: ["/p/u1.jsonl", "/p/u1"] })),
    );
    revealSessionFile.mockResolvedValue();

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Show files for Old work" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("/p/u1.jsonl")).toBeInTheDocument();
    await user.click(within(dialog).getAllByRole("button", { name: "Show in folder" })[0]);
    expect(revealSessionFile).toHaveBeenCalledWith("/p/u1.jsonl");

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(previewDelete).not.toHaveBeenCalled();
  });

  it("opens a session's transcript from its title", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(scanOf(makeSession({ uid: "u1", title: "Mystery session" })));
    loadTranscript.mockResolvedValue({
      head: [{ role: "user", text: "Explain the flaky test", timestamp: null, tool: null }],
      omitted: 0,
      tail: [],
      shortened: false,
      note: null,
    });

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Mystery session" }));

    expect(loadTranscript).toHaveBeenCalledWith("u1");
    expect(await screen.findByText("Explain the flaky test")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close transcript" }));
    expect(screen.queryByText("Explain the flaky test")).not.toBeInTheDocument();
  });

  it("shows project folders relative to home", async () => {
    scanSessions.mockResolvedValue(scanOf(makeSession({ projectDir: "/Users/me/Documents/Projects/team/api" })));

    render(<App />);

    const button = await screen.findByRole("button", { name: "Open project folder /Users/me/Documents/Projects/team/api" });
    expect(button).toHaveTextContent("~/…/team/api");
  });

  it("opens the project folder from the table", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(scanOf(makeSession({ uid: "u1", projectDir: "/work/app" })));
    openProjectDir.mockRejectedValue("The project folder /work/app no longer exists.");

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open project folder /work/app" }));

    expect(openProjectDir).toHaveBeenCalledWith("u1");
    expect(await screen.findByRole("alert")).toHaveTextContent("no longer exists");
  });

  it("filters to old sessions", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "old", title: "Ancient", lastActivity: "2020-01-01T00:00:00Z" }),
        makeSession({ uid: "new", title: "Fresh", lastActivity: new Date().toISOString() }),
      ),
    );

    render(<App />);
    await user.type(await screen.findByLabelText("Older than (days)"), "30");

    expect(screen.getByText("Ancient")).toBeInTheDocument();
    expect(screen.queryByText("Fresh")).not.toBeInTheDocument();
  });

  it("warns when an agent's scan was incomplete and shows scan errors", async () => {
    scanSessions.mockResolvedValue({
      agents: [{ id: "pi", name: "Pi", complete: false, errors: ["/x: permission denied"], sessionCount: 0 }],
      sessions: [],
      homeDir: "/Users/me",
    });

    render(<App />);

    expect(await screen.findByText(/Some Pi sessions couldn't be read: \/x: permission denied/)).toBeInTheDocument();
  });

  it("shows a centered spinner while scanning, over the table on rescans", async () => {
    const user = userEvent.setup();
    let finish: (value: ScanResponse) => void = () => {};
    scanSessions.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));

    render(<App />);

    expect(screen.getByRole("status")).toHaveTextContent("Scanning sessions…");
    finish(scanOf(makeSession({ uid: "u1", title: "Loaded" })));
    expect(await screen.findByText("Loaded")).toBeInTheDocument();
    expect(screen.queryByText("Scanning sessions…")).not.toBeInTheDocument();

    scanSessions.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));
    await user.click(screen.getByRole("button", { name: "Rescan" }));
    expect(screen.getByText("Scanning sessions…")).toBeInTheDocument();
    expect(screen.getByText("Loaded")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rescan" })).toBeDisabled();
    finish(scanOf(makeSession({ uid: "u1", title: "Loaded" })));
    expect(await screen.findByRole("button", { name: "Rescan" })).toBeEnabled();
  });

  it("shows an error when scanning fails", async () => {
    scanSessions.mockRejectedValue("boom");

    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  });
});
