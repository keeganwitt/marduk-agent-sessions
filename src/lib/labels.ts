import type { Label } from "./types";

/** Must match `labels::COLORS` in marduk-core. Fills are solid so labels never look like the
 * tinted status badges; text colors are chosen for contrast on each fill. */
export const LABEL_COLORS: Record<string, { fill: string; text: string; name: string }> = {
  gray: { fill: "#5f6368", text: "#ffffff", name: "Gray" },
  red: { fill: "#d93025", text: "#ffffff", name: "Red" },
  orange: { fill: "#e8710a", text: "#ffffff", name: "Orange" },
  yellow: { fill: "#f9ab00", text: "#202124", name: "Yellow" },
  green: { fill: "#188038", text: "#ffffff", name: "Green" },
  teal: { fill: "#129eaf", text: "#ffffff", name: "Teal" },
  blue: { fill: "#1a73e8", text: "#ffffff", name: "Blue" },
  purple: { fill: "#9334e6", text: "#ffffff", name: "Purple" },
  pink: { fill: "#d01884", text: "#ffffff", name: "Pink" },
};

export const COLOR_NAMES = Object.keys(LABEL_COLORS);

export function labelStyle(color: string): { background: string; color: string } {
  const palette = LABEL_COLORS[color] ?? LABEL_COLORS.gray;
  return { background: palette.fill, color: palette.text };
}

/** Rotates through the palette so new labels start out distinguishable. */
export function nextColor(labels: Label[]): string {
  return COLOR_NAMES[(labels.length + 1) % COLOR_NAMES.length];
}

export type LabelState = "all" | "some" | "none";

/** Whether all, some or none of `sessions` have the label, for the tri-state menu checkboxes. */
export function labelState(labelId: number, sessions: { labelIds: number[] }[]): LabelState {
  const count = sessions.filter((session) => session.labelIds.includes(labelId)).length;
  if (count === 0) return "none";
  return count === sessions.length ? "all" : "some";
}
