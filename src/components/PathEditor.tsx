import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { editApi, type PathEntry, type PathInfo } from "../lib/api";
import { pickFolder } from "../lib/files";
import { BASE, diffPath, entryError, moveSlot, toSlots, type Slot } from "../lib/pathEdit";
import { plural } from "../lib/useEnvironment";
import { HELP } from "../lib/helpText";
import { Button, IconButton } from "./IconButton";
import { Help } from "./Help";
import { ArrowDownIcon, ArrowUpIcon, FolderIcon, LockIcon, PlusIcon, TrashIcon } from "./icons";
import { Modal } from "./Modal";
import { input } from "./VariableTable";

interface Props {
  windows: boolean;
  busy: boolean;
  onClose: () => void;
  onSave: (entries: string[], baseAt: number) => Promise<boolean>;
}

/** One of your entries, with a stable id for React while it's edited. */
interface Item {
  id: number;
  value: string;
}
type Row = Item | typeof BASE;

let nextId = 1;
const item = (value: string): Item => ({ id: nextId++, value });

function rowsFrom(info: PathInfo): Row[] {
  return toSlots(
    info.entries.map((e) => e.value),
    info.baseAt,
  ).map((s) => (s === BASE ? BASE : item(s)));
}

const slotsOf = (rows: Row[]): Slot[] => rows.map((r) => (r === BASE ? BASE : r.value));
const entriesOf = (rows: Row[]) => rows.filter((r): r is Item => r !== BASE).map((r) => r.value);
const baseAtOf = (rows: Row[]) => Math.max(rows.indexOf(BASE), 0);

