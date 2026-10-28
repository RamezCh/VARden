import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { EnvVar, Scope } from "../lib/api";
import { rowId, STATUS_META, type SortKey, type SortState } from "../lib/filter";
import { Button, IconButton } from "./IconButton";
import { CheckIcon, ChevronRightIcon, CopyIcon, EyeIcon, EyeOffIcon, PencilIcon, SortIcon, TrashIcon } from "./icons";
import { SecretValue } from "./SecretValue";
import { StatusPill } from "./StatusStrip";
import { Help } from "./Help";
import { HELP } from "../lib/helpText";

// One grid for header and rows. Below `lg` each row stacks:
//   key ............ actions
//   value
//   scope   status
const ROW =
  "grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-1.5 px-4 py-3 lg:grid-cols-[minmax(160px,1.1fr)_minmax(0,2fr)_92px_164px_128px] lg:items-center lg:gap-y-0 lg:py-2";
const C = {
  key: "order-1 min-w-0 lg:order-none",
  value: "order-3 col-span-2 min-w-0 lg:order-none lg:col-span-1",
  scope: "order-4 lg:order-none",
  status: "order-5 lg:order-none",
  actions: "order-2 flex items-center justify-end gap-1 lg:order-none",
};

const SCOPE_LABEL: Record<Scope, string> = { user: "User", process: "Temporary" };
const SCOPE_HELP: Record<Scope, string> = {
  user: "Saved for your account. New terminals and programs get it, also after a restart.",
  process: "Never saved. Pushed into open terminals when you sync.",
};

export const input =
  "h-8 w-full min-w-0 rounded-md border border-rule bg-ink px-2.5 font-mono text-[0.9rem] text-fg placeholder:font-sans placeholder:text-faint focus:border-process focus:outline-none";

export interface TableProps {
  rows: EnvVar[];
  allVars: EnvVar[];
  sort: SortState;
  onSort: (key: SortKey) => void;
  isRevealed: (v: EnvVar) => boolean;
  onToggleReveal: (v: EnvVar) => void;
  busy: boolean;
  onDelete: (key: string, scope: Scope) => Promise<boolean>;
  /** Open the side panel for this variable. */
  onOpen: (v: EnvVar) => void;
  /** PATH (user scope) opens the PATH editor instead of inline editing. */
  onEditPath: () => void;
  caseInsensitive: boolean;
  selected: ReadonlySet<string>;
  onToggleSelect: (v: EnvVar) => void;
  /** Select or clear every row shown. */
  onToggleAll: () => void;
  empty: ReactNode;
}

const checkbox = "h-4 w-4 shrink-0 cursor-pointer accent-[var(--color-process)]";

function SelectAll({ rows, selected, onToggleAll }: Pick<TableProps, "rows" | "selected" | "onToggleAll">) {
  const ref = useRef<HTMLInputElement>(null);
  const picked = rows.filter((v) => selected.has(rowId(v))).length;
  const all = rows.length > 0 && picked === rows.length;
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = picked > 0 && !all;
  }, [picked, all]);
  return (
    <input
      ref={ref}
      type="checkbox"
      aria-label={all ? "Clear selection" : "Select all shown"}
      title={all ? "Clear selection" : "Select all shown"}
      className={checkbox}
      checked={all}
      disabled={rows.length === 0}
      onChange={onToggleAll}
    />
  );
}

