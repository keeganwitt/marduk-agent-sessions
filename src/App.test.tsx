import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import * as ipc from "./lib/ipc";
import type {
  MemoryStore,
  PreviewItem,
  ScanResponse,
  SessionRow,
} from "./lib/types";
import { makeSession } from "./test/factories";

vi.mock("./lib/ipc", () => ({
  scanSessions: vi.fn(),
  previewDelete: vi.fn(),
  deleteSessions: vi.fn(),
  toggleArchive: vi.fn(),
  revealSessionFile: vi.fn(),
  openProjectDir: vi.fn(),
  agentHelp: vi.fn(),
  appVersion: vi.fn(),
  getColumns: vi.fn(),
  saveColumns: vi.fn(),
  getUiPref: vi.fn(),
  saveUiPref: vi.fn(),
  listMemory: vi.fn(),
  memoryLeftBehind: vi.fn(),
  readMemoryFile: vi.fn(),
  trashMemory: vi.fn(),
  revealMemory: vi.fn(),
  loadTranscript: vi.fn(),
  getTheme: vi.fn(),
  listLabels: vi.fn(),
  createLabel: vi.fn(),
  applyLabels: vi.fn(),
  fileDetails: vi.fn(),
}));

const scanSessions = vi.mocked(ipc.scanSessions);
const previewDelete = vi.mocked(ipc.previewDelete);
const deleteSessions = vi.mocked(ipc.deleteSessions);
const toggleArchive = vi.mocked(ipc.toggleArchive);
const revealSessionFile = vi.mocked(ipc.revealSessionFile);
const openProjectDir = vi.mocked(ipc.openProjectDir);
const loadTranscript = vi.mocked(ipc.loadTranscript);
const getTheme = vi.mocked(ipc.getTheme);

function rowTitles(): string[] {
  return screen
    .getAllByRole("row")
    .slice(1)
    .map((row) =>
      (within(row).getAllByRole("cell")[1].textContent ?? "").trim(),
    );
}

function scanOf(...sessions: SessionRow[]): ScanResponse {
  return {
    agents: [
      {
        id: "claude",
        name: "Claude Code",
        complete: true,
        errors: [],
        sessionCount: sessions.length,
      },
    ],
    sessions,
    homeDir: "/Users/me",
  };
}

function memoryStore(overrides: Partial<MemoryStore>): MemoryStore {
  return {
    agent: "claude",
    agentName: "Claude Code",
    folder: "/c/projects/-old/memory",
    projectDir: "/Users/me/old",
    projectExists: true,
    sessions: 0,
    files: [
      {
        path: "/c/projects/-old/memory/MEMORY.md",
        title: "MEMORY.md",
        description: null,
        kind: "index",
        sessionId: null,
        modified: null,
        size: 10,
      },
      {
        path: "/c/projects/-old/memory/git.md",
        title: "git",
        description: "Use the personal email",
        kind: "feedback",
        sessionId: null,
        modified: null,
        size: 20,
      },
    ],
    problems: [],
    note: null,
    canTrash: true,
    ...overrides,
  };
}

