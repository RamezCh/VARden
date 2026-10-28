import { useState, type RefObject } from "react";
import type { EnvVar, Scope, Status } from "../lib/api";
import { countByStatus, filterVars, rowId, sortVars, type ScopeFilter, type SortKey, type SortState } from "../lib/filter";
import { Button } from "./IconButton";
import { EyeIcon, EyeOffIcon, PlusIcon, SearchIcon, SyncIcon } from "./icons";
import { StatusStrip } from "./StatusStrip";
import { Help } from "./Help";
import { HELP } from "../lib/helpText";
import { VariableTable } from "./VariableTable";

interface Props {
  vars: EnvVar[];
  loading: boolean;
  busy: boolean;
  creating: boolean;
  setCreating: (v: boolean) => void;
  searchRef: RefObject<HTMLInputElement | null>;
  onSet: (key: string, value: string, scope: Scope, existed: boolean) => Promise<boolean>;
  onDelete: (key: string, scope: Scope) => Promise<boolean>;
  onOpenImport: () => void;
  onOpenExport: (visible: EnvVar[]) => void;
  /** Number of shells with the terminal-sync hook installed. */
  hooksInstalled: number;
  onSync: () => void;
  onSetUpSync: () => void;
}

const SCOPES: { value: ScopeFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "user", label: "User" },
  { value: "process", label: "Temporary" },
];

