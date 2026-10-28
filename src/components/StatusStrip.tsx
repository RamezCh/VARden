// Status filters for the toolbar, and the pill shown in each row.
import type { Status } from "../lib/api";
import { STATUS_META, STATUS_ORDER } from "../lib/filter";

interface Props {
  counts: Record<Status, number>;
  active: ReadonlySet<Status>;
  onToggle: (s: Status) => void;
  onClear: () => void;
}

/** Compact status filters: a dot, a count and a label; click to filter. */
export function StatusStrip({ counts, active, onToggle }: Props) {
  return (
    <div role="group" aria-label="Filter by status" className="flex flex-wrap items-center gap-1">
      {STATUS_ORDER.map((s) => {
        const on = active.has(s);
        const meta = STATUS_META[s];
        return (
          <button
            key={s}
            type="button"
            aria-pressed={on}
            title={meta.help}
            onClick={() => onToggle(s)}
            disabled={counts[s] === 0 && !on}
            className={`flex h-8 items-center gap-2 rounded-md border px-2.5 text-sm transition-colors disabled:opacity-40 ${
              on ? "border-current bg-raised text-fg" : "border-transparent text-muted hover:bg-raised/60 hover:text-fg"
            }`}
            style={on ? { borderColor: meta.color } : undefined}
          >
            <span className="h-2 w-2 shrink-0 rounded-full" style={{ background: meta.color }} />
            <span className="font-mono tabular-nums text-fg">{counts[s]}</span>
            <span>{meta.label}</span>
          </button>
        );
      })}
    </div>
  );
}

export function StatusPill({ status }: { status: Status }) {
  const meta = STATUS_META[status];
  return (
    <span className="inline-flex items-center gap-2 whitespace-nowrap text-sm" title={meta.help}>
      <span className="h-2 w-2 shrink-0 rounded-full" style={{ background: meta.color }} />
      {meta.label}
    </span>
  );
}
