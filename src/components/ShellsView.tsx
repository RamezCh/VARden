import type { DetectedShell, HookStatus, PlatformInfo, ShellKind, ShellTarget, Syntax, TerminalSyncInfo } from "../lib/api";
import { fileName, pickStartupFile } from "../lib/files";
import { TerminalIcon } from "./icons";
import { PageHeader } from "./PageHeader";
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
        <Button variant="secondary" disabled={busy} onClick={() => void add()}>
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

/** A file path shown as its name, with the folder underneath (full path on hover). */
function PathField({ label, path, empty, note }: { label: string; path: string | null; empty: string; note?: string }) {
  const name = path ? fileName(path) : null;
  const dir = path && name ? path.slice(0, path.length - name.length) : "";
  return (
    <div className="min-w-0">
      <dt className="text-xs text-muted">{label}</dt>
      <dd className="selectable min-w-0" title={path ?? undefined}>
        {path ? (
          <>
            <span className="font-mono text-[0.85rem]">{name}</span>
            {note && <span className="ml-2 text-xs text-faint">({note})</span>}
            <span className="block truncate text-left font-mono text-xs text-faint" dir="rtl">
              <bdi>{dir}</bdi>
            </span>
          </>
        ) : (
          <span className="text-faint">{empty}</span>
        )}
      </dd>
    </div>
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
        variant={hook.installed ? "secondary" : "primary"}
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
    <div className="flex flex-col gap-5">
      <PageHeader
        title="Shells"
        help={<Help label="What is terminal sync?">{HELP.sync}</Help>}
        description={
          <>
            {windows
              ? "Every shell reads your saved variables from your Windows account, so new terminals always get them."
              : "Saved variables are written into your startup files, so new terminals always get them."}{" "}
            Install terminal sync to also update terminals that are already open.
            {sync?.lastSyncedAt && <> Last synced {formatWhen(sync.lastSyncedAt)}.</>}
          </>
        }
      />

      {platform?.configurableTargets && (
        <StartupFiles files={startupFiles} busy={busy} onSave={onSaveStartupFiles} onError={onError} />
      )}

      {shells.length === 0 ? (
        <p className="text-sm text-muted">No shells were found on this computer.</p>
      ) : (
        <ul className="grid gap-3 lg:grid-cols-2">
          {shells.map((s) => (
            <li key={s.kind} className="flex flex-col gap-3 rounded-lg border border-rule bg-panel px-4 py-3.5">
              <div className="flex items-center gap-2.5">
                <TerminalIcon className="shrink-0 text-muted" />
                <span className="font-semibold">{s.name}</span>
                {s.isDefault && (
                  <span className="rounded-full bg-synced/15 px-2 py-0.5 text-xs text-synced">
                    {windows ? "Preferred" : "Login shell"}
                  </span>
                )}
              </div>
              <dl className="flex flex-col gap-2 text-sm">
                <PathField label="Program" path={s.executable} empty="Not found" />
                <PathField
                  label="Startup file"
                  path={s.configPath}
                  empty="None"
                  note={s.configPath && !s.configExists ? "not created yet" : undefined}
                />
              </dl>
              <div className="mt-auto border-t border-rule pt-3">
                <SyncCell hook={hookFor(s.kind)} busy={busy} onSetHook={onSetHook} />
              </div>
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