export function PathEditor({ windows, busy, onClose, onSave }: Props) {
  const [info, setInfo] = useState<PathInfo | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [original, setOriginal] = useState<Row[]>([]);
  const [rows, setRows] = useState<Row[]>([]);
  const [draft, setDraft] = useState("");
  const [draftError, setDraftError] = useState<string | null>(null);
  const [step, setStep] = useState<"edit" | "review">("edit");
  const [showBase, setShowBase] = useState(false);
  const addRef = useRef<HTMLInputElement>(null);

  const sep = windows ? ";" : ":";
  const unix = !windows;

  useEffect(() => {
    editApi
      .pathInfo()
      .then((i) => {
        const r = rowsFrom(i);
        setInfo(i);
        setOriginal(r);
        setRows(r);
      })
      .catch((e) => setLoadError(String(e)));
  }, []);

  // Re-check folders as you edit (debounced), so warnings follow the list.
  const entries = entriesOf(rows);
  const baseAt = baseAtOf(rows);
  const checkKey = JSON.stringify([entries, baseAt]);
  useEffect(() => {
    if (!info) return;
    const t = window.setTimeout(() => {
      editApi
        .checkPathEntries(entries, baseAt)
        .then(setInfo)
        .catch(() => {});
    }, 250);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [checkKey]);

  /** The latest check for the entry at `index` among your entries. */
  const checkFor = (index: number, value: string): PathEntry | null => {
    const e = info?.entries[index];
    return e && e.value === value ? e : null;
  };

  const errors = entries.map((e) => entryError(e, sep, unix));
  const hasErrors = errors.some(Boolean);
  const diff = useMemo(() => diffPath(slotsOf(original), slotsOf(rows), windows), [original, rows, windows]);

  const update = (id: number, value: string) => setRows((rs) => rs.map((r) => (r !== BASE && r.id === id ? { ...r, value } : r)));
  const remove = (id: number) => setRows((rs) => rs.filter((r) => r === BASE || r.id !== id));
  const move = (index: number, dir: -1 | 1) =>
    setRows((rs) => moveSlot(rs, index, dir, info?.baseMovable ?? false) ?? rs);

  const add = (value: string) => {
    const v = value.trim();
    const err = entryError(v, sep, unix);
    if (err) {
      setDraftError(err);
      return;
    }
    const norm = (s: string) => (windows ? s.toLowerCase() : s);
    if (entries.some((e) => norm(e) === norm(v))) {
      setDraftError("That folder is already in your PATH.");
      return;
    }
    setRows((rs) => [...rs, item(v)]);
    setDraft("");
    setDraftError(null);
    addRef.current?.focus();
  };

  const browse = async () => {
    const dir = await pickFolder("Add a folder to PATH").catch(() => null);
    if (dir) add(dir);
  };

  const missing = info?.entries.filter((e) => e.missing).length ?? 0;
  const duplicates = info?.entries.filter((e) => e.duplicate).length ?? 0;

  const footer =
    step === "edit" ? (
      <>
        {hasErrors && <p className="mr-auto text-sm text-missing">Fix the highlighted entries first.</p>}
        <Button onClick={onClose}>Cancel</Button>
        <Button variant="primary" disabled={!info || !diff.changed || hasErrors} onClick={() => setStep("review")}>
          Review changes
        </Button>
      </>
    ) : (
      <>
        <Button onClick={() => setStep("edit")}>Back</Button>
        <Button
          variant="primary"
          disabled={busy}
          onClick={async () => {
            if (await onSave(entries, baseAt)) onClose();
          }}
        >
          Save PATH
        </Button>
      </>
    );

  return (
    <Modal
      open
      onClose={onClose}
      title={step === "edit" ? "Edit your PATH" : "Review PATH changes"}
      description={
        <span className="flex items-center gap-1.5">
          {step === "edit"
            ? "Folders where your terminal looks for programs, searched from top to bottom."
            : "Nothing is saved until you confirm. A backup is taken first."}
          <Help label="How does the PATH editor keep things safe?">{HELP.path}</Help>
        </span>
      }
      footer={footer}
    >
      {loadError ? (
        <p className="text-sm text-missing">Couldn't read your PATH: {loadError}</p>
      ) : !info ? (
        <p className="text-sm text-muted">Reading your PATH…</p>
      ) : step === "edit" ? (
        <div className="flex flex-col gap-3">
          {info.note && (
            <p className="rounded-md border border-persistent/40 bg-persistent/10 px-3 py-2 text-sm text-persistent">{info.note}</p>
          )}
          <ol className="divide-y divide-rule overflow-hidden rounded-lg border border-rule">
            {rows.map((r, i) => {
              if (r === BASE) {
                return (
                  <SystemRow
                    key="base"
                    base={info.base}
                    windows={windows}
                    open={showBase}
                    onToggle={() => setShowBase(!showBase)}
                  />
                );
              }
              const index = rows.slice(0, i).filter((x) => x !== BASE).length;
              const check = checkFor(index, r.value);
              const error = errors[index];
              const canUp = moveSlot(rows, i, -1, info.baseMovable) !== null;
              const canDown = moveSlot(rows, i, 1, info.baseMovable) !== null;
              return (
                <li key={r.id} className="flex items-start gap-2 px-3 py-2">
                  <span className="mt-1.5 w-5 shrink-0 text-right font-mono text-xs text-faint">{index + 1}</span>
                  <div className="min-w-0 flex-1">
                    <input
                      aria-label={`PATH entry ${index + 1}`}
                      aria-invalid={Boolean(error)}
                      className={`${input} ${error ? "border-missing" : ""}`}
                      value={r.value}
                      spellCheck={false}
                      autoComplete="off"
                      onChange={(e) => update(r.id, e.target.value)}
                    />
                    <EntryNote error={error} check={check} />
                  </div>
                  <div className="flex shrink-0 items-center pt-0.5">
                    <IconButton label="Move up" disabled={!canUp} onClick={() => move(i, -1)}>
                      <ArrowUpIcon />
                    </IconButton>
                    <IconButton label="Move down" disabled={!canDown} onClick={() => move(i, 1)}>
                      <ArrowDownIcon />
                    </IconButton>
                    <IconButton label={`Remove ${r.value || "entry"}`} className="hover:text-missing" onClick={() => remove(r.id)}>
                      <TrashIcon />
                    </IconButton>
                  </div>
                </li>
              );
            })}
          </ol>
          {entries.length === 0 && (
            <p className="text-sm text-muted">You haven't added any folders of your own yet.</p>
          )}

          <div>
            <div className="flex items-center gap-2">
              <input
                ref={addRef}
                aria-label="Folder to add"
                className={input}
                placeholder={windows ? "Add a folder, e.g. %USERPROFILE%\\bin" : "Add a folder, e.g. ~/bin"}
                value={draft}
                spellCheck={false}
                autoComplete="off"
                data-autofocus
                onChange={(e) => {
                  setDraft(e.target.value);
                  setDraftError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    add(draft);
                  }
                }}
              />
              <Button className="border border-rule" onClick={() => void browse()}>
                <FolderIcon /> Browse…
              </Button>
              <Button className="border border-rule" disabled={draft.trim() === ""} onClick={() => add(draft)}>
                <PlusIcon /> Add
              </Button>
            </div>
            {draftError && <p className="mt-1 text-xs text-missing">{draftError}</p>}
          </div>
        </div>
      ) : (
        <Review
          rows={rows}
          diff={diff}
          info={info}
          missing={missing}
          duplicates={duplicates}
          windows={windows}
        />
      )}
    </Modal>
  );
}

