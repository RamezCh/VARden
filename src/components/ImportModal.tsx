import { useEffect, useMemo, useState } from "react";
import type { EnvEntry, EnvVar, ParsedEnv, Scope } from "../lib/api";
import { looksSensitive } from "../lib/filter";
import { fileName } from "../lib/files";
import { plural } from "../lib/useEnvironment";
import { Button } from "./IconButton";
import { Modal } from "./Modal";
import { SecretValue } from "./SecretValue";

export interface ImportSource {
  path: string;
  parsed: ParsedEnv;
}

interface Props {
  source: ImportSource | null;
  vars: EnvVar[];
  caseInsensitive: boolean;
  busy: boolean;
  onChooseFile: () => void;
  onClose: () => void;
  onImport: (entries: EnvEntry[], scope: Scope, source: string) => Promise<boolean>;
}

type Change = "new" | "replace" | "same";
const CHANGE_LABEL: Record<Change, string> = { new: "New", replace: "Replaces current value", same: "Unchanged" };

export function ImportModal({ source, vars, caseInsensitive, busy, onChooseFile, onClose, onImport }: Props) {
  const [scope, setScope] = useState<Scope>("user");
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set());
  const [revealed, setRevealed] = useState<ReadonlySet<string>>(new Set());

  const rows = useMemo(() => {
    if (!source) return [];
    const same = (a: string, b: string) => (caseInsensitive ? a.toLowerCase() === b.toLowerCase() : a === b);
    return source.parsed.entries.map((e) => {
      const existing = vars.find((v) => v.scope === scope && same(v.key, e.key));
      const change: Change = !existing ? "new" : existing.value === e.value ? "same" : "replace";
      return { ...e, change, sensitive: existing?.sensitive || looksSensitive(e.key) };
    });
  }, [source, vars, scope, caseInsensitive]);

  // Pre-select everything that would actually change something.
  useEffect(() => {
    setPicked(new Set(rows.filter((r) => r.change !== "same").map((r) => r.key)));
  }, [rows]);

  const chosen = rows.filter((r) => picked.has(r.key));
  const toggle = (key: string) =>
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(key)) n.delete(key);
      else n.add(key);
      return n;
    });

  return (
    <Modal
      open
      onClose={onClose}
      title="Import from a .env file"
      description={
        source ? (
          <span className="font-mono">{source.path}</span>
        ) : (
          "Choose a file, or drop one anywhere on the window."
        )
      }
      footer={
        <>
          {source && (
            <Button className="mr-auto" onClick={onChooseFile}>
              Choose another file
            </Button>
          )}
          <Button onClick={onClose}>Cancel</Button>
          {source ? (
            <Button
              variant="primary"
              disabled={busy || chosen.length === 0}
              onClick={async () => {
                const entries = chosen.map(({ key, value }) => ({ key, value }));
                if (await onImport(entries, scope, fileName(source.path))) onClose();
              }}
            >
              Import {plural(chosen.length, "variable")}
            </Button>
          ) : (
            <Button variant="primary" onClick={onChooseFile}>
              Choose file
            </Button>
          )}
        </>
      }
    >
      {!source ? (
        <div className="grid place-items-center rounded-lg border border-dashed border-rule px-6 py-14 text-center text-sm text-muted">
          <p className="max-w-[46ch]">
            Lines like <span className="font-mono text-fg">KEY=value</span> are read, including quoted and multi-line
            values. You'll see every variable before anything is saved.
          </p>
        </div>
      ) : (
        <div className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-3 text-sm">
            <span className="text-muted">Save to</span>
            <div role="radiogroup" aria-label="Scope" className="flex rounded-md border border-rule bg-ink p-0.5">
              {(["user", "process"] as Scope[]).map((s) => (
                <button
                  key={s}
                  type="button"
                  role="radio"
                  aria-checked={scope === s}
                  onClick={() => setScope(s)}
                  className={`rounded px-3 py-1 ${scope === s ? "bg-raised text-fg" : "text-muted hover:text-fg"}`}
                >
                  {s === "user" ? "User environment" : "This session only"}
                </button>
              ))}
            </div>
            <button
              type="button"
              className="ml-auto text-muted underline-offset-4 hover:text-fg hover:underline"
              onClick={() => setPicked(new Set(picked.size === rows.length ? [] : rows.map((r) => r.key)))}
            >
              {picked.size === rows.length ? "Select none" : "Select all"}
            </button>
          </div>

          {rows.length === 0 ? (
            <p className="text-sm text-muted">This file has no variables.</p>
          ) : (
            <ul className="divide-y divide-rule overflow-hidden rounded-lg border border-rule">
              {rows.map((r) => (
                <li key={r.key} className="grid grid-cols-[auto_minmax(0,1fr)_minmax(0,1.4fr)_10rem] items-center gap-3 px-3 py-2">
                  <input
                    type="checkbox"
                    aria-label={`Import ${r.key}`}
                    checked={picked.has(r.key)}
                    onChange={() => toggle(r.key)}
                    className="h-4 w-4 accent-[var(--color-process)]"
                  />
                  <span className="truncate font-mono text-[0.9rem]">{r.key}</span>
                  <SecretValue
                    value={r.value}
                    sensitive={r.sensitive}
                    revealed={revealed.has(r.key)}
                    onToggle={() =>
                      setRevealed((p) => {
                        const n = new Set(p);
                        if (n.has(r.key)) n.delete(r.key);
                        else n.add(r.key);
                        return n;
                      })
                    }
                  />
                  <span
                    className={`justify-self-end text-xs ${
                      r.change === "replace" ? "text-persistent" : r.change === "new" ? "text-synced" : "text-faint"
                    }`}
                  >
                    {CHANGE_LABEL[r.change]}
                  </span>
                </li>
              ))}
            </ul>
          )}

          {source.parsed.skipped.length > 0 && (
            <details className="text-sm">
              <summary className="cursor-pointer text-persistent">
                {plural(source.parsed.skipped.length, "line")} couldn't be read
              </summary>
              <ul className="mt-2 space-y-1">
                {source.parsed.skipped.map((s) => (
                  <li key={s.line} className="grid grid-cols-[4.5rem_minmax(0,1fr)] gap-2">
                    <span className="text-muted">Line {s.line}</span>
                    <span>
                      {s.reason}: <span className="font-mono text-muted">{s.text.trim().slice(0, 80)}</span>
                    </span>
                  </li>
                ))}
              </ul>
            </details>
          )}

          {scope === "user" && chosen.length > 0 && (
            <p className="text-xs text-muted">A backup is saved first, and the whole import can be undone in one step.</p>
          )}
        </div>
      )}
    </Modal>
  );
}
