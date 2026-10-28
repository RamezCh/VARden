import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { EnvVar, Scope } from "../lib/api";
import { keyError, looksSensitive, rowId, type SortKey, type SortState } from "../lib/filter";
import { Button, IconButton } from "./IconButton";
import { EyeIcon, EyeOffIcon, PencilIcon, SortIcon, TrashIcon } from "./icons";
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
  creating: boolean;
  onCloseCreate: () => void;
  busy: boolean;
  onSet: (key: string, value: string, scope: Scope, existed: boolean) => Promise<boolean>;
  onDelete: (key: string, scope: Scope) => Promise<boolean>;
  empty: ReactNode;
}

export function VariableTable(props: TableProps) {
  const { rows, sort, onSort, creating } = props;
  const [editing, setEditing] = useState<string | null>(null);
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
        {header("Name", "key", "")}
        {header("Value", null, "")}
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
        {creating && <NewVariableRow {...props} />}
        {rows.map((v) => {
          const id = rowId(v);
          if (editing === id) return <EditRow key={id} v={v} {...props} onDone={() => setEditing(null)} />;
          if (confirming === id)
            return <ConfirmDeleteRow key={id} v={v} {...props} onDone={() => setConfirming(null)} />;
          return (
            <ViewRow
              key={id}
              v={v}
              {...props}
              onEdit={() => {
                setConfirming(null);
                setEditing(id);
              }}
              onAskDelete={() => {
                setEditing(null);
                setConfirming(id);
              }}
            />
          );
        })}
        {rows.length === 0 && !creating && <div className="px-4 py-12 text-center">{props.empty}</div>}
      </div>
    </div>
  );
}

