import { beforeEach, describe, expect, it } from "vitest";
import {
  COLUMNS,
  DEFAULT_PREFS,
  columnWidth,
  loadColumnPrefs,
  resize,
  saveColumnPrefs,
  setHidden,
  visibleColumns,
} from "./columns";

const column = (id: string) =>
  COLUMNS.find((candidate) => candidate.id === id)!;

describe("column prefs", () => {
  beforeEach(() => localStorage.clear());

  it("defaults to every column at its default width", () => {
    expect(loadColumnPrefs()).toEqual(DEFAULT_PREFS);
    expect(visibleColumns(DEFAULT_PREFS).map((c) => c.id)).toEqual(
      COLUMNS.filter((c) => !c.defaultHidden).map((c) => c.id),
    );
    expect(visibleColumns(DEFAULT_PREFS).map((c) => c.id)).not.toContain(
      "model",
    );
    expect(columnWidth(DEFAULT_PREFS, column("size"))).toBe(
      column("size").defaultWidth,
    );
  });

  it("resizes with a minimum and resets to the default", () => {
    const wider = resize(DEFAULT_PREFS, "project", 400.4);
    expect(columnWidth(wider, column("project"))).toBe(400);
    expect(columnWidth(resize(wider, "project", 5), column("project"))).toBe(
      column("project").minWidth,
    );
    expect(resize(wider, "project", null).widths).toEqual({});
  });

  it("hides and shows columns, but never the session column", () => {
    const hidden = setHidden(DEFAULT_PREFS, "agent", true);
    expect(visibleColumns(hidden).map((c) => c.id)).not.toContain("agent");
    expect(setHidden(hidden, "agent", false).hidden).toEqual([]);
    expect(setHidden(DEFAULT_PREFS, "session", true)).toBe(DEFAULT_PREFS);
  });

  it("shows a hidden-by-default column only once turned on, even with older saved prefs", () => {
    const withModel = setHidden(DEFAULT_PREFS, "model", false);
    expect(withModel.shown).toEqual(["model"]);
    expect(visibleColumns(withModel).map((c) => c.id)).toContain("model");
    expect(setHidden(withModel, "model", true).shown).toEqual([]);

    localStorage.setItem(
      "marduk.columns",
      JSON.stringify({ widths: {}, hidden: ["labels"] }),
    );
    expect(visibleColumns(loadColumnPrefs()).map((c) => c.id)).not.toContain(
      "model",
    );
  });

  it("round-trips through storage and ignores unknown or invalid entries", () => {
    saveColumnPrefs(
      setHidden(resize(DEFAULT_PREFS, "size", 120), "labels", true),
    );
    expect(loadColumnPrefs()).toEqual({
      widths: { size: 120 },
      hidden: ["labels"],
      shown: [],
    });

    localStorage.setItem(
      "marduk.columns",
      JSON.stringify({
        widths: { bogus: 1, size: "x" },
        hidden: ["session", "nope"],
      }),
    );
    expect(loadColumnPrefs()).toEqual(DEFAULT_PREFS);
    localStorage.setItem("marduk.columns", "{ not json");
    expect(loadColumnPrefs()).toEqual(DEFAULT_PREFS);
  });
});
