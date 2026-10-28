import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { EnvVar, Scope } from "../lib/api";
import { keyError, looksSensitive, STATUS_META } from "../lib/filter";
import { Button, IconButton } from "./IconButton";
import { EyeIcon, EyeOffIcon, XIcon } from "./icons";
import { StatusPill } from "./StatusStrip";

export type PanelState = { kind: "new" } | { kind: "edit"; v: EnvVar };

interface Props {
  state: PanelState;
  allVars: EnvVar[];
  caseInsensitive: boolean;
  busy: boolean;
  onClose: () => void;
  onSet: (key: string, value: string, scope: Scope, existed: boolean) => Promise<boolean>;
  onRename: (from: string, to: string, scope: Scope) => Promise<boolean>;
  onDelete: (key: string, scope: Scope) => Promise<boolean>;
}

const field =
  "w-full min-w-0 rounded-md border border-rule bg-ink px-3 font-mono text-[0.9rem] text-fg placeholder:font-sans placeholder:text-faint focus:border-process focus:outline-none disabled:opacity-60";

const SCOPES: { value: Scope; title: string; body: string }[] = [
  {
    value: "user",
    title: "User",
    body: "Saved for your account. New terminals and programs get it, also after a restart.",
  },
  {
    value: "process",
    title: "Temporary",
    body: "Never saved. Pushed into open terminals when you sync, gone when they close.",
  },
];

/**
 * Adding and editing happen in a panel on the right: the list stays where
 * it is on the left, and the form reads top to bottom (name, value, scope,
 * then the buttons at the bottom right).
 */
