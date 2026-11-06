import { getCurrentWindow } from "@tauri-apps/api/window";

export type Theme = "system" | "light" | "dark";

export const THEMES: readonly Theme[] = ["system", "light", "dark"];

const CACHE_KEY = "marduk.theme";

export function isTheme(value: unknown): value is Theme {
  return typeof value === "string" && (THEMES as readonly string[]).includes(value);
}

/** The last applied theme, so it can be set before the first render without a flash. */
export function cachedTheme(): Theme {
  try {
    const value = localStorage.getItem(CACHE_KEY);
    return isTheme(value) ? value : "system";
  } catch {
    return "system";
  }
}

/** Applies the theme to the page and the native window chrome (title bar, scrollbars). */
export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(CACHE_KEY, theme);
  } catch {
    // Storage can be unavailable; the theme still applies for this session.
  }
  try {
    void getCurrentWindow()
      .setTheme(theme === "system" ? null : theme)
      .catch(() => {});
  } catch {
    // Outside Tauri (tests, plain browser) there's no native window to theme.
  }
}
