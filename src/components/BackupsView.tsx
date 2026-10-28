import { useState } from "react";
import type { BackupEntry, PlatformInfo } from "../lib/api";
import { formatWhen } from "../lib/useEnvironment";
import { Button } from "./IconButton";

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
                  <div className="flex basis-full flex-wrap items-center justify-end gap-2 border-t border-rule pt-2">
                    <span className="mr-auto text-sm">Restore this state? The current one is backed up first.</span>
                    <Button onClick={() => setConfirming(null)}>Cancel</Button>
                    <Button
                      variant="primary"
                      disabled={busy}
                      onClick={async () => {
                        if (await onRestore(b)) setConfirming(null);
                      }}
                    >
                      Restore
                    </Button>
                  </div>
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