export function VariablePanel({ state, allVars, caseInsensitive, busy, onClose, onSet, onRename, onDelete }: Props) {
  const editing = state.kind === "edit" ? state.v : null;
  const ref = useRef<HTMLDialogElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const valueRef = useRef<HTMLTextAreaElement>(null);

  const [key, setKey] = useState(editing?.key ?? "");
  const [value, setValue] = useState(editing?.value ?? "");
  const [scope, setScope] = useState<Scope>(editing?.scope ?? "user");
  const [touched, setTouched] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const trimmed = key.trim();
  const norm = (k: string) => (caseInsensitive ? k.toUpperCase() : k);
  const sensitive = editing ? editing.sensitive || looksSensitive(trimmed) : looksSensitive(trimmed);
  const [shown, setShown] = useState(!sensitive);
  useEffect(() => setShown(!sensitive), [sensitive]);

  const renamed = editing !== null && trimmed !== editing.key;
  const nameProblem =
    editing && !renamed
      ? null
      : (keyError(trimmed) ??
        (editing && caseInsensitive && norm(trimmed) === norm(editing.key)
          ? "Windows ignores upper/lower case in names, so this wouldn't change anything."
          : editing && allVars.some((x) => x.scope === editing.scope && x !== editing && norm(x.key) === norm(trimmed))
            ? `${trimmed} already exists.`
            : null));
  // Adding a name that exists replaces its value: say so, but allow it.
  const existing = !editing ? allVars.find((v) => v.scope === scope && norm(v.key) === norm(trimmed)) : undefined;
  const expression = editing?.kind === "expression";

  useEffect(() => {
    const d = ref.current;
    if (!d || d.open) return;
    d.showModal();
    (editing ? valueRef.current : nameRef.current)?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const save = async () => {
    setTouched(true);
    if (nameProblem) {
      nameRef.current?.focus();
      return;
    }
    if (!editing) {
      if (await onSet(trimmed, value, scope, Boolean(existing))) onClose();
      return;
    }
    if (renamed && !(await onRename(editing.key, trimmed, editing.scope))) return;
    if (value !== editing.value || scope !== editing.scope) {
      if (!(await onSet(trimmed, value, scope, scope === editing.scope))) return;
    }
    onClose();
  };

  // Enter saves from any single field; values never contain line breaks.
  const onEnter = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void save();
    }
  };

  const changed = !editing || renamed || value !== editing.value || scope !== editing.scope;

  return (
    <dialog
      ref={ref}
      aria-labelledby="panel-title"
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onMouseDown={(e) => e.target === ref.current && onClose()}
      className="panel-in fixed inset-y-0 left-auto right-0 m-0 h-dvh max-h-none w-[min(460px,100vw)] border-0 border-l border-rule bg-panel p-0 text-fg shadow-2xl shadow-[var(--shadow)] backdrop:bg-[var(--backdrop)]"
    >
      <div className="flex h-full flex-col">
        <header className="flex items-start gap-3 border-b border-rule px-6 py-5">
          <div className="min-w-0 flex-1">
            <h2 id="panel-title" className="truncate text-lg font-semibold">
              {editing ? `Edit ${editing.key}` : "New variable"}
            </h2>
            {editing ? (
              <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted">
                <StatusPill status={editing.status} />
                <span>{STATUS_META[editing.status].help}</span>
              </div>
            ) : (
              <p className="mt-0.5 text-sm text-muted">Programs read it by its name. A backup is taken before saving.</p>
            )}
          </div>
          <IconButton label="Close" onClick={onClose}>
            <XIcon />
          </IconButton>
        </header>

        <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-6 py-5">
          <Field label="Name" htmlFor="panel-name" hint={editing ? "Change it to rename the variable; the value moves with it." : "Letters, digits and underscores, for example API_URL."}>
            <input
              id="panel-name"
              ref={nameRef}
              className={`${field} h-9`}
              value={key}
              placeholder="NAME"
              spellCheck={false}
              autoComplete="off"
              disabled={expression}
              title={expression ? "This is a shell expression in your startup file. Rename it there by hand." : undefined}
              aria-invalid={touched && Boolean(nameProblem)}
              onChange={(e) => setKey(e.target.value)}
              onBlur={() => setTouched(true)}
              onKeyDown={onEnter}
            />
            {touched && nameProblem ? (
              <Note tone="missing">{nameProblem}</Note>
            ) : existing ? (
              <Note tone="persistent">{existing.key} already exists. Saving replaces its value.</Note>
            ) : renamed && !nameProblem ? (
              <Note>Renames {editing?.key} to {trimmed}.</Note>
            ) : null}
          </Field>

          <Field
            label="Value"
            htmlFor="panel-value"
            action={
              sensitive && (
                <button
                  type="button"
                  className="inline-flex items-center gap-1 text-xs text-muted hover:text-fg"
                  onClick={() => setShown(!shown)}
                >
                  {shown ? <EyeOffIcon /> : <EyeIcon />}
                  {shown ? "Hide" : "Show"}
                </button>
              )
            }
          >
            {/* One element whether shown or hidden: swapping a textarea for a
                password input moved keyboard focus out of the panel. Hidden
                values are masked with text-security instead. */}
            <textarea
              id="panel-value"
              ref={valueRef}
              className={`${field} min-h-[5.5rem] resize-y py-2 leading-relaxed [overflow-wrap:anywhere] ${
                shown ? "" : "[-webkit-text-security:disc]"
              }`}
              rows={shown ? Math.min(10, Math.max(3, Math.ceil(value.length / 44))) : 3}
              value={value}
              placeholder="Value"
              spellCheck={false}
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              onChange={(e) => setValue(e.target.value.replace(/\r?\n/g, ""))}
              onKeyDown={onEnter}
            />
            {editing?.expanded && shown && value === editing.value && (
              <Note>
                Expands to <span className="font-mono text-fg [overflow-wrap:anywhere]">{editing.expanded}</span>
              </Note>
            )}
            {editing && editing.missingPaths.length > 0 && value === editing.value && (
              <Note tone="missing">
                Doesn't exist:{" "}
                <span className="font-mono [overflow-wrap:anywhere]">{shown ? editing.missingPaths.join(", ") : "a path in this value"}</span>
              </Note>
            )}
            {sensitive && <Note>Looks like a secret, so it's hidden on screen. It's still stored as plain text, like any variable.</Note>}
          </Field>

          <fieldset>
            <legend className="mb-2 text-sm font-semibold">Scope</legend>
            <div className="flex flex-col gap-2">
              {SCOPES.map((s) => {
                // A saved variable can't become temporary here: that would
                // quietly delete saved data.
                const locked = editing?.scope === "user" && s.value === "process";
                return (
                  <label
                    key={s.value}
                    className={`flex gap-3 rounded-lg border px-3.5 py-3 transition-colors ${
                      scope === s.value ? "border-process bg-process/[0.07]" : "border-rule hover:bg-raised/50"
                    } ${locked ? "opacity-50" : "cursor-pointer"}`}
                    title={locked ? "To make a saved variable temporary, delete it and add it again as Temporary." : undefined}
                  >
                    <input
                      type="radio"
                      name="panel-scope"
                      className="mt-1 h-4 w-4 accent-[var(--color-process)]"
                      checked={scope === s.value}
                      disabled={locked}
                      onChange={() => setScope(s.value)}
                    />
                    <span className="min-w-0">
                      <span className="block text-sm font-semibold">{s.title}</span>
                      <span className="block text-sm text-muted">{s.body}</span>
                    </span>
                  </label>
                );
              })}
            </div>
            {editing && editing.scope === "process" && scope === "user" && (
              <Note>Saving keeps {trimmed || editing.key} after VARden closes.</Note>
            )}
          </fieldset>
        </div>

        <footer className="flex flex-wrap items-center gap-2 border-t border-rule px-6 py-4">
          {editing && confirmDelete ? (
            <>
              <span className="mr-auto text-sm">
                Delete {editing.key}?{" "}
                <span className="text-muted">{editing.scope === "user" ? "A backup is saved first." : ""}</span>
              </span>
              <Button onClick={() => setConfirmDelete(false)}>Cancel</Button>
              <Button
                variant="danger"
                disabled={busy}
                onClick={async () => {
                  if (await onDelete(editing.key, editing.scope)) onClose();
                }}
              >
                Delete
              </Button>
            </>
          ) : (
            <>
              {editing && (
                <Button className="mr-auto hover:text-missing" onClick={() => setConfirmDelete(true)}>
                  Delete…
                </Button>
              )}
              <Button className={editing ? "" : "ml-auto"} onClick={onClose}>
                Cancel
              </Button>
              <Button variant="primary" disabled={busy || !changed} onClick={() => void save()} title="Save (Enter)">
                {editing ? "Save" : "Add variable"}
              </Button>
            </>
          )}
        </footer>
      </div>
    </dialog>
  );
}

function Field({
  label,
  htmlFor,
  hint,
  action,
  children,
}: {
  label: string;
  htmlFor: string;
  hint?: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  // The action (Show/Hide) sits beside the label but comes after the field
  // in tab order, so Tab goes straight from one field to the next.
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_auto] items-baseline gap-x-2 gap-y-1.5">
      <label htmlFor={htmlFor} className="col-start-1 row-start-1 text-sm font-semibold">
        {label}
      </label>
      <div className="col-span-2 row-start-2 flex min-w-0 flex-col gap-1.5">
        {children}
        {hint && <p className="text-xs text-muted">{hint}</p>}
      </div>
      {action && <div className="col-start-2 row-start-1">{action}</div>}
    </div>
  );
}

function Note({ tone, children }: { tone?: "missing" | "persistent"; children: ReactNode }) {
  const color = tone === "missing" ? "text-missing" : tone === "persistent" ? "text-persistent" : "text-muted";
  return <p className={`text-xs ${color}`}>{children}</p>;
}