export function VariablesView({
  vars,
  loading,
  busy,
  creating,
  setCreating,
  searchRef,
  onSet,
  onDelete,
  onOpenImport,
  onOpenExport,
  hooksInstalled,
  onSync,
  onSetUpSync,
}: Props) {
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<ScopeFilter>("all");
  const [statuses, setStatuses] = useState<ReadonlySet<Status>>(new Set());
  const [sort, setSort] = useState<SortState>({ key: "key", dir: "asc" });
  const [revealAll, setRevealAll] = useState(false);
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(new Set());

  const isRevealed = (v: EnvVar) => revealAll || revealed.has(rowId(v));
  const isVisible = (v: EnvVar) => !v.sensitive || isRevealed(v);

  // Counts follow the scope filter, so the strip always describes what the
  // status buttons would show.
  const counts = countByStatus(vars.filter((v) => scope === "all" || v.scope === scope));
  const pending = vars.filter((v) => v.pendingSync).length;
  const rows = sortVars(filterVars(vars, { query, scope, statuses }, isVisible), sort);
  const filtered = query.trim() !== "" || scope !== "all" || statuses.size > 0;

  const toggleStatus = (s: Status) =>
    setStatuses((prev) => {
      const next = new Set(prev);
      if (next.has(s)) next.delete(s);
      else next.add(s);
      return next;
    });

  const onSort = (key: SortKey) =>
    setSort((s) => (s.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: "asc" }));

  const clearFilters = () => {
    setQuery("");
    setScope("all");
    setStatuses(new Set());
  };

  const empty = loading ? (
    <p className="text-sm text-muted">Reading your environment…</p>
  ) : filtered ? (
    <div className="flex flex-col items-center gap-2">
      <p className="text-sm text-muted">No variables match these filters.</p>
      <Button onClick={clearFilters}>Clear filters</Button>
    </div>
  ) : (
    <div className="flex flex-col items-center gap-3">
      <p className="max-w-[44ch] text-sm text-muted">
        You haven't saved any user variables yet. Add one and every new terminal will pick it up.
      </p>
      <Button variant="primary" onClick={() => setCreating(true)}>
        <PlusIcon /> Add a variable
      </Button>
    </div>
  );

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">Variables</h1>
          <p className="mt-1 text-sm text-muted">Your saved user variables, plus temporary ones for open terminals.</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button onClick={onOpenImport}>Import .env</Button>
          <Button onClick={() => onOpenExport(rows)} disabled={vars.length === 0}>
            Export .env
          </Button>
          {hooksInstalled > 0 ? (
            <>
            <Button
              className="border border-rule"
              onClick={onSync}
              disabled={busy}
              title="Push saved variables into open terminals that have terminal sync"
            >
              <SyncIcon />
              Sync to terminals
              {pending > 0 && (
                <span
                  className="ml-0.5 rounded-full bg-persistent/20 px-1.5 font-mono text-xs text-persistent"
                  title={`${pending} change${pending === 1 ? "" : "s"} not in open terminals yet`}
                >
                  {pending}
                </span>
              )}
            </Button>
            <Help label="What does Sync to terminals do?">{HELP.sync}</Help>
            </>
          ) : (
            <>
            <Button className="border border-rule" onClick={onSetUpSync} title="Install the terminal hook for your shells">
              <SyncIcon />
              Set up terminal sync
            </Button>
            <Help label="What is terminal sync?">{HELP.sync}</Help>
            </>
          )}
          <Button variant="primary" onClick={() => setCreating(true)} disabled={creating} title="New variable (N)">
            <PlusIcon /> New variable
          </Button>
        </div>
      </div>

      <StatusStrip counts={counts} active={statuses} onToggle={toggleStatus} onClear={() => setStatuses(new Set())} />

      <div className="flex flex-wrap items-center gap-3">
        <label className="relative flex min-w-56 flex-1 items-center">
          <SearchIcon className="pointer-events-none absolute left-2.5 text-faint" />
          <span className="sr-only">Search variables</span>
          <input
            ref={searchRef}
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setQuery("")}
            placeholder="Search names and visible values"
            className="h-9 w-full rounded-md border border-rule bg-panel pl-8 pr-10 text-sm text-fg placeholder:text-faint focus:border-process focus:outline-none"
          />
          <kbd className="pointer-events-none absolute right-2.5 rounded border border-rule px-1.5 font-mono text-[0.7rem] text-faint">/</kbd>
        </label>

        <div role="radiogroup" aria-label="Scope" className="flex h-9 items-center rounded-md border border-rule bg-panel p-0.5">
          {SCOPES.map((s) => (
            <button
              key={s.value}
              type="button"
              role="radio"
              aria-checked={scope === s.value}
              onClick={() => setScope(s.value)}
              className={`h-full rounded px-3 text-sm transition-colors ${
                scope === s.value ? "bg-raised text-fg" : "text-muted hover:text-fg"
              }`}
            >
              {s.label}
            </button>
          ))}
        </div>

        <Button
          className="h-9 border border-rule"
          aria-pressed={revealAll}
          onClick={() => {
            setRevealAll(!revealAll);
            setRevealed(new Set());
          }}
        >
          {revealAll ? <EyeOffIcon /> : <EyeIcon />}
          {revealAll ? "Hide secrets" : "Show secrets"}
        </Button>
      </div>

      <VariableTable
        rows={rows}
        allVars={vars}
        sort={sort}
        onSort={onSort}
        isRevealed={isRevealed}
        onToggleReveal={(v) =>
          setRevealed((prev) => {
            const next = new Set(prev);
            const id = rowId(v);
            if (isRevealed(v)) {
              if (revealAll) {
                // Hiding one while "show all" is on: switch to per-row mode.
                setRevealAll(false);
                vars.filter((x) => x.sensitive && rowId(x) !== id).forEach((x) => next.add(rowId(x)));
              }
              next.delete(id);
            } else next.add(id);
            return next;
          })
        }
        creating={creating}
        onCloseCreate={() => setCreating(false)}
        busy={busy}
        onSet={onSet}
        onDelete={onDelete}
        empty={empty}
      />

      {rows.length > 0 && filtered && (
        <p className="-mt-3 text-sm text-muted">
          Showing {rows.length} of {vars.length}.{" "}
          <button type="button" onClick={clearFilters} className="underline underline-offset-4 hover:text-fg">
            Clear filters
          </button>
        </p>
      )}
    </div>
  );
}
