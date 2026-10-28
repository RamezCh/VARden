import type { DetectedShell, PlatformInfo } from "../lib/api";

export function ShellsView({ shells, platform }: { shells: DetectedShell[]; platform: PlatformInfo | null }) {
  const windows = platform?.os === "windows";
  return (
    <div className="flex max-w-3xl flex-col gap-5">
      <div>
        <h1 className="text-xl font-semibold">Shells</h1>
        <p className="mt-1 max-w-[68ch] text-sm text-muted">
          {windows
            ? "Every shell below reads your user environment from the registry, so variables saved here reach them without editing profile files. Terminals that are already open need a restart to see changes."
            : "Variables are written into a managed block in the profile file of the shells you choose. Everything outside that block is left untouched."}
        </p>
      </div>

      {shells.length === 0 ? (
        <p className="text-sm text-muted">No shells were found on this computer.</p>
      ) : (
        <ul className="divide-y divide-rule overflow-hidden rounded-lg border border-rule bg-panel">
          {shells.map((s) => (
            <li key={s.kind} className="flex flex-col gap-1 px-4 py-3">
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
                  {s.configPath ?? <span className="font-sans text-faint">None, reads the registry only</span>}
                  {s.configPath && !s.configExists && <span className="ml-2 font-sans text-faint">(not created yet)</span>}
                </dd>
              </dl>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
