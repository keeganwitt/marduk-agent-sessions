import { labelStyle } from "../lib/labels";
import type { Label } from "../lib/types";

export function LabelChip({ label }: { label: Label }) {
  return (
    <span
      className="label-chip"
      style={labelStyle(label.color)}
      title={label.name}
    >
      {label.name}
    </span>
  );
}

export function LabelChips({
  ids,
  labelsById,
}: {
  ids: number[];
  labelsById: ReadonlyMap<number, Label>;
}) {
  const labels = ids.flatMap((id) => {
    const label = labelsById.get(id);
    return label ? [label] : [];
  });
  if (labels.length === 0) return null;
  return (
    <span className="label-chips">
      {labels.map((label) => (
        <LabelChip key={label.id} label={label} />
      ))}
    </span>
  );
}
