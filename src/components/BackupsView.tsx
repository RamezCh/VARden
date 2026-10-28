import { useEffect, useState, type ReactNode } from "react";
import { syncApi, type BackupEntry, type PlatformInfo, type RestorePreview, type VarChange } from "../lib/api";
import { plural } from "../lib/useEnvironment";
import { Button, IconButton } from "./IconButton";
import { Help } from "./Help";
import { TrashIcon } from "./icons";
import { HELP } from "../lib/helpText";

const MASK = "••••••";

function ChangeLine({ c }: { c: VarChange }) {
  const show = (v: string) => (c.sensitive ? MASK : v === "" ? "empty" : v);
  return (
    <li className="grid grid-cols-[minmax(120px,0.8fr)_minmax(0,2fr)] gap-3 py-1 text-sm">
      <span className="truncate font-mono">{c.key}</span>
      <span className="min-w-0 truncate">
        {c.current === null ? (
          <>
            <span className="text-synced">Comes back</span> <span className="font-mono text-muted">{show(c.restored ?? "")}</span>
          </>
        ) : c.restored === null ? (
          <span className="text-missing">Removed</span>
        ) : (
          <>
            <span className="font-mono text-muted line-through decoration-faint">{show(c.current)}</span>
            <span className="mx-1.5 text-faint">to</span>
            <span className="font-mono">{show(c.restored)}</span>
          </>
        )}
      </span>
    </li>
  );
}

/** What restoring would change, so nobody restores blind. */
function RestoreConfirm({
  backup,
  busy,
  onCancel,
  onRestore,
}: {
  backup: BackupEntry;
  busy: boolean;
  onCancel: () => void;
  onRestore: () => void;
}) {
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    syncApi.previewRestore(backup.id).then(setPreview, (e) => setError(String(e)));
  }, [backup.id]);

  const nothing = preview && preview.changes.length === 0 && !preview.otherChanges && !preview.deletesFile;

  return (
    <div className="flex basis-full flex-col gap-2 border-t border-rule pt-3">
      {error && <p className="text-sm text-missing">Couldn't compare with the current state: {error}</p>}
      {!preview && !error && <p className="text-sm text-muted">Comparing with the current state…</p>}
      {nothing && <p className="text-sm text-muted">This backup matches the current state. Restoring would change nothing.</p>}
      {preview && preview.changes.length > 0 && (
        <>
          <p className="text-sm">Restoring changes {plural(preview.changes.length, "variable")}:</p>
          <ul className="max-h-48 overflow-y-auto rounded-md bg-ink px-3 py-1.5">
            {preview.changes.map((c) => (
              <ChangeLine key={c.key} c={c} />
            ))}
          </ul>
        </>
      )}
      {preview?.otherChanges && (
        <p className="text-sm text-persistent">The file also differs outside the variables block; those lines are restored too.</p>
      )}
      {preview?.deletesFile && (
        <p className="text-sm text-persistent">The file didn't exist when this backup was taken, so restoring deletes it.</p>
      )}
      <div className="flex flex-wrap items-center justify-end gap-2">
        <span className="mr-auto text-xs text-muted">The current state is backed up first, so you can undo this.</span>
        <Button onClick={onCancel}>Cancel</Button>
        <Button variant="primary" disabled={busy || !preview || Boolean(nothing)} onClick={onRestore}>
          Restore
        </Button>
      </div>
    </div>
  );
}




function targetName(b: BackupEntry): string {
  return b.target.type === "registry" ? "Windows user environment" : b.target.path;
}

const dayKey = (iso: string) => new Date(iso).toDateString();

/** "Today", "Yesterday", otherwise "Monday, Oct 26" (plus the year if it differs). */
function dayLabel(iso: string): string {
  const d = new Date(iso);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  if (d.toDateString() === today.toDateString()) return "Today";
  if (d.toDateString() === yesterday.toDateString()) return "Yesterday";
  return d.toLocaleDateString(undefined, {
    weekday: "long",
    month: "short",
    day: "numeric",
    year: d.getFullYear() === today.getFullYear() ? undefined : "numeric",
  });
}

const timeOf = (iso: string) => new Date(iso).toLocaleTimeString(undefined, { timeStyle: "short" });

type Confirming =
  | { kind: "restore"; id: string }
  | { kind: "delete"; id: string }
  | { kind: "day"; day: string }
  | { kind: "all" }
  | null;

interface Props {
  backups: BackupEntry[];
  platform: PlatformInfo | null;
  busy: boolean;
  onRestore: (b: BackupEntry) => Promise<boolean>;
  onDelete: (ids: string[]) => Promise<boolean>;
}

/** An inline "are you sure" strip with the destructive button last. */
function ConfirmStrip({
  children,
  action,
  busy,
  onCancel,
  onConfirm,
}: {
  children: ReactNode;
  action: string;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center justify-end gap-2 rounded-md bg-missing/[0.08] px-3 py-2">
      <span className="mr-auto text-sm">{children}</span>
      <Button onClick={onCancel} autoFocus>
        Cancel
      </Button>
      <Button variant="danger" disabled={busy} onClick={onConfirm}>
        {action}
      </Button>
    </div>
  );
}

