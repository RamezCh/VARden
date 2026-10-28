import { useCallback, useEffect, useRef, useState } from "react";
import { BackupsView } from "./components/BackupsView";
import { ExportModal } from "./components/ExportModal";
import { ImportModal, type ImportSource } from "./components/ImportModal";
import { RefreshIcon } from "./components/icons";
import { IconButton } from "./components/IconButton";
import { ShellsView } from "./components/ShellsView";
import { Toasts } from "./components/Toasts";
import { VariablesView } from "./components/VariablesView";
import { syncApi, type EnvVar, type PlatformInfo } from "./lib/api";
import { pickEnvFile, pickExportPath, useFileDrop } from "./lib/files";
import { formatShort, useEnvironment } from "./lib/useEnvironment";
import { useToasts } from "./lib/useToasts";

type View = "variables" | "shells" | "backups";

function storeDescription(p: PlatformInfo | null): string {
  if (!p) return "";
  return p.stores
    .map((s) => (s.type === "registry" ? s.location : s.path))
    .join("\n");
}

const isTyping = (el: EventTarget | null) =>
  el instanceof HTMLElement && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName));

export default function App() {
  const { toasts, push, dismiss } = useToasts();
  const [view, setView] = useState<View>("variables");
  const openBackups = useCallback(() => setView("backups"), []);
  const env = useEnvironment(push, openBackups);
  const [creating, setCreating] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  // .env import: null = closed, "picking" = modal open without a file yet.
  const [importing, setImporting] = useState<ImportSource | "picking" | null>(null);
  const [exporting, setExporting] = useState<EnvVar[] | null>(null);
  const [dropHover, setDropHover] = useState(false);

  const loadEnvFile = useCallback(
    async (path: string) => {
      try {
        setImporting({ path, parsed: await syncApi.readEnvFile(path) });
      } catch (e) {
        push("error", `Couldn't read ${path}: ${String(e)}`);
      }
    },
    [push],
  );
  const chooseEnvFile = useCallback(async () => {
    try {
      const path = await pickEnvFile();
      if (path) await loadEnvFile(path);
    } catch (e) {
      push("error", String(e));
    }
  }, [loadEnvFile, push]);

  // Dropping a file anywhere on the window opens it for import.
  useFileDrop((path) => void loadEnvFile(path), setDropHover);

  const hooksInstalled = env.sync?.hooks.filter((h) => h.installed).length ?? 0;

  // "/" focuses search, "N" starts a new variable.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey || isTyping(e.target)) return;
      if (e.key === "/") {
        e.preventDefault();
        setView("variables");
        window.setTimeout(() => searchRef.current?.focus());
      } else if (e.key.toLowerCase() === "n") {
        e.preventDefault();
        setView("variables");
        setCreating(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const nav: { id: View; label: string; count: number }[] = [
    { id: "variables", label: "Variables", count: env.vars.length },
    { id: "shells", label: "Shells", count: env.shells.length },
    { id: "backups", label: "Backups", count: env.backups.length },
  ];

  return (
    <div className="flex h-screen min-h-0">
      <aside className="flex w-56 shrink-0 flex-col border-r border-rule bg-panel/60 px-3 py-5">
        <div className="flex items-center gap-2.5 px-2">
          <svg width="22" height="22" viewBox="0 0 22 22" aria-hidden>
            <rect x="1" y="1" width="20" height="20" rx="6" fill="var(--color-raised)" stroke="var(--color-rule)" />
            <circle cx="7" cy="14" r="2" fill="var(--color-synced)" />
            <circle cx="11" cy="14" r="2" fill="var(--color-persistent)" />
            <circle cx="15" cy="14" r="2" fill="var(--color-process)" />
          </svg>
          <span className="font-semibold leading-tight">Environment Manager</span>
        </div>

        <nav aria-label="Sections" className="mt-7 flex flex-col gap-0.5">
          {nav.map((n) => (
            <button
              key={n.id}
              type="button"
              aria-current={view === n.id ? "page" : undefined}
              onClick={() => setView(n.id)}
              className={`flex items-center justify-between rounded-md px-2.5 py-2 text-left text-[0.95rem] transition-colors ${
                view === n.id ? "bg-raised text-fg" : "text-muted hover:bg-raised/60 hover:text-fg"
              }`}
            >
              {n.label}
              <span className="font-mono text-xs tabular-nums text-faint">{n.count}</span>
            </button>
          ))}
        </nav>

        <div className="mt-auto flex flex-col gap-2 px-2">
          <p className="text-xs text-muted">Saving to</p>
          <p className="whitespace-pre-line font-mono text-xs leading-relaxed text-fg/80">
            {/* Allow line breaks only after path separators (zero-width space). */}
            {storeDescription(env.platform).replace(/([\\/])/g, "$1\u200B")}
          </p>
          <div className="mt-1 flex items-center justify-between gap-2">
            <span className="text-xs text-faint">
              {env.loading
                ? "Reading…"
                : env.sync?.lastSyncedAt
                  ? `Terminals synced ${formatShort(env.sync.lastSyncedAt)}`
                  : "Up to date"}
            </span>
            <IconButton label="Reload from disk" onClick={() => void env.refresh()} disabled={env.loading}>
              <RefreshIcon />
            </IconButton>
          </div>
        </div>
      </aside>

      <main className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-[1180px] px-8 py-7">
          {view === "variables" && (
            <VariablesView
              vars={env.vars}
              loading={env.loading}
              busy={env.busy}
              creating={creating}
              setCreating={setCreating}
              searchRef={searchRef}
              onSet={env.setVariable}
              onDelete={env.deleteVariable}
              onOpenImport={() => setImporting("picking")}
              onOpenExport={setExporting}
              hooksInstalled={hooksInstalled}
              onSync={() => void env.syncTerminals()}
              onSetUpSync={() => setView("shells")}
            />
          )}
          {view === "shells" && (
            <ShellsView
              shells={env.shells}
              platform={env.platform}
              sync={env.sync}
              busy={env.busy}
              onSetHook={(kind, name, install) => void env.setHook(kind, name, install)}
            />
          )}
          {view === "backups" && (
            <BackupsView backups={env.backups} platform={env.platform} busy={env.busy} onRestore={env.restoreBackup} />
          )}
        </div>
      </main>

      {importing && (
        <ImportModal
          source={importing === "picking" ? null : importing}
          vars={env.vars}
          caseInsensitive={env.platform?.os === "windows"}
          busy={env.busy}
          onChooseFile={() => void chooseEnvFile()}
          onClose={() => setImporting(null)}
          onImport={env.importVariables}
        />
      )}
      {exporting && (
        <ExportModal
          vars={env.vars}
          initial={exporting}
          busy={env.busy}
          onClose={() => setExporting(null)}
          onExport={async (items) => {
            const path = await pickExportPath();
            return path ? env.exportVariables(path, items) : false;
          }}
        />
      )}

      {dropHover && (
        <div className="pointer-events-none fixed inset-3 z-40 grid place-items-center rounded-xl border-2 border-dashed border-process bg-ink/80">
          <p className="text-lg font-semibold">Drop a .env file to import it</p>
        </div>
      )}

      <Toasts toasts={toasts} onDismiss={dismiss} />
    </div>
  );
}