function EntryNote({ error, check }: { error: string | null; check: PathEntry | null }) {
  const notes: ReactNode[] = [];
  if (error) notes.push(<span className="text-missing">{error}</span>);
  else if (check?.missing) notes.push(<span className="text-missing">This folder doesn't exist.</span>);
  if (!error && check?.duplicate) notes.push(<span className="text-persistent">Already on PATH above, so this copy is never used.</span>);
  if (!error && check?.expanded) {
    notes.push(
      <span className="text-muted">
        Points to <span className="font-mono">{check.expanded}</span>
      </span>,
    );
  }
  if (notes.length === 0) return null;
  return (
    <p className="mt-1 flex flex-col gap-0.5 text-xs">
      {notes.map((n, i) => (
        <span key={i} className="truncate">
          {n}
        </span>
      ))}
    </p>
  );
}

function SystemRow({
  base,
  windows,
  open,
  onToggle,
}: {
  base: PathEntry[];
  windows: boolean;
  open: boolean;
  onToggle: () => void;
}) {
  return (
    <li className="bg-ink/60 px-3 py-2">
      <div className="flex items-center gap-2">
        <LockIcon className="ml-1 shrink-0 text-faint" />
        <div className="min-w-0 flex-1 text-sm">
          <span className="font-semibold">System PATH</span>{" "}
          <span className="text-muted">
            · {plural(base.length, "folder")}, read-only
            {windows ? ", always searched first" : ""}
          </span>
        </div>
        <button type="button" className="text-sm text-muted underline-offset-4 hover:text-fg hover:underline" onClick={onToggle}>
          {open ? "Hide" : "Show"}
        </button>
      </div>
      {open && (
        <ul className="mt-2 space-y-0.5 border-l border-rule pl-3 ml-2">
          {base.map((e, i) => (
            <li key={i} className="flex min-w-0 items-baseline gap-2 text-xs">
              <span className="truncate font-mono text-muted" title={e.expanded ?? e.value}>
                {e.value}
              </span>
              {e.missing && <span className="shrink-0 text-missing">doesn't exist</span>}
              {e.duplicate && <span className="shrink-0 text-persistent">duplicate</span>}
            </li>
          ))}
          {base.length === 0 && <li className="text-xs text-muted">Empty</li>}
        </ul>
      )}
    </li>
  );
}

function Review({
  rows,
  diff,
  info,
  missing,
  duplicates,
  windows,
}: {
  rows: Row[];
  diff: ReturnType<typeof diffPath>;
  info: PathInfo;
  missing: number;
  duplicates: number;
  windows: boolean;
}) {
  const norm = (s: string) => (windows ? s.toLowerCase() : s);
  const added = new Set(diff.added.map(norm));
  const summary = [
    diff.added.length > 0 && `${diff.added.length} added`,
    diff.removed.length > 0 && `${diff.removed.length} removed`,
    diff.reordered && "order changed",
  ]
    .filter(Boolean)
    .join(", ");
  let n = 0;
  return (
    <div className="flex flex-col gap-4 text-sm">
      <p>
        <strong>{summary.charAt(0).toUpperCase() + summary.slice(1)}.</strong>{" "}
        <span className="text-muted">
          {windows
            ? "Only your part of PATH, in HKEY_CURRENT_USER, is written."
            : "Only the PATH line in VARden's block of your startup file is written."}
        </span>
      </p>

      {(missing > 0 || duplicates > 0) && (
        <ul className="space-y-1 rounded-md border border-persistent/40 bg-persistent/10 px-3 py-2 text-persistent">
          {missing > 0 && (
            <li>
              {plural(missing, "folder")} {missing === 1 ? "doesn't" : "don't"} exist. Programs skip {missing === 1 ? "it" : "them"}, so
              it's harmless, but check for typos.
            </li>
          )}
          {duplicates > 0 && (
            <li>
              {plural(duplicates, "entry")} {duplicates === 1 ? "is" : "are"} already earlier on PATH and won't be used.
            </li>
          )}
        </ul>
      )}

      <div>
        <h3 className="mb-1.5 text-[0.8rem] text-muted">New PATH, searched from top to bottom</h3>
        <ol className="divide-y divide-rule overflow-hidden rounded-lg border border-rule">
          {rows.map((r) =>
            r === BASE ? (
              <li key="base" className="flex items-center gap-2 bg-ink/60 px-3 py-1.5 text-muted">
                <LockIcon className="shrink-0 text-faint" /> System PATH ({plural(info.base.length, "folder")}, unchanged)
              </li>
            ) : (
              <li key={r.id} className="flex items-center gap-2 px-3 py-1.5">
                <span className="w-5 shrink-0 text-right font-mono text-xs text-faint">{++n}</span>
                <span className={`min-w-0 flex-1 truncate font-mono ${added.has(norm(r.value)) ? "text-synced" : ""}`}>
                  {r.value}
                </span>
                {added.has(norm(r.value)) && <span className="shrink-0 text-xs text-synced">Added</span>}
              </li>
            ),
          )}
        </ol>
      </div>

      {diff.removed.length > 0 && (
        <div>
          <h3 className="mb-1.5 text-[0.8rem] text-muted">Removed</h3>
          <ul className="space-y-1">
            {diff.removed.map((r) => (
              <li key={r} className="truncate font-mono text-missing line-through decoration-missing/60">
                {r}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
