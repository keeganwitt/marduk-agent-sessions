// @vitest-environment node
import { describe, expect, it } from "vitest";
import { resolve } from "node:path";

import { commands, parseArgs } from "./demo.mjs";

describe("parseArgs", () => {
  it("defaults to a padded demo in the given folder, run with tauri dev", () => {
    expect(parseArgs([], "/tmp/demo")).toEqual({
      dir: "/tmp/demo",
      app: undefined,
      padding: true,
    });
  });

  it("reads the folder, app and padding options, skipping pnpm's --", () => {
    expect(
      parseArgs(["--", "--dir", "d", "--app", "bin/marduk", "--no-padding"]),
    ).toEqual({
      dir: resolve("d"),
      app: resolve("bin/marduk"),
      padding: false,
    });
  });

  it("rejects unknown options and missing values", () => {
    expect(() => parseArgs(["--size"])).toThrow("Unknown option --size");
    expect(() => parseArgs(["--dir"])).toThrow("--dir needs a value");
    expect(() => parseArgs(["--app", "--no-padding"])).toThrow(
      "--app needs a value",
    );
  });
});

describe("commands", () => {
  it("generates the demo home and starts tauri dev", () => {
    const { generate, launch } = commands(
      { dir: "/tmp/demo", padding: true },
      "darwin",
    );

    expect(generate.slice(-3)).toEqual(["--", "/tmp/demo", "--json"]);
    expect(launch).toEqual({
      command: "pnpm",
      args: ["tauri", "dev"],
      shell: false,
    });
  });

  it("starts pnpm through a shell on Windows", () => {
    expect(commands({ dir: "d", padding: true }, "win32").launch.shell).toBe(
      true,
    );
  });

  it("runs a built app and passes --no-padding through", () => {
    const { generate, launch } = commands(
      { dir: "d", app: "/apps/marduk", padding: false },
      "linux",
    );

    expect(generate.at(-1)).toBe("--no-padding");
    expect(launch).toEqual({ command: "/apps/marduk", args: [], shell: false });
  });
});
