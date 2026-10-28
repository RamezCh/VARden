import { useEffect, useRef, useState, type RefObject } from "react";
import type { ExportItem } from "../lib/api";
import type { ActiveProfile, EnvVar, Scope, Status } from "../lib/api";
import { countByStatus, filterVars, rowId, sortVars, type ScopeFilter, type SortKey, type SortState } from "../lib/filter";
import { Button } from "./IconButton";
import { ExportIcon, EyeIcon, EyeOffIcon, ImportIcon, PathIcon, PlusIcon, SearchIcon, SyncIcon, TrashIcon } from "./icons";
import { Menu } from "./Menu";
import { PageHeader } from "./PageHeader";
import { StatusStrip } from "./StatusStrip";
import { Help } from "./Help";
import { HELP } from "../lib/helpText";
import { VariableTable } from "./VariableTable";
import { VariablePanel, type PanelState } from "./VariablePanel";

interface Props {
  vars: EnvVar[];
  loading: boolean;
  busy: boolean;
  creating: boolean;
  setCreating: (v: boolean) => void;
  searchRef: RefObject<HTMLInputElement | null>;
  onSet: (key: string, value: string, scope: Scope, existed: boolean) => Promise<boolean>;
  onDelete: (key: string, scope: Scope) => Promise<boolean>;
  onDeleteMany: (items: ExportItem[]) => Promise<boolean>;
  onRename: (from: string, to: string, scope: Scope) => Promise<boolean>;
  onEditPath: () => void;
  caseInsensitive: boolean;
  onOpenImport: () => void;
  onOpenExport: (visible: EnvVar[]) => void;
  /** Number of shells with the terminal-sync hook installed. */
  hooksInstalled: number;
  onSync: () => void;
  onSetUpSync: () => void;
  activeProfile: ActiveProfile | null;
  onOpenProfiles: () => void;
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
  onDeleteMany,
  onRename,
  onEditPath,
  caseInsensitive,
  onOpenImport,
  onOpenExport,
  hooksInstalled,
  onSync,
  onSetUpSync,
  activeProfile,
  onOpenProfiles,
}: Props) {
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<ScopeFilter>("all");
  const [statuses, setStatuses] = useState<ReadonlySet<Status>>(new Set());
  const [sort, setSort] = useState<SortState>({ key: "key", dir: "asc" });
  const [revealAll, setRevealAll] = useState(false);
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(new Set());
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  // The side panel: adding (from "New variable") or editing one row.
  const [openVar, setOpenVar] = useState<EnvVar | null>(null);
  const panel: PanelState | null = creating ? { kind: "new" } : openVar ? { kind: "edit", v: openVar } : null;
  const closePanel = () => {
    setCreating(false);
    setOpenVar(null);
  };
  const [confirmBulk, setConfirmBulk] = useState(false);

  const isRevealed = (v: EnvVar) => revealAll || revealed.has(rowId(v));
  const isVisible = (v: EnvVar) => !v.sensitive || isRevealed(v);

  // Counts follow the scope filter, so the strip always describes what the
  // status buttons would show.
  const counts = countByStatus(vars.filter((v) => scope === "all" || v.scope === scope));
  const pending = vars.filter((v) => v.pendingSync).length;
  const rows = sortVars(filterVars(vars, { query, scope, statuses }, isVisible), sort);
  const filtered = query.trim() !== "" || scope !== "all" || statuses.size > 0;
  // Rows that disappear (deleted, renamed) drop out of the selection.
  const picked = vars.filter((v) => selected.has(rowId(v)));
  const pickedSaved = picked.filter((v) => v.scope === "user").length;

  const toggleSelect = (v: EnvVar) =>
    setSelected((prev) => {
      const next = new Set(prev);
      const id = rowId(v);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const toggleAll = () =>
    setSelected((prev) => {
      const shown = rows.map(rowId);
      const all = shown.length > 0 && shown.every((id) => prev.has(id));
      const next = new Set(prev);
      shown.forEach((id) => (all ? next.delete(id) : next.add(id)));
      return next;
    });
  const clearSelection = () => {
    setSelected(new Set());
    setConfirmBulk(false);
  };
  const deletePicked = async () => {
    if (await onDeleteMany(picked.map(({ key, scope }) => ({ key, scope })))) clearSelection();
  };

  // Ctrl+A selects what's shown, Del asks to delete the selection, Esc clears it.
  const keys = useRef({ rows, picked: picked.length });
  keys.current = { rows, picked: picked.length };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (document.querySelector("dialog[open]") || t?.closest("input, textarea, select, [role=menu]")) return;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
        e.preventDefault();
        setSelected(new Set(keys.current.rows.map(rowId)));
      } else if (e.key === "Delete" && keys.current.picked > 0) {
        e.preventDefault();
        setConfirmBulk(true);
      } else if (e.key === "Escape" && keys.current.picked > 0) {
        setSelected(new Set());
        setConfirmBulk(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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

  const syncButton =
    hooksInstalled > 0 ? (
      <Button
        variant="secondary"
        onClick={onSync}
        disabled={busy}
        title="Push saved and temporary variables into open terminals that have terminal sync"
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
    ) : (
      <Button variant="secondary" onClick={onSetUpSync} title="Install the terminal hook for your shells">
        <SyncIcon />
        Set up terminal sync
      </Button>
    );

  return (
    <div className="flex flex-col gap-5">
      <PageHeader
        title="Variables"
        description="Your saved user variables, plus temporary ones for open terminals."
        actions={
          <>
            <span className="flex items-center gap-1.5">
              {syncButton}
              <Help label={hooksInstalled > 0 ? "What does Sync to terminals do?" : "What is terminal sync?"}>{HELP.sync}</Help>
            </span>
            <Button variant="primary" onClick={() => setCreating(true)} disabled={creating} title="New variable (Ctrl+N)">
              <PlusIcon /> New variable
            </Button>
          </>
        }
      />

      {activeProfile && (
        <p className="-mt-1 flex flex-wrap items-center gap-2 text-sm text-muted">
          <span className="h-2 w-2 rounded-full bg-process" />
          <span>
            Profile <strong className="text-fg">{activeProfile.name}</strong> is active
            {activeProfile.scope === "process" ? " as temporary variables" : ""}.
          </span>
          <button type="button" className="underline underline-offset-4 hover:text-fg" onClick={onOpenProfiles}>
            Manage profiles
          </button>
        </p>
      )}

      <div className="flex flex-col gap-2.5">
        <div className="flex items-center gap-2">
          <label className="relative flex min-w-0 flex-1 items-center">
            <SearchIcon className="pointer-events-none absolute left-2.5 text-faint" />
            <span className="sr-only">Search variables</span>
            <input
              ref={searchRef}
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && setQuery("")}
              placeholder="Search names and visible values"
              className="h-9 w-full rounded-md border border-rule bg-panel pl-8 pr-16 text-sm text-fg placeholder:text-faint focus:border-process focus:outline-none"
            />
            <kbd className="pointer-events-none absolute right-2.5 rounded border border-rule px-1.5 font-sans text-[0.7rem] text-faint">
              Ctrl F
            </kbd>
          </label>
          <Button
            variant="secondary"
            className="h-9"
            aria-pressed={revealAll}
            onClick={() => {
              setRevealAll(!revealAll);
              setRevealed(new Set());
            }}
          >
            {revealAll ? <EyeOffIcon /> : <EyeIcon />}
            {revealAll ? "Hide secrets" : "Show secrets"}
          </Button>
          {/* Tools that work on the list sit with the list, not in the header. */}
          <Button variant="secondary" className="h-9" onClick={onEditPath} title="Add, remove or reorder the folders in your PATH">
            <PathIcon /> Edit PATH
          </Button>
          <Menu
            label=".env"
            className="h-9"
            items={[
              { label: "Import from .env…", icon: <ImportIcon />, onSelect: onOpenImport },
              {
                label: "Export to .env…",
                icon: <ExportIcon />,
                disabled: vars.length === 0,
                onSelect: () => onOpenExport(rows),
              },
            ]}
          />
        </div>

        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <div role="radiogroup" aria-label="Scope" className="flex h-8 items-center rounded-md border border-rule bg-panel p-0.5">
            {SCOPES.map((s) => (
              <button
                key={s.value}
                type="button"
                role="radio"
                aria-checked={scope === s.value}
                onClick={() => setScope(s.value)}
                className={`h-full rounded px-3 text-sm transition-colors ${
                  scope === s.value ? "bg-raised font-semibold text-fg" : "text-muted hover:text-fg"
                }`}
              >
                {s.label}
              </button>
            ))}
          </div>
          <span className="h-5 w-px bg-rule" aria-hidden />
          <StatusStrip counts={counts} active={statuses} onToggle={toggleStatus} onClear={() => setStatuses(new Set())} />
          <span className="ml-auto text-sm text-muted">
            {filtered ? (
              <>
                {rows.length} of {vars.length}{" "}
                <button type="button" onClick={clearFilters} className="underline underline-offset-4 hover:text-fg">
                  Clear filters
                </button>
              </>
            ) : (
              `${vars.length} variable${vars.length === 1 ? "" : "s"}`
            )}
          </span>
        </div>
      </div>

      {picked.length > 0 && (
        <div
          role="region"
          aria-label="Selected variables"
          className={`-mb-2 flex flex-wrap items-center gap-2 rounded-lg border px-3 py-1.5 text-sm ${
            confirmBulk ? "border-missing/40 bg-missing/[0.07]" : "border-process/40 bg-process/[0.07]"
          }`}
          onKeyDown={(e) => e.key === "Escape" && setConfirmBulk(false)}
        >
          {confirmBulk ? (
            <>
              <span className="mr-auto">
                Delete {picked.length === 1 ? picked[0].key : `${picked.length} variables`}?{" "}
                <span className="text-muted">
                  {pickedSaved > 0
                    ? "A backup is saved first, and Undo brings them back."
                    : "Open terminals drop them at the next sync."}
                </span>
              </span>
              <Button onClick={() => setConfirmBulk(false)}>Cancel</Button>
              <Button variant="danger" disabled={busy} onClick={() => void deletePicked()}>
                Delete {picked.length}
              </Button>
            </>
          ) : (
            <>
              <span className="mr-auto font-semibold">{picked.length} selected</span>
              <Button onClick={() => onOpenExport(picked)}>
                <ExportIcon /> Export…
              </Button>
              <Button className="hover:text-missing" onClick={() => setConfirmBulk(true)} title="Delete (Del)">
                <TrashIcon /> Delete…
              </Button>
              <Button onClick={clearSelection} title="Clear selection (Esc)">
                Clear
              </Button>
            </>
          )}
        </div>
      )}

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
        busy={busy}
        onDelete={onDelete}
        onOpen={setOpenVar}
        onEditPath={onEditPath}
        caseInsensitive={caseInsensitive}
        selected={selected}
        onToggleSelect={toggleSelect}
        onToggleAll={toggleAll}
        empty={empty}
      />

      {panel && (
        <VariablePanel
          key={panel.kind === "edit" ? rowId(panel.v) : "new"}
          state={panel}
          allVars={vars}
          caseInsensitive={caseInsensitive}
          busy={busy}
          onClose={closePanel}
          onSet={onSet}
          onRename={onRename}
          onDelete={onDelete}
        />
      )}
    </div>
  );
}
