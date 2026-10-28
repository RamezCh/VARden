import type { DetectedShell, HookStatus, PlatformInfo, ShellKind, TerminalSyncInfo } from "../lib/api";
import { formatWhen } from "../lib/useEnvironment";
import { Button } from "./IconButton";

interface Props {
  shells: DetectedShell[];
  platform: PlatformInfo | null;
  sync: TerminalSyncInfo | null;
  busy: boolean;
  onSetHook: (kind: ShellKind, name: string, install: boolean) => void;
}

function SyncCell({ hook, busy, onSetHook }: { hook?: HookStatus; busy: boolean; onSetHook: Props["onSetHook"] }) {
  if (!hook) return null;
  if (hook.unsupported) return <p className="text-sm text-muted">{hook.unsupported}</p>;
  if (hook.problem) return <p className="text-sm text-missing">{hook.problem}</p>;
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <p className="flex items-center gap-2 text-sm">
        <span className={`h-2 w-2 rounded-full ${hook.installed ? "bg-synced" : "bg-faint"}`} />
        {hook.installed ? "Terminal sync installed" : "Terminal sync not installed"}
      </p>
      <Button
        variant={hook.installed ? "quiet" : "primary"}
        className={hook.installed ? "border border-rule" : ""}
        disabled={busy}
        onClick={() => onSetHook(hook.kind, hook.name, !hook.installed)}
      >
        {hook.installed ? "Remove" : "Install"}
      </Button>
    </div>
  );
}

export function ShellsView({ shells, platform, sync, busy, onSetHook }: Props) {
  const windows = platform?.os === "windows";
  const hookFor = (kind: ShellKind) => sync?.hooks.find((h) => h.kind === kind);

  return (
    <div className="flex max-w-3xl flex-col gap-5">
      <div>
        <h1 className="text-xl font-semibold">Shells</h1>
        <p className="mt-1 max-w-[68ch] text-sm text-muted">
          {windows
            ? "Every shell below reads your user environment from the registry, so new terminals always get your saved variables."
            : "Saved variables are written into a managed block in your login shell's profile, so new terminals always get them."}{" "}
          To update terminals that are already open, install terminal sync and use <em>Sync to terminals</em>. Installing adds a
          short, clearly marked block to the shell's profile (backed up first). Synced terminals update before their next prompt.
        </p>
        {sync?.lastSyncedAt && (
          <p className="mt-2 text-sm text-muted">Last synced {formatWhen(sync.lastSyncedAt)}.</p>
        )}
      </div>

      {shells.length === 0 ? (
        <p className="text-sm text-muted">No shells were found on this computer.</p>
      ) : (
        <ul className="divide-y divide-rule overflow-hidden rounded-lg border border-rule bg-panel">
          {shells.map((s) => (
            <li key={s.kind} className="flex flex-col gap-2 px-4 py-3">
              <div className="flex items-baseline gap-2">
                <span className="font-semibold">{s.name}</span>
                {s.isDefault && <span className="text-xs text-synced">{windows ? "Preferred" : "Login shell"}</span>}
              </div>
              <dl className="grid grid-cols-[88px_minmax(0,1fr)] gap-x-3 gap-y-0.5 text-sm">
                <dt className="text-muted">Program</dt>
                <dd className="truncate font-mono text-[0.85rem]" title={s.executable ?? undefined}>
                  {s.executable ?? <span className="font-sans text-faint">Not found</span>}
                </dd>
                <dt className="text-muted">Profile</dt>
                <dd className="truncate font-mono text-[0.85rem]" title={s.configPath ?? undefined}>
                  {s.configPath ?? <span className="font-sans text-faint">None</span>}
                  {s.configPath && !s.configExists && <span className="ml-2 font-sans text-faint">(not created yet)</span>}
                </dd>
              </dl>
              <SyncCell hook={hookFor(s.kind)} busy={busy} onSetHook={onSetHook} />
            </li>
          ))}
        </ul>
      )}

      {windows && (
        <p className="max-w-[68ch] text-xs text-muted">
          Windows PowerShell 5.1 only loads profiles when its execution policy allows local scripts (for example{" "}
          <span className="font-mono">RemoteSigned</span>). PowerShell 7 allows them by default.
        </p>
      )}
    </div>
  );
}
