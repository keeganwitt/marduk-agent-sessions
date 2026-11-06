import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import * as ipc from "./lib/ipc";
import type { ScanResponse } from "./lib/types";
import { makeSession } from "./test/factories";

vi.mock("./lib/ipc", () => ({
  scanSessions: vi.fn(),
  previewDelete: vi.fn(),
  deleteSessions: vi.fn(),
}));

const scanSessions = vi.mocked(ipc.scanSessions);
const previewDelete = vi.mocked(ipc.previewDelete);
const deleteSessions = vi.mocked(ipc.deleteSessions);

function scanOf(...sessions: ReturnType<typeof makeSession>[]): ScanResponse {
  return {
    agents: [{ id: "claude", name: "Claude Code", complete: true, errors: [], sessionCount: sessions.length }],
    sessions,
  };
}

describe("App", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("previews and trashes selected sessions, then removes them from the list", async () => {
    const user = userEvent.setup();
    const old = makeSession({ uid: "u1", title: "Old work" });
    const other = makeSession({ uid: "u2", title: "Other work" });
    scanSessions.mockResolvedValue(scanOf(old, other));
    previewDelete.mockResolvedValue([
      { uid: "u1", title: "Old work", agentName: "Claude Code", files: ["/p/u1.jsonl", "/p/u1"], refusedReason: null },
    ]);
    deleteSessions.mockResolvedValue([{ status: "trashed", uid: "u1", files: ["/p/u1.jsonl", "/p/u1"] }]);

    render(<App />);
    await user.click(await screen.findByRole("checkbox", { name: "Select Old work" }));
    await user.click(screen.getByRole("button", { name: /Move 1 to Trash/ }));

    const dialog = await screen.findByRole("dialog");
    expect(previewDelete).toHaveBeenCalledWith(["u1"]);
    expect(within(dialog).getByText(/2 items will be moved/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));

    expect(deleteSessions).toHaveBeenCalledWith(["u1"]);
    expect(await screen.findByText(/Moved 1 session to the Trash/)).toBeInTheDocument();
    expect(screen.queryByText("Old work")).not.toBeInTheDocument();
    expect(screen.getByText("Other work")).toBeInTheDocument();
  });

  it("does not let running or read-only sessions be selected", async () => {
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
    expect(screen.getByRole("checkbox", { name: "Select all deletable sessions shown" })).toBeDisabled();
  });

  it("shows sessions the backend refused and only confirms the rest", async () => {
    const user = userEvent.setup();
    scanSessions.mockResolvedValue(scanOf(makeSession({ uid: "a", title: "A" }), makeSession({ uid: "b", title: "B" })));
    previewDelete.mockResolvedValue([
      { uid: "a", title: "A", agentName: "Claude Code", files: ["/a"], refusedReason: null },
      { uid: "b", title: "B", agentName: "Claude Code", files: [], refusedReason: "The session changed since it was scanned." },
    ]);
    deleteSessions.mockResolvedValue([{ status: "trashed", uid: "a", files: ["/a"] }]);

    render(<App />);
    await user.click(await screen.findByRole("checkbox", { name: "Select all deletable sessions shown" }));
    await user.click(screen.getByRole("button", { name: /Move 2 to Trash/ }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText(/Move 1 session to the Trash/)).toBeInTheDocument();
    expect(within(dialog).getByText(/changed since it was scanned/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));

    expect(deleteSessions).toHaveBeenCalledWith(["a"]);
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
    });

    render(<App />);

    expect(await screen.findByText(/Some Pi sessions couldn't be read: \/x: permission denied/)).toBeInTheDocument();
  });

  it("shows an error when scanning fails", async () => {
    scanSessions.mockRejectedValue("boom");

    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  });
});
