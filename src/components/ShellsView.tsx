import type { DetectedShell, HookStatus, PlatformInfo, ShellKind, ShellTarget, Syntax, TerminalSyncInfo } from "../lib/api";
import { pickStartupFile } from "../lib/files";
import { formatWhen } from "../lib/useEnvironment";
import { Button } from "./IconButton";
import { Help } from "./Help";
import { HELP } from "../lib/helpText";

interface Props {
  shells: DetectedShell[];
  platform: PlatformInfo | null;
  sync: TerminalSyncInfo | null;
  busy: boolean;
  onSetHook: (kind: ShellKind, name: string, install: boolean) => void;
  startupFiles: ShellTarget[];
  onSaveStartupFiles: (targets: ShellTarget[]) => Promise<boolean>;
  onError: (message: string) => void;
}

const SYNTAX_LABEL: Record<Syntax, string> = {
  posix: "bash / zsh",
  fish: "fish",
  nushell: "Nushell",
  powerShell: "PowerShell",
};

/** macOS/Linux: which startup files VARden writes variables into. */
function StartupFiles({
  files,
  busy,
  onSave,
  onError,
}: {
  files: ShellTarget[];
  busy: boolean;
  onSave: (targets: ShellTarget[]) => Promise<boolean>;
  onError: (message: string) => void;
}) {
  const add = async () => {
    try {
      const path = await pickStartupFile();
      if (path && !files.some((f) => f.path === path)) await onSave([...files, { path, syntax: null }]);
    } catch (e) {
      onError(String(e));
    }
  };
  return (
    <section className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="flex items-center gap-2 font-semibold">
          Where variables are saved <Help label="What are startup files?">{HELP.startupFiles}</Help>
        </h2>
        <Button className="border border-rule" disabled={busy} onClick={() => void add()}>
          Add startup file…
        </Button>
      </div>
      <ul className="divide-y divide-rule overflow-hidden rounded-lg border border-rule bg-panel">
        {files.map((f) => (
          <li key={f.path} className="flex items-center gap-3 px-4 py-2.5">
            <span className="min-w-0 flex-1 truncate font-mono text-[0.85rem]" title={f.path}>
              {f.path}
            </span>
            {f.syntax && <span className="shrink-0 text-xs text-muted">{SYNTAX_LABEL[f.syntax]}</span>}
            <Button
              disabled={busy || files.length === 1}
              title={files.length === 1 ? "Keep at least one startup file" : undefined}
              onClick={() => void onSave(files.filter((x) => x.path !== f.path))}
            >
              Remove
            </Button>
          </li>
        ))}
      </ul>
      <p className="text-xs text-muted">
        Removing a file only stops VARden from writing to it; the variables already in it stay until you edit them.
      </p>
    </section>
  );
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
        <Help label="What does installing terminal sync do?">{HELP.hook}</Help>
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

export function ShellsView({
  shells,
  platform,
  sync,
  busy,
  onSetHook,
  startupFiles,
  onSaveStartupFiles,
  onError,
}: Props) {
  const windows = platform?.os === "windows";
  const hookFor = (kind: ShellKind) => sync?.hooks.find((h) => h.kind === kind);

  return (
    <div className="flex max-w-3xl flex-col gap-5">
      <div>
        <h1 className="text-xl font-semibold">Shells</h1>
        <p className="mt-1 max-w-[68ch] text-sm text-muted">
          {windows
            ? "Every shell below reads your user environment from the registry, so new terminals always get your saved variables."
            : "Saved variables are written into a managed block in your login shell's startup file, so new terminals always get them."}{" "}
          To update terminals that are already open, install terminal sync and use <em>Sync to terminals</em>. Installing adds a
          short, clearly marked block to the shell's startup file (backed up first). Synced terminals update before their next prompt.
        </p>
        {sync?.lastSyncedAt && (
          <p className="mt-2 text-sm text-muted">Last synced {formatWhen(sync.lastSyncedAt)}.</p>
        )}
      </div>

      {platform?.configurableTargets && (
        <StartupFiles files={startupFiles} busy={busy} onSave={onSaveStartupFiles} onError={onError} />
      )}

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
              <dl className="grid grid-cols-[100px_minmax(0,1fr)] gap-x-3 gap-y-0.5 text-sm">
                <dt className="text-muted">Program</dt>
                <dd className="truncate font-mono text-[0.85rem]" title={s.executable ?? undefined}>
                  {s.executable ?? <span className="font-sans text-faint">Not found</span>}
                </dd>
                <dt className="text-muted">Startup file</dt>
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
          Windows PowerShell 5.1 only runs its startup file when its execution policy allows local scripts (for example{" "}
          <span className="font-mono">RemoteSigned</span>). PowerShell 7 allows them by default.
        </p>
      )}
    </div>
  );
}
