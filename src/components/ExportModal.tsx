import { useEffect, useState } from "react";
import { editApi, type EnvVar, type ExportItem } from "../lib/api";
import { fileName } from "../lib/files";
import { FolderIcon } from "./icons";
import { rowId } from "../lib/filter";
import { plural } from "../lib/useEnvironment";
import { Button } from "./IconButton";
import { Modal } from "./Modal";
import { Help } from "./Help";
import { HELP } from "../lib/helpText";

interface Props {
  vars: EnvVar[];
  /** Rows to pre-select (what the table currently shows). */
  initial: EnvVar[];
  busy: boolean;
  onClose: () => void;
  /** `dir`: start the save dialog in this folder. */
  onExport: (items: ExportItem[], dir?: string) => Promise<boolean>;
}

export function ExportModal({ vars, initial, busy, onClose, onExport }: Props) {
  const [picked, setPicked] = useState<ReadonlySet<string>>(
    new Set(initial.filter((v) => v.value !== "").map(rowId)),
  );
  const [recent, setRecent] = useState<string[]>([]);
  useEffect(() => {
    editApi
      .recentExportFolders()
      .then(setRecent)
      .catch(() => {});
  }, []);
  const chosen = vars.filter((v) => picked.has(rowId(v)));
  const items = chosen.map(({ key, scope }) => ({ key, scope }));
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
      description={
        <span className="flex items-center gap-1.5">
          Pick the variables to write, then choose a folder, usually your project's.
          <Help label="What is a .env file?">{HELP.dotenv}</Help>
        </span>
      }
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
              if (await onExport(items, recent[0])) onClose();
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
      {recent.length > 0 && (
        <div className="mb-4">
          <h3 className="mb-1.5 text-[0.8rem] text-muted">Recent folders</h3>
          <div className="flex flex-wrap gap-2">
            {recent.map((dir) => (
              <button
                key={dir}
                type="button"
                title={`Save in ${dir}`}
                disabled={busy || chosen.length === 0}
                onClick={async () => {
                  if (await onExport(items, dir)) onClose();
                }}
                className="inline-flex max-w-full items-center gap-1.5 rounded-md border border-rule px-2.5 py-1 text-sm hover:border-process hover:bg-raised disabled:opacity-50"
              >
                <FolderIcon className="shrink-0 text-muted" />
                <span className="truncate">{fileName(dir) || dir}</span>
              </button>
            ))}
          </div>
        </div>
      )}
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
                <span className="w-16 text-right text-xs text-muted">{v.scope === "user" ? "User" : "Temporary"}</span>
              </label>
            </li>
          );
        })}
      </ul>
    </Modal>
  );
}
