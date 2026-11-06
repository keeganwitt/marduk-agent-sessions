import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyTheme, cachedTheme, isTheme } from "./theme";

const setTheme = vi.fn(() => Promise.resolve());
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ setTheme }) }));

describe("theme", () => {
  beforeEach(() => {
    localStorage.clear();
    setTheme.mockClear();
  });

  it("defaults to system and remembers the applied theme", () => {
    expect(cachedTheme()).toBe("system");
    applyTheme("dark");
    expect(cachedTheme()).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("sets the native window theme, clearing it for system", () => {
    applyTheme("light");
    expect(setTheme).toHaveBeenLastCalledWith("light");
    applyTheme("system");
    expect(setTheme).toHaveBeenLastCalledWith(null);
  });

  it("ignores unknown cached values", () => {
    localStorage.setItem("marduk.theme", "neon");
    expect(cachedTheme()).toBe("system");
    expect(isTheme("neon")).toBe(false);
  });
});
