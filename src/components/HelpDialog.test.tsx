import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as ipc from "../lib/ipc";
import { HelpDialog } from "./HelpDialog";

vi.mock("../lib/ipc", () => ({ agentHelp: vi.fn(), appVersion: vi.fn() }));

const agentHelp = vi.mocked(ipc.agentHelp);

describe("HelpDialog", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(ipc.appVersion).mockResolvedValue("1.2.3");
  });

  it("shows the installed version", async () => {
    render(<HelpDialog homeDir="" onClose={() => {}} />);

    expect(await screen.findByText("Marduk 1.2.3")).toBeInTheDocument();
  });

  it("opens on Deleting and switches topics with tabs or arrow keys", async () => {
    const user = userEvent.setup();
    render(<HelpDialog homeDir="" onClose={() => {}} />);

    const tab = (name: string) => screen.getByRole("tab", { name });
    expect(tab("Deleting")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel")).toHaveTextContent("Permanent");

    await user.click(tab("Safety"));
    expect(screen.getByRole("tabpanel")).toHaveTextContent(
      "What Marduk refuses to do",
    );

    await user.keyboard("{ArrowRight}");
    expect(tab("Subagents & labels")).toHaveAttribute("aria-selected", "true");
    expect(tab("Subagents & labels")).toHaveFocus();
  });

  it("explains each agent's delete and archive, with its folders", async () => {
    const user = userEvent.setup();
    agentHelp.mockResolvedValue([
      {
        id: "codex",
        name: "Codex",
        roots: [
          "/Users/me/.codex/sessions",
          "/Users/me/.codex/archived_sessions",
        ],
        delete: "Runs `codex delete --force <id>`, which deletes permanently.",
        archive: "Runs `codex archive <id>`.",
      },
      {
        id: "pi",
        name: "Pi",
        roots: ["/Users/me/.pi/agent/sessions"],
        delete: "Moves the transcript to the Trash.",
        archive: "Pi has no archive.",
      },
    ]);

    render(<HelpDialog homeDir="/Users/me" onClose={() => {}} />);
    await user.click(screen.getByRole("tab", { name: "Agents" }));

    const detail = await screen.findByRole("tabpanel", { name: "Codex" });
    expect(within(detail).getByText("codex delete --force <id>").tagName).toBe(
      "CODE",
    );
    expect(detail).toHaveTextContent("which deletes permanently.");
    expect(detail).toHaveTextContent(
      "~/.codex/sessions~/.codex/archived_sessions",
    );

    await user.click(screen.getByRole("tab", { name: "Pi" }));

    expect(screen.getByRole("tabpanel", { name: "Pi" })).toHaveTextContent(
      "Pi has no archive.",
    );
  });

  it("shows an error if the agents can't be loaded", async () => {
    agentHelp.mockRejectedValue("backend unavailable");

    const user = userEvent.setup();
    render(<HelpDialog homeDir="" onClose={() => {}} />);
    await user.click(screen.getByRole("tab", { name: "Agents" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "backend unavailable",
    );
  });

  it("closes with Done or Escape", async () => {
    const user = userEvent.setup();
    agentHelp.mockResolvedValue([]);
    const onClose = vi.fn();

    render(<HelpDialog homeDir="" onClose={onClose} />);
    await user.click(screen.getByRole("button", { name: "Done" }));
    await user.keyboard("{Escape}");

    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
