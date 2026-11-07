import type { SessionRow } from "./types";

/**
 * The ids from `anchor` to `target` inclusive, in the order shown, for shift-click range
 * selection. Falls back to just `target` when the anchor is no longer shown.
 */
export function rangeBetween(
  order: readonly string[],
  anchor: string | null,
  target: string,
): string[] {
  const end = order.indexOf(target);
  const start = anchor === null ? -1 : order.indexOf(anchor);
  if (start < 0 || end < 0) return [target];
  const [from, to] = start <= end ? [start, end] : [end, start];
  return order.slice(from, to + 1);
}

/**
 * `chosen` plus the subagent sessions under them that the agent's own delete leaves behind
 * (Codex Guardian reviews), so deleting a parent takes its whole conversation.
 */
export function withLeftoverSubagents(
  chosen: readonly SessionRow[],
  all: readonly SessionRow[],
): SessionRow[] {
  const result = [...chosen];
  const included = new Set(chosen.map((session) => session.uid));
  const pending = [...chosen];
  for (let parent = pending.pop(); parent; parent = pending.pop()) {
    for (const child of all) {
      if (
        child.agent !== parent.agent ||
        child.parentId !== parent.id ||
        child.partOfParent ||
        included.has(child.uid)
      ) {
        continue;
      }
      included.add(child.uid);
      pending.push(child);
      if (!child.deletedWithParent) result.push(child);
    }
  }
  return result;
}