export function VariableTable(props: TableProps) {
  const { rows, sort, onSort } = props;
  const [confirming, setConfirming] = useState<string | null>(null);

  const header = (label: string, key: SortKey | null, cls: string) => {
    const active = key !== null && sort.key === key;
    return (
      <div role="columnheader" className={cls} aria-sort={active ? (sort.dir === "asc" ? "ascending" : "descending") : undefined}>
        {key ? (
          <button type="button" onClick={() => onSort(key)} className="-mx-1 inline-flex items-center gap-1 rounded px-1 hover:text-fg">
            {label}
            <SortIcon dir={active ? sort.dir : null} />
          </button>
        ) : (
          label
        )}
      </div>
    );
  };

  return (
    <div role="table" aria-label="Environment variables" className="overflow-hidden rounded-lg border border-rule bg-panel">
      <div role="row" className={`${ROW} hidden border-b border-rule text-[0.8rem] text-muted lg:grid`}>
        <div className="flex items-center gap-3">
          <SelectAll rows={rows} selected={props.selected} onToggleAll={props.onToggleAll} />
          {header("Name", "key", "")}
        </div>
        {header("Value", null, "pl-[1.375rem]")}
        <div className="flex items-center gap-1.5">
          {header("Scope", "scope", "")}
          <Help label="What does scope mean?">{HELP.scope}</Help>
        </div>
        <div className="flex items-center gap-1.5">
          {header("Status", "status", "")}
          <Help label="What do the statuses mean?">{HELP.status}</Help>
        </div>
        <div role="columnheader" className="sr-only">
          Actions
        </div>
      </div>

      <div className="divide-y divide-rule">
        {rows.map((v) => {
          const id = rowId(v);
          if (confirming === id)
            return <ConfirmDeleteRow key={id} v={v} {...props} onDone={() => setConfirming(null)} />;
          return (
            <ViewRow
              key={id}
              v={v}
              {...props}
              onEdit={() => {
                setConfirming(null);
                if (isPathVar(v, props.caseInsensitive)) props.onEditPath();
                else props.onOpen(v);
              }}
              onAskDelete={() => setConfirming(id)}
            />
          );
        })}
        {rows.length === 0 && <div className="px-4 py-12 text-center">{props.empty}</div>}
      </div>
    </div>
  );
}

/** A list of folders (PATH-like), split into entries. */
function listEntries(v: EnvVar, sep: string): string[] | null {
  if (v.kind === "expression") return null;
  const value = v.expanded ?? v.value;
  if (!value.includes(sep)) return null;
  const parts = value.split(sep).map((p) => p.trim()).filter(Boolean);
  const listy = v.checkedPaths > 1 || /PATH$/i.test(v.key);
  return listy && parts.length > 0 ? parts : null;
}

/** Long values and lists can be opened to read them in full. */
const isLong = (v: EnvVar, sep: string) =>
  v.value.length > 48 || Boolean(v.expanded) || listEntries(v, sep) !== null || v.missingPaths.length > 0;

function CopyButton({ text, label = "Copy" }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <Button
      variant="quiet"
      className="h-7 px-2 text-xs"
      onClick={async () => {
        await navigator.clipboard.writeText(text);
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1400);
      }}
    >
      {copied ? <CheckIcon className="text-synced" /> : <CopyIcon />}
      {copied ? "Copied" : label}
    </Button>
  );
}

