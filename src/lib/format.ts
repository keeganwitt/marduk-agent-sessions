const UNITS = ["B", "KB", "MB", "GB", "TB"];
const DAY_MS = 24 * 60 * 60 * 1000;

export function formatBytes(bytes: number): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

export function ageInDays(isoTimestamp: string, now: Date): number {
  return Math.max(
    0,
    (now.getTime() - new Date(isoTimestamp).getTime()) / DAY_MS,
  );
}

export function formatAge(isoTimestamp: string, now: Date): string {
  const days = ageInDays(isoTimestamp, now);
  const minutes = days * 24 * 60;
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${Math.floor(minutes)}m ago`;
  if (days < 1) return `${Math.floor(days * 24)}h ago`;
  if (days < 60) return `${Math.floor(days)}d ago`;
  if (days < 730) return `${Math.floor(days / 30)}mo ago`;
  return `${Math.floor(days / 365)}y ago`;
}

/**
 * `~` for the home folder, and for long paths only the root plus the last two folders, so a
 * shortened path still reads as absolute (`~/…/team/app`, `/…/c/d`, `C:\\…\\team\\app`).
 */
export function shortenPath(path: string, homeDir: string): string {
  const home = homeDir.replace(/[\\/]+$/, "");
  const inHome =
    home !== "" &&
    (path === home ||
      path.startsWith(`${home}/`) ||
      path.startsWith(`${home}\\`));
  const display = inHome ? `~${path.slice(home.length)}` : path;
  const separator =
    display.includes("\\") && !display.includes("/") ? "\\" : "/";
  const parts = display.split(/[\\/]/).filter(Boolean);
  const root = inHome ? "~" : /^[A-Za-z]:/.test(display) ? parts[0] : "";
  const rest = root === "" ? parts : parts.slice(1);
  if (rest.length <= 3) return display;
  return [root, "…", ...rest.slice(-2)].join(separator);
}