export function BackupsView({ backups, platform, busy, onRestore, onDelete }: Props) {
  const [confirming, setConfirming] = useState<Confirming>(null);
  const close = () => setConfirming(null);

  // Newest first, grouped by calendar day.
  const days = new Map<string, BackupEntry[]>();
  for (const b of backups) days.set(dayKey(b.createdAt), [...(days.get(dayKey(b.createdAt)) ?? []), b]);
  const deletable = (list: BackupEntry[]) => list.filter((b) => !b.original);
  const manyTargets = new Set(backups.map(targetName)).size > 1;
  const allDeletable = deletable(backups);
  const originals = backups.length - allDeletable.length;

  const remove = async (ids: string[]) => {
    if (await onDelete(ids)) close();
  };

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="flex items-center gap-2 text-xl font-semibold">
            Backups <Help label="How do backups work?">{HELP.backups}</Help>
          </h1>
          <p className="mt-1 max-w-[62ch] text-sm text-muted">
            Saved before every change. Restore any of them, or delete the ones you don't need.
          </p>
          {platform && <p className="mt-2 truncate font-mono text-xs text-faint">{platform.backupDir}</p>}
        </div>
        <Button
          className="border border-rule"
          disabled={allDeletable.length === 0}
          onClick={() => setConfirming({ kind: "all" })}
        >
          Clear backups
        </Button>
      </div>

      {confirming?.kind === "all" && (
        <ConfirmStrip
          action={`Delete ${plural(allDeletable.length, "backup")}`}
          busy={busy}
          onCancel={close}
          onConfirm={() => void remove(allDeletable.map((b) => b.id))}
        >
          Delete all {plural(allDeletable.length, "backup")}?{" "}
          {originals > 0 && `The original${originals === 1 ? " is" : "s are"} kept.`} This can't be undone.
        </ConfirmStrip>
      )}

      {days.size === 0 && <p className="text-sm text-muted">No backups yet. One is saved the first time you change something.</p>}

      {[...days].map(([day, entries]) => {
        const removable = deletable(entries);
        return (
          <section key={day} className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-3">
              <h2 className="text-sm font-semibold">
                {dayLabel(entries[0].createdAt)}{" "}
                <span className="font-normal text-muted">{plural(entries.length, "backup")}</span>
              </h2>
              {removable.length > 0 && (
                <button
                  type="button"
                  className="text-sm text-muted underline-offset-4 hover:text-missing hover:underline"
                  onClick={() => setConfirming({ kind: "day", day })}
                >
                  Delete this day
                </button>
              )}
            </div>

            {confirming?.kind === "day" && confirming.day === day && (
              <ConfirmStrip
                action={`Delete ${plural(removable.length, "backup")}`}
                busy={busy}
                onCancel={close}
                onConfirm={() => void remove(removable.map((b) => b.id))}
              >
                Delete {plural(removable.length, "backup")} from {dayLabel(entries[0].createdAt).toLowerCase()}?
                {removable.length < entries.length && " The original is kept."}
              </ConfirmStrip>
            )}

            <ol className="divide-y divide-rule overflow-hidden rounded-lg border border-rule bg-panel">
              {entries.map((b) => (
                <li key={b.id} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
                  <span className="w-[4.5rem] shrink-0 font-mono text-sm tabular-nums text-muted">{timeOf(b.createdAt)}</span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate">{b.reason}</span>
                      {b.original && (
                        <span className="flex shrink-0 items-center gap-1.5 rounded bg-synced/15 px-1.5 py-0.5 text-xs text-synced">
                          Original <Help label="What is the original backup?">{HELP.original}</Help>
                        </span>
                      )}
                    </div>
                    {manyTargets && (
                      <p className="truncate font-mono text-xs text-faint" title={targetName(b)}>
                        {targetName(b)}
                      </p>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    <Button className="border border-rule" onClick={() => setConfirming({ kind: "restore", id: b.id })}>
                      Restore
                    </Button>
                    {b.original ? (
                      <span className="w-7" aria-hidden />
                    ) : (
                      <IconButton
                        label={`Delete the backup from ${timeOf(b.createdAt)}`}
                        className="hover:text-missing"
                        onClick={() => setConfirming({ kind: "delete", id: b.id })}
                      >
                        <TrashIcon />
                      </IconButton>
                    )}
                  </div>

                  {confirming?.kind === "restore" && confirming.id === b.id && (
                    <RestoreConfirm
                      backup={b}
                      busy={busy}
                      onCancel={close}
                      onRestore={async () => {
                        if (await onRestore(b)) close();
                      }}
                    />
                  )}
                  {confirming?.kind === "delete" && confirming.id === b.id && (
                    <div className="basis-full">
                      <ConfirmStrip action="Delete" busy={busy} onCancel={close} onConfirm={() => void remove([b.id])}>
                        Delete this backup? You can't restore it afterwards.
                      </ConfirmStrip>
                    </div>
                  )}
                </li>
              ))}
            </ol>
          </section>
        );
      })}
    </div>
  );
}
