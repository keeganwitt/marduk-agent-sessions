import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LATEST_RELEASE_API } from "../lib/updates";
import { UpdateCheck } from "./UpdateCheck";

const url =
  "https://github.com/keeganwitt/marduk-agent-sessions/releases/tag/v1.3.0";
const response = (version: string) =>
  new Response(
    JSON.stringify({
      tag_name: version,
      html_url: `https://github.com/keeganwitt/marduk-agent-sessions/releases/tag/${version}`,
    }),
  );

afterEach(() => vi.unstubAllGlobals());

describe("UpdateCheck", () => {
  it("checks only when clicked and offers the newer release in the browser", async () => {
    const fetcher = vi.fn().mockResolvedValue(response("v1.3.0"));
    vi.stubGlobal("fetch", fetcher);
    render(<UpdateCheck version="1.2.3" versionError={false} />);
    expect(fetcher).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("button", { name: "Check for updates" }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Version 1.3.0 is available.",
    );
    expect(screen.getByRole("link", { name: "Download" })).toHaveAttribute(
      "href",
      url,
    );
    expect(fetcher).toHaveBeenCalledWith(
      LATEST_RELEASE_API,
      expect.any(Object),
    );
  });

  it.each(["1.2.3", "1.0.0"])(
    "reports no newer release for %s",
    async (version) => {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValue(response(version)));
      render(<UpdateCheck version="1.2.3" versionError={false} />);
      await userEvent.click(
        screen.getByRole("button", { name: "Check for updates" }),
      );
      expect(await screen.findByRole("status")).toHaveTextContent(
        "You're running the latest version.",
      );
    },
  );

  it("disables checks while pending and clears errors on retry", async () => {
    let resolve!: (value: Response) => void;
    const fetcher = vi
      .fn()
      .mockRejectedValueOnce(new TypeError("offline"))
      .mockImplementationOnce(
        () =>
          new Promise<Response>((done) => {
            resolve = done;
          }),
      );
    vi.stubGlobal("fetch", fetcher);
    const user = userEvent.setup();
    render(<UpdateCheck version="1.2.3" versionError={false} />);
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not reach GitHub",
    );
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Checking…" })).toBeDisabled();
    await act(async () => resolve(response("1.3.0")));
    expect(
      screen.getByRole("button", { name: "Check for updates" }),
    ).toBeEnabled();
    expect(screen.getByRole("link", { name: "Download" })).toBeInTheDocument();
  });

  it("disables checks until the installed version is loaded", () => {
    render(<UpdateCheck version={null} versionError={false} />);
    expect(
      screen.getByRole("button", { name: "Check for updates" }),
    ).toBeDisabled();
  });

  it("explains an installed-version load failure", () => {
    render(<UpdateCheck version={null} versionError />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Could not read the installed version",
    );
    expect(
      screen.getByRole("button", { name: "Check for updates" }),
    ).toBeDisabled();
  });
});
