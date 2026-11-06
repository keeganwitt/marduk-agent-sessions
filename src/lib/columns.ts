export type ColumnId =
  | "session"
  | "labels"
  | "agent"
  | "project"
  | "created"
  | "lastActivity"
  | "size";

export interface ColumnDef {
  id: ColumnId;
  label: string;
  defaultWidth: number;
  minWidth: number;
  /** Session can't be hidden: it holds the title, expander and transcript link. */
  hideable: boolean;
  numeric: boolean;
}

export const COLUMNS: readonly ColumnDef[] = [
  {
    id: "session",
    label: "Session",
    defaultWidth: 380,
    minWidth: 160,
    hideable: false,
    numeric: false,
  },
  {
    id: "labels",
    label: "Labels",
    defaultWidth: 150,
    minWidth: 60,
    hideable: true,
    numeric: false,
  },
  {
    id: "agent",
    label: "Agent",
    defaultWidth: 120,
    minWidth: 60,
    hideable: true,
    numeric: false,
  },
  {
    id: "project",
    label: "Project",
    defaultWidth: 240,
    minWidth: 80,
    hideable: true,
    numeric: false,
  },
  {
    id: "created",
    label: "Created",
    defaultWidth: 90,
    minWidth: 60,
    hideable: true,
    numeric: true,
  },
  {
    id: "lastActivity",
    label: "Last activity",
    defaultWidth: 110,
    minWidth: 70,
    hideable: true,
    numeric: true,
  },
  {
    id: "size",
    label: "Size",
    defaultWidth: 80,
    minWidth: 56,
    hideable: true,
    numeric: true,
  },
];

export interface ColumnPrefs {
  widths: Partial<Record<ColumnId, number>>;
  hidden: ColumnId[];
}

const STORAGE_KEY = "marduk.columns";

export const DEFAULT_PREFS: ColumnPrefs = { widths: {}, hidden: [] };

function isColumnId(value: unknown): value is ColumnId {
  return COLUMNS.some((column) => column.id === value);
}

export function loadColumnPrefs(): ColumnPrefs {
  try {
    const parsed = JSON.parse(
      localStorage.getItem(STORAGE_KEY) ?? "null",
    ) as Partial<ColumnPrefs> | null;
    if (!parsed || typeof parsed !== "object") return DEFAULT_PREFS;
    const widths: ColumnPrefs["widths"] = {};
    for (const [id, width] of Object.entries(parsed.widths ?? {})) {
      if (isColumnId(id) && typeof width === "number" && Number.isFinite(width))
        widths[id] = width;
    }
    const hidden = (parsed.hidden ?? []).filter(
      (id): id is ColumnId =>
        isColumnId(id) && COLUMNS.find((column) => column.id === id)!.hideable,
    );
    return { widths, hidden };
  } catch {
    return DEFAULT_PREFS;
  }
}

export function saveColumnPrefs(prefs: ColumnPrefs): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // Storage can be unavailable; the layout still applies for this session.
  }
}

export function columnWidth(prefs: ColumnPrefs, column: ColumnDef): number {
  return Math.max(
    column.minWidth,
    prefs.widths[column.id] ?? column.defaultWidth,
  );
}

export function visibleColumns(prefs: ColumnPrefs): ColumnDef[] {
  return COLUMNS.filter((column) => !prefs.hidden.includes(column.id));
}

export function resize(
  prefs: ColumnPrefs,
  id: ColumnId,
  width: number | null,
): ColumnPrefs {
  const column = COLUMNS.find((candidate) => candidate.id === id)!;
  const widths = { ...prefs.widths };
  if (width === null) delete widths[id];
  else widths[id] = Math.max(column.minWidth, Math.round(width));
  return { ...prefs, widths };
}

export function setHidden(
  prefs: ColumnPrefs,
  id: ColumnId,
  hidden: boolean,
): ColumnPrefs {
  const column = COLUMNS.find((candidate) => candidate.id === id)!;
  if (!column.hideable) return prefs;
  const rest = prefs.hidden.filter((candidate) => candidate !== id);
  return { ...prefs, hidden: hidden ? [...rest, id] : rest };
}