/** Paths in the value that don't exist (what makes the row 🔴). */
function PathProblems({ v, hidden }: { v: EnvVar; hidden: boolean }) {
  if (hidden) return <p className="mt-0.5 text-xs text-missing">Points to a path that doesn't exist.</p>;
  if (v.checkedPaths <= 1) {
    return (
      <p className="mt-0.5 truncate text-xs text-missing" title={v.missingPaths[0]}>
        Doesn't exist: <span className="font-mono">{v.missingPaths[0]}</span>
      </p>
    );
  }
  return (
    <details className="mt-0.5 text-xs">
      <summary className="cursor-pointer text-missing">
        {v.missingPaths.length} of {v.checkedPaths} entries don't exist
      </summary>
      <ul className="mt-1 space-y-0.5 border-l border-missing/40 pl-2">
        {v.missingPaths.map((p) => (
          <li key={p} className="truncate font-mono text-muted" title={p}>
            {p}
          </li>
        ))}
      </ul>
    </details>
  );
}

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
}: TableProps & { v: EnvVar; onEdit: () => void; onAskDelete: () => void }) {
  const revealed = isRevealed(v);
  return (
    <div role="row" className={`${ROW} hover:bg-raised/40`}>
      <div role="cell" className={`${C.key} truncate font-mono text-[0.9rem] font-medium`} title={v.key}>
        {v.key}
      </div>
      <div role="cell" className={C.value}>
        <SecretValue value={v.value} sensitive={v.sensitive} revealed={revealed} onToggle={() => onToggleReveal(v)} />
        {v.status === "persistentOnly" && v.liveValue !== null && (
          <p className="mt-0.5 truncate text-xs text-muted">
            {v.sensitive && !revealed ? (
              "Running environment still has an older value"
            ) : (
              <>
                Running environment still has <span className="font-mono">{v.liveValue || "an empty value"}</span>
              </>
            )}
          </p>
        )}
        {v.expanded && !(v.sensitive && !revealed) && (
          <p className="mt-0.5 truncate text-xs text-muted" title={v.expanded}>
            Expands to <span className="font-mono">{v.expanded}</span>
          </p>
        )}
        {v.missingPaths.length > 0 && <PathProblems v={v} hidden={v.sensitive && !revealed} />}
      </div>
      <div role="cell" className={C.scope}>
        <ScopeCell scope={v.scope} />
      </div>
      <div role="cell" className={C.status}>
        <StatusPill status={v.status} />
      </div>
      <div role="cell" className={C.actions}>
        <IconButton label={`Edit ${v.key}`} onClick={onEdit}>
          <PencilIcon />
        </IconButton>
        <IconButton label={`Delete ${v.key}`} onClick={onAskDelete} className="hover:text-missing">
          <TrashIcon />
        </IconButton>
      </div>
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

function ScopeSelect({ value, onChange, disabled }: { value: Scope; onChange: (s: Scope) => void; disabled?: boolean }) {
  return (
    <select
      aria-label="Scope"
      value={value}
      disabled={disabled}
      title={disabled ? "To make a saved variable temporary, delete it and add it again as Temporary." : SCOPE_HELP[value]}
      onChange={(e) => onChange(e.target.value as Scope)}
      className="h-8 w-full rounded-md border border-rule bg-ink px-1.5 text-sm text-fg focus:border-process focus:outline-none disabled:opacity-60"
    >
      <option value="user">User</option>
      <option value="process">Temporary</option>
    </select>
  );
}

const enterEscape = (save: () => void, cancel: () => void) => (e: KeyboardEvent) => {
  if (e.key === "Enter") {
    e.preventDefault();
    save();
  } else if (e.key === "Escape") {
    e.preventDefault();
    cancel();
  }
};

function EditRow({ v, busy, onSet, onDone }: TableProps & { v: EnvVar; onDone: () => void }) {
  const [value, setValue] = useState(v.value);
  // A temporary variable can be promoted to user scope; the reverse would
  // silently delete saved data, so it isn't offered here.
  const [scope, setScope] = useState<Scope>(v.scope);
  const save = async () => {
    if (await onSet(v.key, value, scope, scope === v.scope)) onDone();
  };
  const keys = enterEscape(() => void save(), onDone);
  return (
    <div role="row" className={`${ROW} bg-raised/50`}>
      <div role="cell" className={`${C.key} truncate font-mono text-[0.9rem] font-medium`}>
        {v.key}
      </div>
      <div role="cell" className={C.value}>
        <ValueInput label={`Value of ${v.key}`} value={value} onChange={setValue} sensitive={v.sensitive} onKeyDown={keys} autoFocus />
        {scope !== v.scope && <p className="mt-1 text-xs text-muted">Saving will keep {v.key} after the app closes.</p>}
      </div>
      <div role="cell" className={C.scope}>
        <ScopeSelect value={scope} onChange={setScope} disabled={v.scope === "user"} />
      </div>
      <div role="cell" className={C.status}>
        <StatusPill status={v.status} />
      </div>
      <div role="cell" className={C.actions}>
        <Button variant="quiet" onClick={onDone}>
          Cancel
        </Button>
        <Button variant="primary" disabled={busy} onClick={() => void save()}>
          Save
        </Button>
      </div>
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

function NewVariableRow({ allVars, busy, onSet, onCloseCreate }: TableProps) {
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const [scope, setScope] = useState<Scope>("user");
  const [touched, setTouched] = useState(false);

  const trimmed = key.trim();
  const error = keyError(trimmed);
  const existing = allVars.find((v) => v.scope === scope && v.key.toLowerCase() === trimmed.toLowerCase());

  const save = async () => {
    setTouched(true);
    if (error) return;
    if (await onSet(trimmed, value, scope, Boolean(existing))) onCloseCreate();
  };
  const keys = enterEscape(() => void save(), onCloseCreate);

  return (
    <div role="row" className={`${ROW} bg-raised/50 lg:items-start lg:py-3`}>
      <div role="cell" className={`${C.key} col-span-2 lg:col-span-1`}>
        <input
          aria-label="Name"
          aria-invalid={touched && Boolean(error)}
          className={input}
          placeholder="NAME"
          value={key}
          spellCheck={false}
          autoComplete="off"
          autoFocus
          onChange={(e) => setKey(e.target.value)}
          onBlur={() => setTouched(true)}
          onKeyDown={keys}
        />
        {touched && error ? (
          <p className="mt-1 text-xs text-missing">{error}</p>
        ) : existing ? (
          <p className="mt-1 text-xs text-persistent">{existing.key} exists. Saving replaces its value.</p>
        ) : null}
      </div>
      <div role="cell" className={C.value}>
        <ValueInput label="Value" value={value} onChange={setValue} sensitive={looksSensitive(trimmed)} onKeyDown={keys} />
      </div>
      <div role="cell" className={C.scope}>
        <ScopeSelect value={scope} onChange={setScope} />
      </div>
      <div role="cell" className={`${C.status} self-center text-xs text-muted`}>
        {SCOPE_HELP[scope]}
      </div>
      <div role="cell" className={`${C.actions} order-6 col-span-2 lg:col-span-1`}>
        <Button variant="quiet" onClick={onCloseCreate}>
          Cancel
        </Button>
        <Button variant="primary" disabled={busy} onClick={() => void save()}>
          Add
        </Button>
      </div>
    </div>
  );
}