function previewOf(
  overrides: Partial<PreviewItem> & { uid: string },
): PreviewItem {
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
    getTheme.mockResolvedValue("system");
    vi.mocked(ipc.listLabels).mockResolvedValue([]);
    vi.mocked(ipc.fileDetails).mockResolvedValue([]);
    vi.mocked(ipc.memoryLeftBehind).mockResolvedValue([]);
    vi.mocked(ipc.getColumns).mockResolvedValue(null);
    vi.mocked(ipc.saveColumns).mockResolvedValue();
    vi.mocked(ipc.getUiPref).mockResolvedValue(null);
    vi.mocked(ipc.saveUiPref).mockResolvedValue();
    vi.mocked(ipc.appVersion).mockResolvedValue("1.2.3");
    localStorage.clear();
  });

  it("labels selected sessions from the Labels menu, then filters by label", async () => {
    const user = userEvent.setup();
    const work = { id: 1, name: "Work", color: "blue", count: 0 };
    vi.mocked(ipc.listLabels).mockResolvedValue([work]);
    vi.mocked(ipc.applyLabels).mockResolvedValue();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "a", title: "Alpha" }),
        makeSession({ uid: "b", title: "Beta", labelIds: [1] }),
      ),
    );

    render(<App />);
    expect(await screen.findByText("Alpha")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Labels ▾" }),
    ).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("checkbox", { name: "Select all sessions shown" }),
    );
    await user.click(screen.getByRole("button", { name: "Labels ▾" }));

    const menu = screen.getByRole("dialog", { name: "Labels" });
    const workBox = within(menu).getByRole("checkbox", {
      name: "Work",
    }) as HTMLInputElement;
    expect(workBox.indeterminate).toBe(true);
    await user.click(workBox);
    await user.click(within(menu).getByRole("button", { name: "Apply" }));

    expect(ipc.applyLabels).toHaveBeenCalledWith(["a", "b"], [1], []);
    expect(
      screen.getAllByText("Work", { selector: ".label-chip" }),
    ).toHaveLength(2);

    await user.selectOptions(screen.getByLabelText("Label"), "none");
    expect(
      screen.getByText("No sessions match these filters."),
    ).toBeInTheDocument();
  });

  it("creates a label from the menu and checks it", async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.createLabel).mockResolvedValue({
      id: 9,
      name: "Keep",
      color: "red",
      count: 0,
    });
    vi.mocked(ipc.applyLabels).mockResolvedValue();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "a", title: "Alpha" })),
    );

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select Alpha" }),
    );
    await user.click(screen.getByRole("button", { name: "Labels ▾" }));
    await user.type(
      screen.getByPlaceholderText("Find or create a label"),
      "Keep",
    );
    await user.click(screen.getByRole("button", { name: "Create “Keep”" }));

    expect(ipc.createLabel).toHaveBeenCalledWith("Keep", expect.any(String));
    const keep = screen.getByRole("checkbox", {
      name: "Keep",
    }) as HTMLInputElement;
    expect(keep.checked).toBe(true);
    await user.click(screen.getByRole("button", { name: "Apply" }));
    expect(ipc.applyLabels).toHaveBeenCalledWith(["a"], [9], []);
  });

  it("applies the saved theme on startup", async () => {
    getTheme.mockResolvedValue("light");
    scanSessions.mockResolvedValue(scanOf());

    render(<App />);

    await vi.waitFor(() =>
      expect(document.documentElement.dataset.theme).toBe("light"),
    );
  });

  it("previews and trashes selected sessions, then removes them from the list", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "u1", title: "Old work" }),
        makeSession({ uid: "u2", title: "Other work" }),
      ),
    );
    previewDelete.mockResolvedValue([
      previewOf({
        uid: "u1",
        title: "Old work",
        files: ["/p/u1.jsonl", "/p/u1"],
      }),
    ]);
    deleteSessions.mockResolvedValue([
      { status: "trashed", uid: "u1", files: ["/p/u1.jsonl", "/p/u1"] },
    ]);

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select Old work" }),
    );
    await user.click(screen.getByRole("button", { name: "Delete…" }));

    const dialog = await screen.findByRole("dialog");
    expect(previewDelete).toHaveBeenCalledWith(["u1"]);
    expect(
      within(dialog).getByText(/Move 1 session to the Trash\?/),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("u1.jsonl")).toBeInTheDocument();
    expect(within(dialog).getByText("/p")).toBeInTheDocument();
    await user.click(
      within(dialog).getByRole("button", { name: "Move to Trash" }),
    );

    expect(deleteSessions).toHaveBeenCalledWith(["u1"]);
    expect(
      await screen.findByText(/Moved 1 session to the Trash/),
    ).toBeInTheDocument();
    expect(screen.queryByText("Old work")).not.toBeInTheDocument();
    expect(screen.getByText("Other work")).toBeInTheDocument();

    vi.mocked(ipc.listMemory).mockResolvedValue([]);
    await user.click(screen.getByRole("tab", { name: "Memory" }));
    expect(
      screen.queryByText(/Moved 1 session to the Trash/),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Sessions" }));
    expect(
      screen.getByText(/Moved 1 session to the Trash/),
    ).toBeInTheDocument();
  });

  it("offers a project's memory with its last session, unchecked, and trashes it only if chosen", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "u1",
          title: "Last one",
          projectDir: "/Users/me/old",
        }),
      ),
    );
    previewDelete.mockResolvedValue([
      previewOf({ uid: "u1", title: "Last one" }),
    ]);
    deleteSessions.mockResolvedValue([
      { status: "trashed", uid: "u1", files: ["/p/u1.jsonl"] },
    ]);
    vi.mocked(ipc.memoryLeftBehind).mockResolvedValue([
      memoryStore({ projectDir: "/Users/me/old", projectExists: false }),
    ]);
    vi.mocked(ipc.trashMemory).mockResolvedValue();

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select Last one" }),
    );
    await user.click(screen.getByRole("button", { name: "Delete…" }));

    const dialog = await screen.findByRole("dialog");
    expect(ipc.memoryLeftBehind).toHaveBeenCalledWith(["u1"]);
    const offer = within(dialog).getByRole("checkbox", {
      name: /Also move Claude Code's memory for ~\/old to the Trash \(2 files\)/,
    });
    expect(offer).not.toBeChecked();
    expect(dialog).toHaveTextContent("Its project folder no longer exists");
    await user.click(offer);
    await user.click(
      within(dialog).getByRole("button", { name: "Move to Trash" }),
    );

    expect(ipc.trashMemory).toHaveBeenCalledWith(
      "/c/projects/-old/memory",
      true,
    );
    expect(
      await screen.findByText(/Moved its project's memory to the Trash/),
    ).toBeInTheDocument();
  });

  it("leaves memory alone when its offer isn't checked", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "u1", title: "Last one" })),
    );
    previewDelete.mockResolvedValue([
      previewOf({ uid: "u1", title: "Last one" }),
    ]);
    deleteSessions.mockResolvedValue([
      { status: "trashed", uid: "u1", files: ["/p/u1.jsonl"] },
    ]);
    vi.mocked(ipc.memoryLeftBehind).mockResolvedValue([memoryStore({})]);

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select Last one" }),
    );
    await user.click(screen.getByRole("button", { name: "Delete…" }));
    await user.click(
      within(await screen.findByRole("dialog")).getByRole("button", {
        name: "Move to Trash",
      }),
    );

    expect(
      await screen.findByText(/Moved 1 session to the Trash/),
    ).toBeInTheDocument();
    expect(ipc.trashMemory).not.toHaveBeenCalled();
  });

  it("switches to the Memory tab", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(scanOf(makeSession({ title: "A session" })));
    vi.mocked(ipc.listMemory).mockResolvedValue([memoryStore({})]);

    render(<App />);
    await screen.findByText("A session");
    await user.click(screen.getByRole("tab", { name: "Memory" }));

    expect(
      await screen.findByRole("heading", { name: "~/old" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("search")).not.toBeInTheDocument();
    expect(screen.queryByText("A session")).not.toBeInTheDocument();
  });

  it("reveals a file from the confirmation dialog", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "u1", title: "Old work" })),
    );
    previewDelete.mockResolvedValue([
      previewOf({ uid: "u1", files: ["/p/u1.jsonl", "/p/u1"] }),
    ]);
    revealSessionFile.mockResolvedValue();

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select Old work" }),
    );
    await user.click(screen.getByRole("button", { name: "Delete…" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(
      within(dialog).getAllByRole("button", { name: "Show in folder" })[1],
    );

    expect(revealSessionFile).toHaveBeenCalledWith("/p/u1");
    expect(deleteSessions).not.toHaveBeenCalled();
  });

  it("warns clearly when a delete is permanent", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "c1",
          title: "Codex thread",
          agent: "codex",
          permanentDelete: true,
        }),
      ),
    );
    previewDelete.mockResolvedValue([
      previewOf({
        uid: "c1",
        title: "Codex thread",
        permanent: true,
        command: "codex delete --force c1",
      }),
    ]);
    deleteSessions.mockResolvedValue([
      {
        status: "deleted",
        uid: "c1",
        files: ["/p/c1.jsonl"],
        command: "codex delete --force c1",
      },
    ]);

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select Codex thread" }),
    );
    await user.click(screen.getByRole("button", { name: "Delete…" }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Delete 1 session\?/)).toBeInTheDocument();
    expect(within(dialog).getByRole("alert")).toHaveTextContent(
      /permanently deleted/,
    );
    expect(
      within(dialog).getByText("codex delete --force c1"),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    expect(
      await screen.findByText(/Permanently deleted 1 session/),
    ).toBeInTheDocument();
  });

  it("shows OpenCode's child-session warning and rescans after an agent delete", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "o1",
          id: "ses_parent",
          title: "Parent",
          agent: "opencode",
          permanentDelete: true,
        }),
        makeSession({
          uid: "o2",
          title: "Child",
          agent: "opencode",
          parentId: "ses_parent",
          deletedWithParent: true,
        }),
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
    deleteSessions.mockResolvedValue([
      {
        status: "deleted",
        uid: "o1",
        files: [],
        command: "opencode session delete ses_parent",
      },
    ]);

    render(<App />);
    expect(await screen.findByText("Parent")).toBeInTheDocument();
    expect(screen.queryByText("Child")).not.toBeInTheDocument();
    expect(screen.getByText("1 subagent")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Show 1 subagent session of Parent" }),
    );
    expect(screen.getByText("Child").closest("td")).toHaveStyle({
      paddingLeft: "28px",
    });
    expect(screen.queryByText("Subagent")).not.toBeInTheDocument();
    await user.click(screen.getByRole("checkbox", { name: "Select Parent" }));
    await user.click(screen.getByRole("button", { name: "Delete…" }));

    const dialog = await screen.findByRole("dialog");
    expect(
      within(dialog).getByText(/also deletes this session's 1 child session/),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    expect(
      await screen.findByText(/Permanently deleted 1 session/),
    ).toBeInTheDocument();
    expect(scanSessions).toHaveBeenCalledTimes(2);
  });

  it("explains database-backed sessions in the files view", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "o1",
          title: "DB session",
          agentName: "OpenCode",
          files: [],
          database: "/data/opencode.db",
        }),
      ),
    );

    render(<App />);
    await user.click(
      await screen.findByRole("button", {
        name: "More actions for DB session",
      }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Files…" }));

    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/one shared database/)).toBeInTheDocument();
    expect(within(dialog).getByText("opencode.db")).toBeInTheDocument();
    expect(within(dialog).getByText("/data")).toBeInTheDocument();
  });

  it("does not let running sessions be selected", async () => {
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "r", title: "Live", running: true }),
        makeSession({
          uid: "ro",
          title: "Locked",
          readOnlyReason: "Needs the agent's CLI",
        }),
      ),
    );

    render(<App />);

    expect(
      await screen.findByRole("checkbox", { name: "Select Live" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("checkbox", { name: "Select Locked" }),
    ).toBeDisabled();
    expect(screen.getByText("Running")).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "Select all sessions shown" }),
    ).toBeDisabled();
  });

  it("archives and unarchives through the agent, then rescans", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "a",
          title: "Active",
          agent: "codex",
          canArchive: true,
        }),
        makeSession({
          uid: "z",
          title: "Stored",
          agent: "codex",
          canArchive: true,
          archived: true,
        }),
      ),
    );
    toggleArchive.mockResolvedValue([
      { status: "done", uid: "a", archived: true, command: "codex archive a" },
    ]);

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", {
        name: "Select all sessions shown",
      }),
    );
    expect(
      screen.getByRole("button", { name: "Unarchive 1" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Archive 1" }));

    expect(toggleArchive).toHaveBeenCalledWith(["a"]);
    expect(await screen.findByText("Archived 1 session.")).toBeInTheDocument();
    expect(scanSessions).toHaveBeenCalledTimes(2);
  });

  it("shows sessions the backend refused and only confirms the rest", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "a", title: "A" }),
        makeSession({ uid: "b", title: "B" }),
      ),
    );
    previewDelete.mockResolvedValue([
      previewOf({ uid: "a", title: "A" }),
      previewOf({
        uid: "b",
        title: "B",
        files: [],
        refusedReason: "The session changed since it was scanned.",
      }),
    ]);
    deleteSessions.mockResolvedValue([
      { status: "trashed", uid: "a", files: ["/a"] },
    ]);

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", {
        name: "Select all sessions shown",
      }),
    );
    await user.click(screen.getByRole("button", { name: "Delete…" }));

    const dialog = await screen.findByRole("dialog");
    expect(
      within(dialog).getByText(/Move 1 session to the Trash/),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(/changed since it was scanned/),
    ).toBeInTheDocument();
    await user.click(
      within(dialog).getByRole("button", { name: "Move to Trash" }),
    );

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
    const sessionHeader = await screen.findByRole("button", {
      name: /^Session/,
    });
    expect(rowTitles()).toEqual(["Beta", "alpha", "Gamma"]);
    expect(
      screen.getByRole("columnheader", { name: /Last activity/ }),
    ).toHaveAttribute("aria-sort", "descending");

    await user.click(sessionHeader);
    expect(rowTitles()).toEqual(["alpha", "Beta", "Gamma"]);
    expect(
      screen.getByRole("columnheader", { name: /Session/ }),
    ).toHaveAttribute("aria-sort", "ascending");

    await user.click(sessionHeader);
    expect(rowTitles()).toEqual(["Gamma", "Beta", "alpha"]);

    await user.click(sessionHeader);
    expect(rowTitles()).toEqual(["Beta", "alpha", "Gamma"]);
    expect(
      screen.getByRole("columnheader", { name: /Session/ }),
    ).toHaveAttribute("aria-sort", "none");
    expect(
      screen.getByRole("columnheader", { name: /Last activity/ }),
    ).toHaveAttribute("aria-sort", "descending");

    await user.click(screen.getByRole("button", { name: /^Size/ }));
    expect(rowTitles()).toEqual(["alpha", "Gamma", "Beta"]);
  });

  it("shows a session's files without starting a delete", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "u1",
          title: "Old work",
          files: ["/p/u1.jsonl", "/p/u1"],
        }),
      ),
    );
    revealSessionFile.mockResolvedValue();
    vi.mocked(ipc.fileDetails).mockResolvedValue([
      { path: "/p/u1.jsonl", sizeBytes: 1536, isDir: false, exists: true },
      { path: "/p/u1", sizeBytes: 900, isDir: true, exists: true },
    ]);

    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "More actions for Old work" }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Files…" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText("u1.jsonl")).toBeInTheDocument();
    expect(await within(dialog).findByText("1.5 KB")).toBeInTheDocument();
    expect(within(dialog).getByText("u1/")).toBeInTheDocument();
    expect(within(dialog).getByText("u1.jsonl").closest("li")).toHaveAttribute(
      "title",
      "/p/u1.jsonl",
    );
    await user.click(
      within(dialog).getAllByRole("button", { name: "Show in folder" })[0],
    );
    expect(revealSessionFile).toHaveBeenCalledWith("/p/u1.jsonl");

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(previewDelete).not.toHaveBeenCalled();
  });

  it("opens a session's transcript from its title", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "u1", title: "Mystery session" })),
    );
    loadTranscript.mockResolvedValue({
      head: [
        {
          role: "user",
          text: "Explain the flaky test",
          timestamp: null,
          tool: null,
        },
      ],
      omitted: 0,
      tail: [],
      shortened: false,
      note: null,
    });

    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Mystery session" }),
    );

    expect(loadTranscript).toHaveBeenCalledWith("u1");
    expect(
      await screen.findByText("Explain the flaky test"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Close transcript" }));
    expect(
      screen.queryByText("Explain the flaky test"),
    ).not.toBeInTheDocument();
  });

  it("restores column settings saved in the settings file", async () => {
    vi.mocked(ipc.getColumns).mockResolvedValue({
      widths: {},
      hidden: ["labels"],
      shown: ["model"],
    });
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ title: "A session", models: ["claude-opus-5-5"] })),
    );

    render(<App />);

    expect(
      await screen.findByRole("columnheader", { name: /Model/ }),
    ).toBeInTheDocument();
    expect(screen.getByText("claude-opus-5-5")).toBeInTheDocument();
    expect(
      screen.queryByRole("columnheader", { name: "Labels" }),
    ).not.toBeInTheDocument();
  });

  it("copies columns from the webview's storage into an empty settings file", async () => {
    localStorage.setItem(
      "marduk.columns",
      JSON.stringify({ widths: { size: 120 }, hidden: [], shown: [] }),
    );
    scanSessions.mockResolvedValue(scanOf(makeSession({ title: "A session" })));

    render(<App />);

    await vi.waitFor(() =>
      expect(ipc.saveColumns).toHaveBeenCalledWith({
        widths: { size: 120 },
        hidden: [],
        shown: [],
      }),
    );
  });

  it("saves column changes to the settings file", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(scanOf(makeSession({ title: "A session" })));

    render(<App />);
    await screen.findByText("A session");
    await user.click(screen.getByRole("button", { name: /Columns/ }));
    await user.click(screen.getByRole("checkbox", { name: "Model" }));

    await vi.waitFor(() =>
      expect(ipc.saveColumns).toHaveBeenLastCalledWith(
        expect.objectContaining({ shown: ["model"] }),
      ),
    );
  });

  it("shows a badge for the agent a session ran as, and filters by it", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "a",
          title: "Plan the data model",
          agentType: "architect",
        }),
        makeSession({ uid: "b", title: "Plain session" }),
      ),
    );

    render(<App />);
    const badge = await screen.findByText("architect", { selector: ".badge" });
    expect(badge).toHaveAttribute("title", "Ran as the architect agent");

    await user.click(screen.getByRole("button", { name: "+ Filter" }));
    await user.click(screen.getByRole("menuitem", { name: "Agent type" }));

    await user.selectOptions(
      screen.getByRole("combobox", { name: "Agent type" }),
      "architect",
    );

    expect(
      rowTitles().map((title) => title.replace("architect", "").trim()),
    ).toEqual(["Plan the data model"]);
  });

  it("doesn't offer an Agent type filter when no session has one", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ title: "Plain session" })),
    );

    render(<App />);
    await screen.findByText("Plain session");
    await user.click(screen.getByRole("button", { name: "+ Filter" }));

    expect(
      screen.getByRole("menuitem", { name: "Status" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Agent type" })).toBeNull();
  });

  it("adds, saves and removes optional filters, and clears them all", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "a", title: "Active one" }),
        makeSession({ uid: "b", title: "Archived one", archived: true }),
      ),
    );

    render(<App />);
    await screen.findByText("Active one");
    expect(screen.queryByRole("combobox", { name: "Status" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "+ Filter" }));
    await user.click(screen.getByRole("menuitem", { name: "Status" }));
    expect(ipc.saveUiPref).toHaveBeenCalledWith("filters", ["archive"]);
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Status" }),
      "archived",
    );
    expect(rowTitles()).toEqual(["Archived oneArchived"]);

    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(rowTitles()).toHaveLength(2);
    expect(
      screen.getByRole("combobox", { name: "Status" }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Remove the Status filter" }),
    );
    expect(screen.queryByRole("combobox", { name: "Status" })).toBeNull();
    expect(ipc.saveUiPref).toHaveBeenLastCalledWith("filters", []);
  });

  it("finds sessions with deleted parents through an optional filter", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "p", id: "p", title: "Parent" }),
        makeSession({ uid: "c", title: "Child", parentId: "p" }),
        makeSession({
          uid: "orphan",
          title: "Orphan",
          parentId: "gone",
          parentMissing: true,
        }),
      ),
    );
    render(<App />);
    await screen.findByText("Parent");
    expect(
      screen.queryByRole("checkbox", { name: "Parent deleted" }),
    ).toBeNull();

    await user.click(screen.getByRole("button", { name: "+ Filter" }));
    await user.click(screen.getByRole("menuitem", { name: "Parent deleted" }));
    expect(
      screen.getByRole("checkbox", { name: "Parent deleted" }),
    ).toBeChecked();
    expect(rowTitles()).toEqual(["OrphanParent deleted"]);
    expect(ipc.saveUiPref).toHaveBeenCalledWith("filters", ["parentMissing"]);

    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(screen.getByText("Parent")).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "Parent deleted" }),
    ).not.toBeChecked();
    await user.click(screen.getByRole("checkbox", { name: "Parent deleted" }));
    expect(rowTitles()).toEqual(["OrphanParent deleted"]);

    await user.click(
      screen.getByRole("button", { name: "Remove the Parent deleted filter" }),
    );
    expect(screen.getByText("Parent")).toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: "Parent deleted" }),
    ).toBeNull();
    expect(ipc.saveUiPref).toHaveBeenLastCalledWith("filters", []);
  });

  it("restores optional filters added in an earlier session", async () => {
    vi.mocked(ipc.getUiPref).mockResolvedValue(["archive", "bogus"]);
    scanSessions.mockResolvedValue(scanOf(makeSession({ title: "A session" })));

    render(<App />);

    expect(
      await screen.findByRole("combobox", { name: "Status" }),
    ).toBeInTheDocument();
  });

  it("opens the help dialog", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(scanOf(makeSession()));
    vi.mocked(ipc.agentHelp).mockResolvedValue([]);

    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Help" }));

    expect(
      screen.getByRole("dialog", { name: "How Marduk changes sessions" }),
    ).toBeInTheDocument();
  });

  it("shows project folders relative to home", async () => {
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ projectDir: "/Users/me/Documents/Projects/team/api" }),
      ),
    );

    render(<App />);

    const button = await screen.findByRole("button", {
      name: "Open project folder /Users/me/Documents/Projects/team/api",
    });
    expect(button).toHaveTextContent("~/…/team/api");
  });

  it("opens the project folder from the table", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "u1", projectDir: "/work/app" })),
    );
    openProjectDir.mockRejectedValue(
      "The project folder /work/app no longer exists.",
    );

    render(<App />);
    await user.click(
      await screen.findByRole("button", {
        name: "Open project folder /work/app",
      }),
    );

    expect(openProjectDir).toHaveBeenCalledWith("u1");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "no longer exists",
    );
  });

  it("filters to a project from the table, and clears it", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "a", title: "API work", projectDir: "/work/api" }),
        makeSession({ uid: "w", title: "Web work", projectDir: "/work/web" }),
      ),
    );

    render(<App />);
    await user.click(
      await screen.findByRole("button", {
        name: "Show only sessions in /work/api",
      }),
    );

    expect(rowTitles()).toEqual(["API work"]);
    expect(screen.getByRole("combobox", { name: "Project" })).toHaveValue(
      "/work/api",
    );
    expect(openProjectDir).not.toHaveBeenCalled();

    await user.click(
      screen.getByRole("button", { name: "Show every project" }),
    );

    expect(rowTitles()).toHaveLength(2);
  });

  it("groups chats started without a project", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "c1",
          title: "Explain a regex",
          projectDir: "/Users/me/Documents/Codex/2026-11-06/explain",
          projectless: true,
        }),
        makeSession({
          uid: "c2",
          title: "Convert a CSV",
          projectDir: "/Users/me/Documents/Codex/2026-11-05/convert",
          projectless: true,
        }),
        makeSession({ uid: "r", title: "Repo work", projectDir: "/work/app" }),
      ),
    );

    render(<App />);
    expect(
      await screen.findByRole("button", {
        name: "Open chat folder /Users/me/Documents/Codex/2026-11-06/explain",
      }),
    ).toHaveTextContent("No project");

    await user.click(screen.getByRole("combobox", { name: "Project" }));
    const options = within(screen.getByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "No project2",
      "/work/app1",
    ]);

    await user.click(options[0]);

    expect(rowTitles().sort()).toEqual(["Convert a CSV", "Explain a regex"]);
    expect(screen.getByRole("combobox", { name: "Project" })).toHaveValue(
      "No project",
    );
  });

  it("picks a project by typing part of its path", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "a",
          title: "API work",
          projectDir: "/Users/me/Projects/api",
        }),
        makeSession({
          uid: "a2",
          title: "API tests",
          projectDir: "/Users/me/Projects/api/tests",
        }),
        makeSession({ uid: "w", title: "Web work", projectDir: "/srv/web" }),
      ),
    );

    render(<App />);
    const input = await screen.findByRole("combobox", { name: "Project" });
    await user.type(input, "~/projects/api");

    const options = within(screen.getByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "~/Projects/api1",
      "~/Projects/api/tests1",
    ]);

    await user.keyboard("{Enter}");

    expect(rowTitles().sort()).toEqual(["API tests", "API work"]);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(input).toHaveValue("~/Projects/api");
  });

  it("filters to old sessions", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "old",
          title: "Ancient",
          lastActivity: "2020-01-01T00:00:00Z",
        }),
        makeSession({
          uid: "new",
          title: "Fresh",
          lastActivity: new Date().toISOString(),
        }),
      ),
    );

    render(<App />);
    await user.type(await screen.findByLabelText("Older than (days)"), "30");

    expect(screen.getByText("Ancient")).toBeInTheDocument();
    expect(screen.queryByText("Fresh")).not.toBeInTheDocument();
  });

  it("warns when an agent's scan was incomplete and shows scan errors", async () => {
    scanSessions.mockResolvedValue({
      agents: [
        {
          id: "pi",
          name: "Pi",
          complete: false,
          errors: ["/x: permission denied"],
          sessionCount: 0,
        },
      ],
      sessions: [],
      homeDir: "/Users/me",
    });

    render(<App />);

    expect(
      await screen.findByText(
        /Some Pi sessions couldn't be read: \/x: permission denied/,
      ),
    ).toBeInTheDocument();
  });

  it("nests subagents under their parent and shows orphans at the top level", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "p",
          id: "p",
          agent: "codex",
          title: "Main thread",
        }),
        makeSession({
          uid: "g",
          id: "g",
          agent: "codex",
          title: "Guardian review",
          parentId: "p",
        }),
        makeSession({
          uid: "o",
          id: "o",
          agent: "codex",
          title: "Lost review",
          parentId: "gone",
          parentMissing: true,
        }),
      ),
    );

    render(<App />);

    expect(await screen.findByText("Main thread")).toBeInTheDocument();
    expect(screen.queryByText("Guardian review")).not.toBeInTheDocument();
    expect(screen.getByText("Lost review")).toBeInTheDocument();
    expect(screen.getByText("Parent deleted")).toBeInTheDocument();

    const expander = screen.getByRole("button", {
      name: "Show 1 subagent session of Main thread",
    });
    await user.click(expander);
    expect(screen.getByText("Guardian review")).toBeInTheDocument();
    expect(expander).toHaveAttribute("aria-expanded", "true");

    await user.click(
      screen.getByRole("checkbox", { name: "Select all sessions shown" }),
    );
    expect(
      screen.getByRole("group", { name: /^2 selected/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "Select Guardian review" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("checkbox", { name: "Select Lost review" }),
    ).toBeChecked();
  });

  it("shows Claude subagents under their session as rows that can't be selected", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "p", id: "p", title: "Main task" }),
        makeSession({
          uid: "s",
          id: "p/agent-a1",
          title: "Research the API · general-purpose",
          parentId: "p",
          deletedWithParent: true,
          partOfParent: true,
          readOnlyReason: "This subagent is part of its parent session.",
        }),
      ),
    );

    render(<App />);
    await user.click(
      await screen.findByRole("button", {
        name: "Show 1 subagent session of Main task",
      }),
    );

    expect(
      screen.getByText("Research the API · general-purpose"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", {
        name: "Select Research the API · general-purpose",
      }),
    ).toBeDisabled();
    expect(screen.queryByText("Read-only")).not.toBeInTheDocument();
  });

  it("shows a parent, dimmed and opened, when only its subagent matches the search", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "p",
          id: "p",
          title: "Main thread",
          projectDir: "/a",
        }),
        makeSession({
          uid: "c",
          id: "c",
          title: "Needle review",
          parentId: "p",
          projectDir: "/a",
        }),
      ),
    );

    render(<App />);
    await user.type(await screen.findByLabelText("Search"), "needle");

    expect(screen.getByText("Needle review")).toBeInTheDocument();
    expect(screen.getByText("Main thread").closest("tr")).toHaveClass(
      "context-row",
    );
    expect(screen.getByText(/1 of 2 sessions/)).toBeInTheDocument();
  });

  it("shift-clicking selects or clears the range from the last clicked row, skipping running ones", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "1", title: "One" }),
        makeSession({ uid: "2", title: "Two" }),
        makeSession({ uid: "3", title: "Three", running: true }),
        makeSession({ uid: "4", title: "Four" }),
        makeSession({ uid: "5", title: "Five" }),
      ),
    );

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select Two" }),
    );
    await user.keyboard("{Shift>}");
    await user.click(screen.getByRole("checkbox", { name: "Select Four" }));
    await user.keyboard("{/Shift}");

    const checked = () =>
      ["One", "Two", "Three", "Four", "Five"].filter(
        (name) =>
          (
            screen.getByRole("checkbox", {
              name: `Select ${name}`,
            }) as HTMLInputElement
          ).checked,
      );
    expect(checked()).toEqual(["Two", "Four"]);
    expect(
      screen.getByRole("group", { name: /^2 selected/ }),
    ).toBeInTheDocument();

    await user.keyboard("{Shift>}");
    await user.click(screen.getByRole("checkbox", { name: "Select One" }));
    await user.keyboard("{/Shift}");
    expect(checked()).toEqual(["One", "Two", "Four"]);

    await user.keyboard("{Shift>}");
    await user.click(screen.getByRole("checkbox", { name: "Select Two" }));
    await user.keyboard("{/Shift}");
    expect(checked()).toEqual(["Four"]);
  });

  it("hides and shows columns from the Columns menu and remembers the choice", async () => {
    const user = userEvent.setup();
    localStorage.clear();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "a", title: "Alpha", agentName: "Codex" })),
    );

    const { unmount } = render(<App />);
    expect(
      await screen.findByRole("columnheader", { name: /Agent/ }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Columns ▾" }));
    await user.click(screen.getByRole("checkbox", { name: "Agent" }));

    expect(
      screen.queryByRole("columnheader", { name: /Agent/ }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Codex")).not.toBeInTheDocument();
    unmount();

    render(<App />);
    await screen.findByText("Alpha");
    expect(
      screen.queryByRole("columnheader", { name: /Agent/ }),
    ).not.toBeInTheDocument();
    localStorage.clear();
  });

  it("resizes columns by dragging or with the arrow keys, and resets on double-click", async () => {
    const user = userEvent.setup();
    localStorage.clear();
    scanSessions.mockResolvedValue(
      scanOf(makeSession({ uid: "a", title: "Alpha" })),
    );

    render(<App />);
    const handle = await screen.findByRole("separator", {
      name: "Resize Project column",
    });
    const width = () => Number(handle.getAttribute("aria-valuenow"));
    const initial = width();

    handle.focus();
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(width()).toBe(initial + 32);

    fireEvent.pointerDown(handle, { clientX: 100 });
    fireEvent.pointerMove(window, { clientX: 150 });
    fireEvent.pointerUp(window);
    expect(width()).toBe(initial + 82);
    expect(
      JSON.parse(localStorage.getItem("marduk.columns")!).widths.project,
    ).toBe(initial + 82);

    await user.dblClick(handle);
    expect(width()).toBe(initial);
    localStorage.clear();
  });

  it("shows a centered spinner while scanning, over the table on rescans", async () => {
    const user = userEvent.setup();
    let finish: (value: ScanResponse) => void = () => {};
    scanSessions.mockReturnValueOnce(
      new Promise((resolve) => (finish = resolve)),
    );

    render(<App />);

    expect(screen.getByRole("status")).toHaveTextContent("Scanning sessions…");
    finish(scanOf(makeSession({ uid: "u1", title: "Loaded" })));
    expect(await screen.findByText("Loaded")).toBeInTheDocument();
    expect(screen.queryByText("Scanning sessions…")).not.toBeInTheDocument();

    scanSessions.mockReturnValueOnce(
      new Promise((resolve) => (finish = resolve)),
    );
    await user.click(screen.getByRole("button", { name: "Rescan" }));
    expect(screen.getByText("Scanning sessions…")).toBeInTheDocument();
    expect(screen.getByText("Loaded")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Rescan" })).toBeDisabled();
    finish(scanOf(makeSession({ uid: "u1", title: "Loaded" })));
    expect(await screen.findByRole("button", { name: "Rescan" })).toBeEnabled();
  });

  it("shows a progress bar with the agent being scanned once progress arrives", async () => {
    let report: (progress: {
      done: number;
      total: number;
      agent: string;
    }) => void = () => {};
    let finish: (value: ScanResponse) => void = () => {};
    scanSessions.mockImplementationOnce((onProgress) => {
      report = onProgress!;
      return new Promise((resolve) => (finish = resolve));
    });

    render(<App />);
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();

    act(() => report({ done: 50, total: 200, agent: "Codex" }));

    expect(
      screen.getByRole("progressbar", { name: "Scan progress" }),
    ).toHaveAttribute("value", "25");
    expect(screen.getByText(/Scanning Codex… 25%/)).toBeInTheDocument();
    finish(scanOf(makeSession({ uid: "u1", title: "Loaded" })));
    expect(await screen.findByText("Loaded")).toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("shows an error when scanning fails", async () => {
    scanSessions.mockRejectedValue("boom");

    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  });
  it("groups session actions behind a selection, and clears it", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "a", title: "Alpha", sizeBytes: 1024 }),
        makeSession({
          uid: "b",
          title: "Beta",
          sizeBytes: 2048,
          readOnlyReason: "Needs the agent's CLI",
          canArchive: true,
        }),
      ),
    );

    render(<App />);
    expect(await screen.findByText("Alpha")).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /selected/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /Delete/ })).toBeNull();

    await user.click(screen.getByRole("checkbox", { name: "Select Alpha" }));
    await user.click(screen.getByRole("checkbox", { name: "Select Beta" }));

    const group = screen.getByRole("group", { name: /^2 selected/ });
    expect(group).toHaveTextContent("2 selected · 3.0 KB");
    expect(
      within(group).getByRole("button", { name: "Delete 1 (1.0 KB)…" }),
    ).toBeInTheDocument();
    expect(
      within(group).getByRole("button", { name: "Archive 1" }),
    ).toBeInTheDocument();

    await user.click(
      within(group).getByRole("button", { name: "Clear selection" }),
    );
    expect(screen.queryByRole("group", { name: /selected/ })).toBeNull();
    expect(
      screen.getByRole("checkbox", { name: "Select Alpha" }),
    ).not.toBeChecked();
  });

  it("copies a session's ID and shows its file from the row menu", async () => {
    const user = userEvent.setup();
    revealSessionFile.mockResolvedValue();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({
          uid: "a",
          id: "abc-123",
          title: "Alpha",
          files: ["/p/abc-123.jsonl"],
        }),
      ),
    );

    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "More actions for Alpha" }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Copy session ID" }));
    expect(await navigator.clipboard.readText()).toBe("abc-123");
    expect(screen.queryByRole("menu")).toBeNull();

    await user.click(
      screen.getByRole("button", { name: "More actions for Alpha" }),
    );
    fireEvent.scroll(window);
    expect(screen.getByRole("menu")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();

    await user.click(
      screen.getByRole("button", { name: "More actions for Alpha" }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Show in folder" }));
    expect(revealSessionFile).toHaveBeenCalledWith("/p/abc-123.jsonl");
  });
  it("deletes a parent's Guardian reviews with it, since Codex leaves them behind", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(
      scanOf(
        makeSession({ uid: "p", id: "p", agent: "codex", title: "Main" }),
        makeSession({
          uid: "s",
          id: "s",
          agent: "codex",
          title: "Spawned",
          parentId: "p",
          deletedWithParent: true,
        }),
        makeSession({
          uid: "r",
          id: "r",
          agent: "codex",
          title: "Review",
          parentId: "p",
        }),
      ),
    );
    previewDelete.mockResolvedValue([]);

    render(<App />);
    await user.click(
      await screen.findByRole("checkbox", { name: "Select Main" }),
    );
    await user.click(screen.getByRole("button", { name: /^Delete 2 / }));

    expect(previewDelete).toHaveBeenCalledWith(["p", "r"]);
  });
});