/** The full value, opened under the row: wrapped text, or one entry per line. */
function ValueDetails({ v, sep }: { v: EnvVar; sep: string }) {
  const entries = listEntries(v, sep);
  const missing = new Set(v.missingPaths);
  return (
    <div role="cell" className="order-last col-span-full mt-2 rounded-md border border-rule bg-ink/60 px-3 py-2.5 text-sm">
      {entries ? (
        <>
          <div className="mb-1.5 flex items-center justify-between gap-2">
            <span className="text-xs text-muted">
              {entries.length} entries, searched top to bottom
              {v.expanded ? ", as programs see them" : ""}
              {v.missingPaths.length > 0 && (
                <span className="text-missing"> · {v.missingPaths.length} don't exist</span>
              )}
            </span>
            <CopyButton text={v.value} label="Copy value" />
          </div>
          <ol className="selectable max-h-72 space-y-0.5 overflow-y-auto">
            {entries.map((e, i) => {
              const gone = missing.has(e);
              return (
                <li key={i} className="flex min-w-0 items-baseline gap-2.5">
                  <span className="w-6 shrink-0 text-right font-mono text-xs text-faint">{i + 1}</span>
                  <span className={`min-w-0 break-all font-mono text-[0.85rem] ${gone ? "text-missing" : ""}`}>{e}</span>
                  {gone && <span className="shrink-0 text-xs text-missing">doesn't exist</span>}
                </li>
              );
            })}
          </ol>
        </>
      ) : (
        <>
          <div className="flex items-start justify-between gap-2">
            <p className="selectable min-w-0 whitespace-pre-wrap break-all font-mono text-[0.85rem]">{v.value}</p>
            <CopyButton text={v.value} />
          </div>
          {v.expanded && (
            <p className="selectable mt-2 break-all text-xs text-muted">
              Expands to <span className="font-mono text-fg">{v.expanded}</span>
            </p>
          )}
          {v.missingPaths.length > 0 && (
            <p className="mt-2 text-xs text-missing">
              Doesn't exist: <span className="selectable break-all font-mono">{v.missingPaths.join(", ")}</span>
            </p>
          )}
        </>
      )}
      {v.status === "persistentOnly" && v.liveValue !== null && (
        <p className="selectable mt-2 break-all border-t border-rule pt-2 text-xs text-muted">
          Open terminals still have <span className="font-mono text-fg">{v.liveValue || "an empty value"}</span>
        </p>
      )}
    </div>
  );
}

export const isPathVar = (v: Pick<EnvVar, "key" | "scope">, caseInsensitive: boolean) =>
  v.scope === "user" && (caseInsensitive ? v.key.toUpperCase() : v.key) === "PATH";

function ScopeCell({ scope }: { scope: Scope }) {
  return (
    <span className="text-sm text-muted" title={SCOPE_HELP[scope]}>
      {SCOPE_LABEL[scope]}
    </span>
  );
}

function ViewRow({
  v,
  isRevealed,
  onToggleReveal,
  onEdit,
  onAskDelete,
  selected,
  onToggleSelect,
  caseInsensitive,
}: TableProps & { v: EnvVar; onEdit: () => void; onAskDelete: () => void }) {
  const revealed = isRevealed(v);
  const masked = v.sensitive && !revealed;
  const picked = selected.has(rowId(v));
  const isPath = isPathVar(v, caseInsensitive);
  const sep = caseInsensitive ? ";" : ":";
  const [open, setOpen] = useState(false);
  const expandable = !masked && v.value !== "" && isLong(v, sep);
  const entries = listEntries(v, sep);

  // One short line under the value; the full story is in the opened row.
  let note: ReactNode = null;
  if (v.missingPaths.length > 0) {
    note = masked
      ? "Points to a path that doesn't exist"
      : v.checkedPaths > 1
        ? `${v.missingPaths.length} of ${v.checkedPaths} entries don't exist`
        : `Doesn't exist: ${v.missingPaths[0]}`;
    note = <span className="text-missing">{note}</span>;
  } else if (v.status === "persistentOnly" && v.liveValue !== null) {
    note = "Open terminals still have an older value";
  } else if (v.expanded && !masked) {
    note = (
      <>
        Expands to <span className="font-mono">{v.expanded}</span>
      </>
    );
  } else if (entries) {
    note = `${entries.length} entries`;
  }

  return (
    <div
      role="row"
      aria-selected={picked}
      className={`group/row ${ROW} ${picked ? "bg-process/[0.07]" : "hover:bg-raised/40"}`}
      onDoubleClick={(e) => {
        if (!(e.target as HTMLElement).closest("button, input, a, .selectable")) onEdit();
      }}
    >
      <div role="cell" className={`${C.key} flex items-center gap-3`}>
        <input
          type="checkbox"
          aria-label={`Select ${v.key}`}
          className={checkbox}
          checked={picked}
          onChange={() => onToggleSelect(v)}
        />
        <span
          className="h-2 w-2 shrink-0 rounded-full"
          style={{ background: STATUS_META[v.status].color }}
          title={STATUS_META[v.status].label}
          aria-hidden
        />
        <span className="truncate font-mono text-[0.9rem] font-medium" title={v.key}>
          {v.key}
        </span>
      </div>
      <div role="cell" className={`${C.value} flex min-w-0 items-start gap-1`}>
        {expandable ? (
          <IconButton
            label={open ? "Hide full value" : "Show full value"}
            aria-expanded={open}
            onClick={() => setOpen(!open)}
            className="-ml-1.5 mt-px h-6 w-6"
          >
            <ChevronRightIcon className={`transition-transform ${open ? "rotate-90" : ""}`} />
          </IconButton>
        ) : (
          <span className="-ml-1.5 w-6 shrink-0" aria-hidden />
        )}
        <div className="min-w-0 flex-1">
          <SecretValue value={v.value} sensitive={v.sensitive} revealed={revealed} onToggle={() => onToggleReveal(v)} />
          {note && !open && (
            <button
              type="button"
              disabled={!expandable}
              onClick={() => setOpen(true)}
              className="block max-w-full truncate text-left text-xs text-muted enabled:hover:text-fg"
            >
              {note}
            </button>
          )}
        </div>
      </div>
      <div role="cell" className={C.scope}>
        <ScopeCell scope={v.scope} />
      </div>
      <div role="cell" className={C.status}>
        <StatusPill status={v.status} />
      </div>
      <div
        role="cell"
        className={`${C.actions} transition-opacity lg:opacity-0 lg:focus-within:opacity-100 lg:group-hover/row:opacity-100`}
      >
        <IconButton label={isPath ? "Edit PATH in the PATH editor" : `Edit ${v.key}`} onClick={onEdit}>
          <PencilIcon />
        </IconButton>
        <IconButton label={`Delete ${v.key}`} onClick={onAskDelete} className="hover:text-missing">
          <TrashIcon />
        </IconButton>
      </div>
      {open && expandable && <ValueDetails v={v} sep={sep} />}
    </div>
  );
}

/** Value input with its own show/hide toggle, masked by default for secrets. */
export function ValueInput({
  value,
  onChange,
  sensitive,
  onKeyDown,
  autoFocus,
  label,
  placeholder = "Value",
}: {
  value: string;
  onChange: (v: string) => void;
  sensitive: boolean;
  onKeyDown: (e: KeyboardEvent) => void;
  autoFocus?: boolean;
  label: string;
  placeholder?: string;
}) {
  const [shown, setShown] = useState(!sensitive);
  useEffect(() => setShown(!sensitive), [sensitive]);
  return (
    <div className="flex items-center gap-1">
      <input
        aria-label={label}
        className={input}
        type={shown ? "text" : "password"}
        value={value}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        autoFocus={autoFocus}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={onKeyDown}
      />
      {sensitive && (
        <IconButton label={shown ? "Hide value" : "Show value"} onClick={() => setShown(!shown)}>
          {shown ? <EyeOffIcon /> : <EyeIcon />}
        </IconButton>
      )}
    </div>
  );
}

function ConfirmDeleteRow({ v, busy, onDelete, onDone }: TableProps & { v: EnvVar; onDone: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => ref.current?.querySelector<HTMLButtonElement>("[data-cancel]")?.focus(), []);
  const remove = async () => {
    if (await onDelete(v.key, v.scope)) onDone();
  };
  return (
    <div role="row" ref={ref} className={`${ROW} bg-missing/[0.07]`} onKeyDown={(e) => e.key === "Escape" && onDone()}>
      <div role="cell" className={`${C.key} truncate font-mono text-[0.9rem] font-medium`}>
        {v.key}
      </div>
      <div role="cell" className={`${C.value} text-sm lg:col-span-3`}>
        {v.scope === "user"
          ? "Delete from your user environment? A backup is saved first, so you can restore it."
          : "Remove this temporary variable? Open terminals drop it at the next sync."}
      </div>
      <div role="cell" className={C.actions}>
        <Button variant="quiet" data-cancel onClick={onDone}>
          Cancel
        </Button>
        <Button variant="danger" disabled={busy} onClick={() => void remove()}>
          Delete
        </Button>
      </div>
    </div>
  );
}
