import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import * as ipc from "../lib/ipc";
import type { CliStatus } from "../lib/types";
import { SettingsDialog } from "./SettingsDialog";

vi.mock("../lib/ipc", () => ({
  cliStatuses: vi.fn(),
  setCliPath: vi.fn(),
  saveTheme: vi.fn(),
  getLanguage: vi.fn(),
  saveLanguage: vi.fn(),
  listLabels: vi.fn(),
  updateLabel: vi.fn(),
  deleteLabel: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const cliStatuses = vi.mocked(ipc.cliStatuses);
const setCliPath = vi.mocked(ipc.setCliPath);
const openDialog = vi.mocked(open);
const saveTheme = vi.mocked(ipc.saveTheme);

function status(overrides: Partial<CliStatus>): CliStatus {
  return {
    agent: "codex",
    agentName: "Codex",
    program: "codex",
    installUrl: "https://github.com/openai/codex",
    path: null,
    overridden: false,
    version: null,
    problem: null,
    ...overrides,
  };
}

describe("SettingsDialog", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    saveTheme.mockResolvedValue();
    vi.mocked(ipc.listLabels).mockResolvedValue([]);
    localStorage.clear();
  });

  it("renames, recolors and deletes labels after confirming", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    cliStatuses.mockResolvedValue([]);
    const listLabels = vi.mocked(ipc.listLabels);
    listLabels.mockResolvedValue([
      { id: 1, name: "Work", color: "blue", count: 3 },
    ]);
    vi.mocked(ipc.updateLabel).mockResolvedValue();
    vi.mocked(ipc.deleteLabel).mockResolvedValue();

    render(<SettingsDialog onClose={onClose} />);
    const name = await screen.findByRole("textbox", { name: "Name of Work" });
    await user.clear(name);
    await user.type(name, "Job{Enter}");
    expect(ipc.updateLabel).toHaveBeenCalledWith(1, "Job", "blue");

    await user.click(screen.getByRole("radio", { name: "Red" }));
    expect(ipc.updateLabel).toHaveBeenCalledWith(1, "Work", "red");

    await user.click(screen.getByRole("button", { name: "Delete label Work" }));
    expect(ipc.deleteLabel).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole("button", { name: "Delete from 3 sessions" }),
    );
    expect(ipc.deleteLabel).toHaveBeenCalledWith(1);

    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(onClose).toHaveBeenCalledWith(true);
  });

  it("defaults to the system theme and applies and saves a chosen one", async () => {
    const user = userEvent.setup();
    cliStatuses.mockResolvedValue([]);

    render(<SettingsDialog onClose={() => {}} />);

    expect(screen.getByRole("radio", { name: "System" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "Dark" }));

    expect(screen.getByRole("radio", { name: "Dark" })).toBeChecked();
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(saveTheme).toHaveBeenCalledWith("dark");
  });

  it("shows found and missing CLIs with an install link for the missing one", async () => {
    cliStatuses.mockResolvedValue([
      status({
        path: "/Users/me/.local/bin/codex",
        version: "codex-cli 0.166.0",
      }),
      status({
        agent: "opencode",
        agentName: "OpenCode",
        program: "opencode",
        installUrl: "https://opencode.ai/docs/",
      }),
    ]);

    render(<SettingsDialog onClose={() => {}} />);

    expect(
      await screen.findByText("/Users/me/.local/bin/codex"),
    ).toBeInTheDocument();
    expect(screen.getByText(/codex-cli 0\.166\.0/)).toBeInTheDocument();
    expect(screen.getByText("Found automatically")).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "How to install opencode" }),
    ).toHaveAttribute("href", "https://opencode.ai/docs/");
  });

  it("saves a chosen path and reports the change on close", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    cliStatuses.mockResolvedValue([status({})]);
    openDialog.mockResolvedValue("/opt/codex/bin/codex");
    setCliPath.mockResolvedValue([
      status({
        path: "/opt/codex/bin/codex",
        overridden: true,
        version: "codex-cli 1.0",
      }),
    ]);

    render(<SettingsDialog onClose={onClose} />);
    await user.click(await screen.findByRole("button", { name: "Choose…" }));

    expect(setCliPath).toHaveBeenCalledWith("codex", "/opt/codex/bin/codex");
    expect(await screen.findByText("Custom location")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(onClose).toHaveBeenCalledWith(true);
  });

  it("resets a custom location and shows save errors", async () => {
    const user = userEvent.setup();
    cliStatuses.mockResolvedValue([
      status({ path: "/opt/codex", overridden: true }),
    ]);
    setCliPath.mockRejectedValue("/opt/codex isn't an executable file.");

    render(<SettingsDialog onClose={() => {}} />);
    const row = (await screen.findByText("Custom location")).closest("li")!;
    await user.click(
      within(row).getByRole("button", { name: "Use automatic" }),
    );

    expect(setCliPath).toHaveBeenCalledWith("codex", null);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "isn't an executable file",
    );
  });

  it("does nothing when the file picker is cancelled", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    cliStatuses.mockResolvedValue([status({})]);
    openDialog.mockResolvedValue(null);

    render(<SettingsDialog onClose={onClose} />);
    await user.click(await screen.findByRole("button", { name: "Choose…" }));
    await user.keyboard("{Escape}");

    expect(setCliPath).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledWith(false);
  });
});
