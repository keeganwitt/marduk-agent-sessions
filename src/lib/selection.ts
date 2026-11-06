/**
 * The ids from `anchor` to `target` inclusive, in the order shown, for shift-click range
 * selection. Falls back to just `target` when the anchor is no longer shown.
 */
export function rangeBetween(order: readonly string[], anchor: string | null, target: string): string[] {
  const end = order.indexOf(target);
  const start = anchor === null ? -1 : order.indexOf(anchor);
  if (start < 0 || end < 0) return [target];
  const [from, to] = start <= end ? [start, end] : [end, start];
  return order.slice(from, to + 1);
}
