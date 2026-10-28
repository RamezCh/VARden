// The dashboard's one loud element: how the variables split across the four
// states, as a proportional bar. Each legend entry doubles as a filter.
import type { Status } from "../lib/api";
import { STATUS_META, STATUS_ORDER } from "../lib/filter";

interface Props {
  counts: Record<Status, number>;
  active: ReadonlySet<Status>;
  onToggle: (s: Status) => void;
  onClear: () => void;
}

export function StatusStrip({ counts, active, onToggle, onClear }: Props) {
  const total = STATUS_ORDER.reduce((n, s) => n + counts[s], 0);
  const filtering = active.size > 0;

  return (
    <section aria-label="Status overview" className="flex flex-col gap-3">
      <div className="flex h-3 w-full gap-[3px]" aria-hidden>
        {total === 0 ? (
          <div className="h-full w-full rounded-full bg-rule" />
        ) : (
          STATUS_ORDER.filter((s) => counts[s] > 0).map((s) => (
            <div
              key={s}
              className="h-full rounded-full transition-opacity duration-200 first:rounded-l-full last:rounded-r-full"
              style={{
                flexGrow: counts[s],
                flexBasis: 0,
                minWidth: 10,
                background: STATUS_META[s].color,
                opacity: filtering && !active.has(s) ? 0.2 : 1,
              }}
            />
          ))
        )}
      </div>

      <div className="flex flex-wrap items-center gap-x-1 gap-y-1">
        {STATUS_ORDER.map((s) => {
          const on = active.has(s);
          return (
            <button
              key={s}
              type="button"
              aria-pressed={on}
              title={STATUS_META[s].help}
              onClick={() => onToggle(s)}
              disabled={counts[s] === 0 && !on}
              className={`flex items-center gap-2 rounded-md px-2.5 py-1.5 text-sm transition-colors disabled:cursor-default disabled:opacity-40 ${
                on ? "bg-raised text-fg" : "text-muted hover:bg-panel hover:text-fg"
              }`}
            >
              <span className="h-2.5 w-2.5 rounded-full" style={{ background: STATUS_META[s].color }} />
              <span className="font-mono tabular-nums text-fg">{counts[s]}</span>
              {STATUS_META[s].label}
            </button>
          );
        })}
        {filtering && (
          <button type="button" onClick={onClear} className="ml-1 px-2 py-1.5 text-sm text-muted underline-offset-4 hover:text-fg hover:underline">
            Show all
          </button>
        )}
      </div>
    </section>
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
