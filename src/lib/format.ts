const UNITS = ["B", "KB", "MB", "GB", "TB"];
const DAY_MS = 24 * 60 * 60 * 1000;

/** A size such as "1.2 MB", with the decimal separator of `locale` when given. */
export function formatBytes(bytes: number, locale?: string): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 10 ? 0 : 1;
  const number = new Intl.NumberFormat(locale ?? "en", {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
    useGrouping: false,
  }).format(value);
  return `${number} ${UNITS[unit]}`;
}

export function ageInDays(isoTimestamp: string, now: Date): number {
  return Math.max(
    0,
    (now.getTime() - new Date(isoTimestamp).getTime()) / DAY_MS,
  );
}

export type AgeUnit = "now" | "minutes" | "hours" | "days" | "months" | "years";

/** How long ago `isoTimestamp` was, in the largest unit that reads naturally. */
export function ageParts(
  isoTimestamp: string,
  now: Date,
): { unit: AgeUnit; count: number } {
  const days = ageInDays(isoTimestamp, now);
  const minutes = days * 24 * 60;
  if (minutes < 1) return { unit: "now", count: 0 };
  if (minutes < 60) return { unit: "minutes", count: Math.floor(minutes) };
  if (days < 1) return { unit: "hours", count: Math.floor(days * 24) };
  if (days < 60) return { unit: "days", count: Math.floor(days) };
  if (days < 730) return { unit: "months", count: Math.floor(days / 30) };
  return { unit: "years", count: Math.floor(days / 365) };
}

const ENGLISH_AGES: Record<AgeUnit, string> = {
  now: "just now",
  minutes: "m ago",
  hours: "h ago",
  days: "d ago",
  months: "mo ago",
  years: "y ago",
};

/** The English age; the UI uses `useI18n().age`, which follows the language setting. */
export function formatAge(isoTimestamp: string, now: Date): string {
  const { unit, count } = ageParts(isoTimestamp, now);
  return unit === "now" ? ENGLISH_AGES.now : `${count}${ENGLISH_AGES[unit]}`;
}

/**
 * `~` for the home folder, and for long paths only the root plus the last two folders, so a
 * shortened path still reads as absolute (`~/…/team/app`, `/…/c/d`, `C:\\…\\team\\app`).
 */
export function shortenPath(path: string, homeDir: string): string {
  const display = tildePath(path, homeDir);
  const inHome = display !== path;
  const separator =
    display.includes("\\") && !display.includes("/") ? "\\" : "/";
  const parts = display.split(/[\\/]/).filter(Boolean);
  const root = inHome ? "~" : /^[A-Za-z]:/.test(display) ? parts[0] : "";
  const rest = root === "" ? parts : parts.slice(1);
  if (rest.length <= 3) return display;
  return [root, "…", ...rest.slice(-2)].join(separator);
}

/** `path` with the home folder written as `~`. */
export function tildePath(path: string, homeDir: string): string {
  const home = homeDir.replace(/[\\/]+$/, "");
  const inHome =
    home !== "" &&
    (path === home ||
      path.startsWith(`${home}/`) ||
      path.startsWith(`${home}\\`));
  return inHome ? `~${path.slice(home.length)}` : path;
}
