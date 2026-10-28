import { useEffect, useState } from "react";
import { syncApi, type BackupEntry, type PlatformInfo, type RestorePreview, type VarChange } from "../lib/api";
import { formatWhen, plural } from "../lib/useEnvironment";
import { Button } from "./IconButton";

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
  return b.target.type === "registry" ? "Windows user environment (registry)" : b.target.path;
}

function formatSize(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
}

interface Props {
  backups: BackupEntry[];
  platform: PlatformInfo | null;
  busy: boolean;
  onRestore: (b: BackupEntry) => Promise<boolean>;
}

export function BackupsView({ backups, platform, busy, onRestore }: Props) {
  const [confirming, setConfirming] = useState<string | null>(null);

  // Group by what was backed up, keeping newest-first order inside each group.
  const groups = new Map<string, BackupEntry[]>();
  for (const b of backups) {
    const name = targetName(b);
    groups.set(name, [...(groups.get(name) ?? []), b]);
  }

  return (
    <div className="flex max-w-3xl flex-col gap-5">
      <div>
        <h1 className="text-xl font-semibold">Backups</h1>
        <p className="mt-1 max-w-[68ch] text-sm text-muted">
          A backup is saved before every change. The original, from before Environment Manager first changed anything,
          is never removed. If a change leads back to a state that is already saved, no duplicate is made.
        </p>
        {platform && <p className="mt-2 truncate font-mono text-xs text-faint">{platform.backupDir}</p>}
      </div>

      {groups.size === 0 && <p className="text-sm text-muted">No backups yet. One is saved the first time you change something.</p>}

      {[...groups].map(([name, entries]) => (
        <section key={name} className="flex flex-col gap-2">
          <h2 className="truncate font-mono text-sm text-muted" title={name}>
            {name}
          </h2>
          <ol className="divide-y divide-rule overflow-hidden rounded-lg border border-rule bg-panel">
            {entries.map((b) => (
              <li key={b.id} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
                <div className="min-w-0 flex-1">
                  <div className="flex items-baseline gap-2">
                    <span className="font-semibold">{formatWhen(b.createdAt)}</span>
                    {b.original && (
                      <span className="rounded bg-synced/15 px-1.5 py-0.5 text-xs text-synced" title="State before Environment Manager changed anything. Never removed.">
                        Original
                      </span>
                    )}
                  </div>
                  <p className="text-sm text-muted">
                    {b.reason}, {formatSize(b.sizeBytes)}
                  </p>
                </div>
                {confirming === b.id ? (
                  <RestoreConfirm
                    backup={b}
                    busy={busy}
                    onCancel={() => setConfirming(null)}
                    onRestore={async () => {
                      if (await onRestore(b)) setConfirming(null);
                    }}
                  />
                ) : (
                  <Button className="border border-rule" onClick={() => setConfirming(b.id)}>
                    Restore
                  </Button>
                )}
              </li>
            ))}
          </ol>
        </section>
      ))}
    </div>
  );
}
