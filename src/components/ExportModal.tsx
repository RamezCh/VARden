import { useState } from "react";
import type { EnvVar, ExportItem } from "../lib/api";
import { rowId } from "../lib/filter";
import { plural } from "../lib/useEnvironment";
import { Button } from "./IconButton";
import { Modal } from "./Modal";

interface Props {
  vars: EnvVar[];
  /** Rows to pre-select (what the table currently shows). */
  initial: EnvVar[];
  busy: boolean;
  onClose: () => void;
  onExport: (items: ExportItem[]) => Promise<boolean>;
}

export function ExportModal({ vars, initial, busy, onClose, onExport }: Props) {
  const [picked, setPicked] = useState<ReadonlySet<string>>(
    new Set(initial.filter((v) => v.value !== "").map(rowId)),
  );
  const chosen = vars.filter((v) => picked.has(rowId(v)));
  const secrets = chosen.filter((v) => v.sensitive).length;

  const toggle = (id: string) =>
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  return (
    <Modal
      open
      onClose={onClose}
      title="Export to a .env file"
      description="Pick the variables to write. You choose where to save next."
      footer={
        <>
          {secrets > 0 && (
            <p className="mr-auto text-sm text-persistent">
              {plural(secrets, "secret")} will be written as plain text.
            </p>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            disabled={busy || chosen.length === 0}
            onClick={async () => {
              if (await onExport(chosen.map(({ key, scope }) => ({ key, scope })))) onClose();
            }}
          >
            Save {plural(chosen.length, "variable")}…
          </Button>
        </>
      }
    >
      <div className="mb-3 flex justify-end text-sm">
        <button
          type="button"
          className="text-muted underline-offset-4 hover:text-fg hover:underline"
          onClick={() => setPicked(new Set(picked.size === vars.length ? [] : vars.map(rowId)))}
        >
          {picked.size === vars.length ? "Select none" : "Select all"}
        </button>
      </div>
      <ul className="divide-y divide-rule overflow-hidden rounded-lg border border-rule">
        {vars.map((v) => {
          const id = rowId(v);
          return (
            <li key={id}>
              <label className="flex cursor-pointer items-center gap-3 px-3 py-2 hover:bg-raised/40">
                <input
                  type="checkbox"
                  checked={picked.has(id)}
                  onChange={() => toggle(id)}
                  className="h-4 w-4 accent-[var(--color-process)]"
                />
                <span className="min-w-0 flex-1 truncate font-mono text-[0.9rem]">{v.key}</span>
                {v.sensitive && <span className="text-xs text-persistent">Secret</span>}
                <span className="w-16 text-right text-xs text-muted">{v.scope === "user" ? "User" : "Session"}</span>
              </label>
            </li>
          );
        })}
      </ul>
    </Modal>
  );
}
