export type ColumnId =
  | "session"
  | "labels"
  | "agent"
  | "agentType"
  | "model"
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
  /** Shown only once turned on in Columns. */
  defaultHidden?: boolean;
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
    id: "agentType",
    label: "Agent type",
    defaultWidth: 130,
    minWidth: 60,
    hideable: true,
    numeric: false,
    defaultHidden: true,
  },
  {
    id: "model",
    label: "Model",
    defaultWidth: 140,
    minWidth: 60,
    hideable: true,
    numeric: false,
    defaultHidden: true,
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
    defaultWidth: 105,
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
  /** Columns hidden by default that were turned on. */
  shown: ColumnId[];
}

const STORAGE_KEY = "marduk.columns";

export const DEFAULT_PREFS: ColumnPrefs = { widths: {}, hidden: [], shown: [] };

function column(id: ColumnId): ColumnDef {
  return COLUMNS.find((candidate) => candidate.id === id)!;
}

function isColumnId(value: unknown): value is ColumnId {
  return COLUMNS.some((column) => column.id === value);
}

/** Prefs from storage, keeping only known columns and valid values. */
export function parseColumnPrefs(value: unknown): ColumnPrefs {
  if (!value || typeof value !== "object") return DEFAULT_PREFS;
  const parsed = value as Partial<Record<keyof ColumnPrefs, unknown>>;
  const widths: ColumnPrefs["widths"] = {};
  const savedWidths =
    parsed.widths && typeof parsed.widths === "object" ? parsed.widths : {};
  for (const [id, width] of Object.entries(savedWidths)) {
    if (isColumnId(id) && typeof width === "number" && Number.isFinite(width))
      widths[id] = width;
  }
  const ids = (list: unknown): unknown[] => (Array.isArray(list) ? list : []);
  const hidden = ids(parsed.hidden).filter(
    (id): id is ColumnId => isColumnId(id) && column(id).hideable,
  );
  const shown = ids(parsed.shown).filter(
    (id): id is ColumnId => isColumnId(id) && column(id).defaultHidden === true,
  );
  return { widths, hidden, shown };
}

/** The copy in the webview's storage, for the first paint before settings load. */
export function loadColumnPrefs(): ColumnPrefs {
  try {
    return parseColumnPrefs(
      JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null"),
    );
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

export function isHidden(prefs: ColumnPrefs, def: ColumnDef): boolean {
  return def.defaultHidden
    ? !prefs.shown.includes(def.id)
    : prefs.hidden.includes(def.id);
}

export function visibleColumns(prefs: ColumnPrefs): ColumnDef[] {
  return COLUMNS.filter((def) => !isHidden(prefs, def));
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
  const def = column(id);
  if (!def.hideable) return prefs;
  if (def.defaultHidden) {
    const rest = prefs.shown.filter((candidate) => candidate !== id);
    return { ...prefs, shown: hidden ? rest : [...rest, id] };
  }
  const rest = prefs.hidden.filter((candidate) => candidate !== id);
  return { ...prefs, hidden: hidden ? [...rest, id] : rest };
}

/**
 * Every shown column's width: the saved widths, except that with `fit`, Session shrinks
 * (never below its minimum, and never wider than saved) so the table fits in `available`.
 * Fitting follows window and screen changes; a Session width the user just dragged wins.
 */
export function layoutWidths(
  prefs: ColumnPrefs,
  available: number | null,
  fixed: number,
  fit: boolean,
): Map<ColumnId, number> {
  const widths = new Map<ColumnId, number>();
  let others = fixed;
  for (const def of visibleColumns(prefs)) {
    const width = columnWidth(prefs, def);
    widths.set(def.id, width);
    if (def.id !== "session") others += width;
  }
  const session = column("session");
  const saved = widths.get("session")!;
  if (fit && available !== null) {
    widths.set(
      "session",
      Math.max(
        session.minWidth,
        Math.min(saved, Math.floor(available - others)),
      ),
    );
  }
  return widths;
}
