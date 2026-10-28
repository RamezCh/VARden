// Phase 1 smoke-test UI: exercises every backend command end to end.
// The full dashboard (sorting, filters, modals) is Phase 2.
import { useCallback, useEffect, useState } from "react";
import {
  api,
  type BackupEntry,
  type DetectedShell,
  type EnvVar,
  type MutationResult,
  type PlatformInfo,
  type Scope,
  type Status,
} from "./lib/api";

const STATUS: Record<Status, { label: string; dot: string; text: string }> = {
  activeSynced: { label: "Active & Synced", dot: "bg-emerald-400", text: "text-emerald-300" },
  persistentOnly: { label: "Persistent Only", dot: "bg-amber-400", text: "text-amber-300" },
  processOnly: { label: "Process Only", dot: "bg-sky-400", text: "text-sky-300" },
  missingStale: { label: "Missing / Stale", dot: "bg-rose-400", text: "text-rose-300" },
};

function StatusPill({ status }: { status: Status }) {
  const s = STATUS[status];
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full bg-zinc-800/80 px-2.5 py-0.5 text-xs ${s.text}`}>
      <span className={`h-1.5 w-1.5 rounded-full ${s.dot}`} />
      {s.label}
    </span>
  );
}

function storeLabel(info: PlatformInfo | null) {
  if (!info) return "…";
  return info.stores
    .map((s) => (s.type === "registry" ? s.location : `${s.path} (${s.syntax})`))
    .join(", ");
}

export default function App() {
  const [info, setInfo] = useState<PlatformInfo | null>(null);
  const [vars, setVars] = useState<EnvVar[]>([]);
  const [shells, setShells] = useState<DetectedShell[]>([]);
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [revealed, setRevealed] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [form, setForm] = useState({ key: "", value: "", scope: "user" as Scope });

  const run = useCallback(async (fn: () => Promise<void>) => {
    setError(null);
    setNotice(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const refresh = useCallback(
    () =>
      run(async () => {
        const [i, v, s, b] = await Promise.all([
          api.platformInfo(),
          api.listVariables(),
          api.detectShells(),
          api.listBackups(),
        ]);
        setInfo(i);
        setVars(v);
        setShells(s);
        setBackups(b);
      }),
    [run],
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const afterMutation = async ({ variables, backups: taken }: MutationResult) => {
    setVars(variables);
    const reused = taken.find((b) => b.reused);
    if (reused) {
      setNotice(
        `No new backup was created: this exact state is already saved in the backup from ${new Date(
          reused.createdAt,
        ).toLocaleString()}${reused.original ? " (original)" : ""}.`,
      );
    }
    setBackups(await api.listBackups());
  };

  const toggleReveal = (key: string) =>
    setRevealed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-6 p-6">
      <header className="flex items-end justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">Environment Manager</h1>
          <p className="mt-1 text-sm text-zinc-400">
            {info?.os ?? "…"} · user store: <span className="font-mono text-zinc-300">{storeLabel(info)}</span>
          </p>
        </div>
        <button
          onClick={() => void refresh()}
          className="rounded-md border border-zinc-700 px-3 py-1.5 text-sm hover:bg-zinc-800"
        >
          Refresh
        </button>
      </header>

      {error && (
        <div className="rounded-md border border-rose-900 bg-rose-950/50 px-4 py-2 text-sm text-rose-200">{error}</div>
      )}

      {notice && (
        <div className="rounded-md border border-sky-900 bg-sky-950/50 px-4 py-2 text-sm text-sky-200">{notice}</div>
      )}

      <form
        className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900 p-3"
        onSubmit={(e) => {
          e.preventDefault();
          void run(async () => {
            const r = await api.setVariable(form.key, form.value, form.scope);
            await afterMutation(r);
            setForm({ ...form, key: "", value: "" });
          });
        }}
      >
        <input
          className="w-56 rounded-md bg-zinc-950 px-3 py-1.5 font-mono text-sm outline-none ring-1 ring-zinc-800 focus:ring-zinc-600"
          placeholder="KEY"
          value={form.key}
          onChange={(e) => setForm({ ...form, key: e.target.value })}
        />
        <input
          className="min-w-64 flex-1 rounded-md bg-zinc-950 px-3 py-1.5 font-mono text-sm outline-none ring-1 ring-zinc-800 focus:ring-zinc-600"
          placeholder="value"
          value={form.value}
          onChange={(e) => setForm({ ...form, value: e.target.value })}
        />
        <select
          className="rounded-md bg-zinc-950 px-2 py-1.5 text-sm ring-1 ring-zinc-800"
          value={form.scope}
          onChange={(e) => setForm({ ...form, scope: e.target.value as Scope })}
        >
          <option value="user">User (persistent)</option>
          <option value="process">Process (this session)</option>
        </select>
        <button className="rounded-md bg-zinc-100 px-3 py-1.5 text-sm font-medium text-zinc-900 hover:bg-white">
          Save
        </button>
      </form>

      <section className="overflow-hidden rounded-lg border border-zinc-800">
        <table className="w-full table-fixed text-sm">
          <thead className="bg-zinc-900 text-left text-xs uppercase tracking-wide text-zinc-500">
            <tr>
              <th className="w-1/4 px-4 py-2">Key</th>
              <th className="px-4 py-2">Value</th>
              <th className="w-24 px-4 py-2">Scope</th>
              <th className="w-40 px-4 py-2">Status</th>
              <th className="w-20 px-4 py-2" />
            </tr>
          </thead>
          <tbody className="divide-y divide-zinc-800/70">
            {vars.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-8 text-center text-zinc-500">
                  No user-level or session variables yet.
                </td>
              </tr>
            )}
            {vars.map((v) => {
              const masked = v.sensitive && !revealed.has(v.key);
              return (
                <tr key={`${v.scope}:${v.key}`} className="hover:bg-zinc-900/60">
                  <td className="truncate px-4 py-2 font-mono">{v.key}</td>
                  <td className="px-4 py-2 font-mono text-zinc-300">
                    <div className="flex items-center gap-2">
                      <span className="truncate" title={masked ? undefined : v.value}>
                        {masked ? "••••••••••••" : v.value || <em className="text-zinc-600">empty</em>}
                      </span>
                      {v.sensitive && (
                        <button
                          type="button"
                          onClick={() => toggleReveal(v.key)}
                          className="shrink-0 text-xs text-zinc-500 hover:text-zinc-300"
                        >
                          {masked ? "show" : "hide"}
                        </button>
                      )}
                    </div>
                  </td>
                  <td className="px-4 py-2 text-zinc-400">{v.scope}</td>
                  <td className="px-4 py-2">
                    <StatusPill status={v.status} />
                  </td>
                  <td className="px-4 py-2 text-right">
                    <button
                      className="text-xs text-zinc-500 hover:text-rose-300"
                      onClick={() =>
                        void run(async () => {
                          const r = await api.deleteVariable(v.key, v.scope);
                          await afterMutation(r);
                        })
                      }
                    >
                      delete
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </section>

      <div className="grid gap-6 md:grid-cols-2">
        <section className="rounded-lg border border-zinc-800 p-4">
          <h2 className="mb-3 text-sm font-semibold text-zinc-300">Detected shells</h2>
          <ul className="space-y-2 text-sm">
            {shells.map((s) => (
              <li key={s.kind} className="flex flex-col">
                <span>
                  {s.name} {s.isDefault && <span className="text-xs text-emerald-400">default</span>}
                </span>
                <span className="truncate font-mono text-xs text-zinc-500">
                  {s.configPath ?? "no profile file"} {s.configPath && !s.configExists && "(not created yet)"}
                </span>
              </li>
            ))}
          </ul>
        </section>

        <section className="rounded-lg border border-zinc-800 p-4">
          <h2 className="mb-3 text-sm font-semibold text-zinc-300">Backups</h2>
          <ul className="max-h-64 space-y-2 overflow-auto text-sm">
            {backups.length === 0 && <li className="text-zinc-500">Nothing modified yet.</li>}
            {backups.map((b) => (
              <li key={b.id} className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate">
                    {b.reason}{" "}
                    {b.original && <span className="text-xs text-emerald-400">original · kept forever</span>}
                  </div>
                  <div className="truncate font-mono text-xs text-zinc-500">
                    {new Date(b.createdAt).toLocaleString()} ·{" "}
                    {b.target.type === "registry" ? "registry" : b.target.path}
                  </div>
                </div>
                <button
                  className="shrink-0 rounded border border-zinc-700 px-2 py-0.5 text-xs hover:bg-zinc-800"
                  onClick={() =>
                    void run(async () => {
                      const r = await api.restoreBackup(b.id);
                      await afterMutation(r);
                    })
                  }
                >
                  Restore
                </button>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </div>
  );
}
