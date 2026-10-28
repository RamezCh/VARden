import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { BackupsView } from "./components/BackupsView";
import { ExportModal } from "./components/ExportModal";
import { ImportModal, type ImportSource } from "./components/ImportModal";
import { BackupIcon, ProfilesIcon, RefreshIcon, TerminalIcon, VariablesIcon } from "./components/icons";
import { IconButton } from "./components/IconButton";
import { Help } from "./components/Help";
import { HELP } from "./lib/helpText";
import { ShellsView } from "./components/ShellsView";
import { Toasts } from "./components/Toasts";
import { VariablesView } from "./components/VariablesView";
import { api, syncApi, type EnvVar, type PlatformInfo, type Profile } from "./lib/api";
import { ProfileEditor } from "./components/ProfileEditor";
import { PathEditor } from "./components/PathEditor";
import { ProfilesView } from "./components/ProfilesView";
import { fileName, pickEnvFile, pickExportPath, useFileDrop } from "./lib/files";
import { useTheme } from "./lib/theme";
import { ThemeSwitch } from "./components/ThemeSwitch";
import { formatShort, useEnvironment } from "./lib/useEnvironment";
import { useToasts } from "./lib/useToasts";

type View = "variables" | "profiles" | "shells" | "backups";

/** Short, human name for where variables are saved; the full path is the tooltip. */
function storeShort(p: PlatformInfo | null): string {
  if (!p) return "…";
  return p.stores.map((s) => (s.type === "registry" ? "Your Windows account" : fileName(s.path))).join(", ");
}

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
  const theme = useTheme();
  const [creating, setCreating] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  // .env import: null = closed, "picking" = modal open without a file yet.
  const [importing, setImporting] = useState<ImportSource | "picking" | null>(null);
  const [exporting, setExporting] = useState<EnvVar[] | null>(null);
  const [editingProfile, setEditingProfile] = useState<Profile | "new" | null>(null);
  const [dropHover, setDropHover] = useState(false);
  const [editingPath, setEditingPath] = useState(false);

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

  // Desktop shortcuts: Ctrl+1–4 switch pages, Ctrl+F or "/" searches,
  // Ctrl+N or "N" adds a variable, F5 re-reads everything from disk.
  const refreshRef = useRef(env.refresh);
  refreshRef.current = env.refresh;
  useEffect(() => {
    const views: View[] = ["variables", "profiles", "shells", "backups"];
    const search = () => {
      setView("variables");
      window.setTimeout(() => searchRef.current?.focus());
    };
    const create = () => {
      setView("variables");
      setCreating(true);
    };
    const onKey = (e: KeyboardEvent) => {
      if (document.querySelector("dialog[open]")) return;
      const mod = e.ctrlKey || e.metaKey;
      if (e.key === "F5") {
        e.preventDefault();
        void refreshRef.current();
        return;
      }
      if (mod && !e.altKey && !e.shiftKey) {
        const n = Number(e.key);
        if (n >= 1 && n <= views.length) {
          e.preventDefault();
          setView(views[n - 1]);
        } else if (e.key.toLowerCase() === "f") {
          e.preventDefault();
          search();
        } else if (e.key.toLowerCase() === "n") {
          e.preventDefault();
          create();
        }
        return;
      }
      if (e.ctrlKey || e.metaKey || e.altKey || isTyping(e.target)) return;
      if (e.key === "/") {
        e.preventDefault();
        search();
      } else if (e.key.toLowerCase() === "n") {
        e.preventDefault();
        create();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const nav: { id: View; label: string; count: number; icon: ReactNode; shortcut: string }[] = [
    { id: "variables", label: "Variables", count: env.vars.length, icon: <VariablesIcon />, shortcut: "1" },
    { id: "profiles", label: "Profiles", count: env.profiles.profiles.length, icon: <ProfilesIcon />, shortcut: "2" },
    { id: "shells", label: "Shells", count: env.shells.length, icon: <TerminalIcon />, shortcut: "3" },
    { id: "backups", label: "Backups", count: env.backups.length, icon: <BackupIcon />, shortcut: "4" },
  ];

  return (
    <div className="flex h-screen min-h-0">
      <aside className="flex w-56 shrink-0 flex-col border-r border-rule bg-panel/70 px-3 pb-4 pt-5">
        <div className="flex items-center gap-2.5 px-2.5">
          <svg width="24" height="24" viewBox="0 0 22 22" aria-hidden>
            <rect x="1" y="1" width="20" height="20" rx="6" fill="var(--color-raised)" stroke="var(--color-rule)" />
            <circle cx="7" cy="14" r="2" fill="var(--color-synced)" />
            <circle cx="11" cy="14" r="2" fill="var(--color-persistent)" />
            <circle cx="15" cy="14" r="2" fill="var(--color-process)" />
          </svg>
          <span className="text-lg font-semibold leading-tight tracking-tight">VARden</span>
        </div>

        <nav aria-label="Sections" className="mt-6 flex flex-col gap-0.5">
          {nav.map((n) => {
            const current = view === n.id;
            return (
              <button
                key={n.id}
                type="button"
                aria-current={current ? "page" : undefined}
                onClick={() => setView(n.id)}
                title={`${n.label} (Ctrl+${n.shortcut})`}
                className={`relative flex items-center gap-2.5 rounded-md px-2.5 py-[0.45rem] text-left text-[0.95rem] transition-colors ${
                  current ? "bg-raised font-semibold text-fg" : "text-muted hover:bg-raised/60 hover:text-fg"
                }`}
              >
                {current && <span className="absolute inset-y-1.5 left-0 w-[3px] rounded-full bg-process" aria-hidden />}
                <span className={current ? "text-process" : ""}>{n.icon}</span>
                <span className="flex-1">{n.label}</span>
                <span className="font-mono text-xs font-normal tabular-nums text-faint">{n.count}</span>
              </button>
            );
          })}
        </nav>

        <div className="mt-auto flex flex-col gap-3">
          <div className="rounded-lg border border-rule bg-ink/50 px-3 py-2.5">
            <p className="flex items-center gap-1.5 text-xs text-muted">
              Saving to <Help label="Where are variables saved?">{HELP.store}</Help>
            </p>
            <p className="mt-0.5 truncate text-sm" title={storeDescription(env.platform)}>
              {storeShort(env.platform)}
            </p>
            <div className="mt-2 flex items-center justify-between gap-2 border-t border-rule pt-2">
              <span className="truncate text-xs text-muted">
                {env.loading
                  ? "Reading…"
                  : env.sync?.lastSyncedAt
                    ? `Terminals synced ${formatShort(env.sync.lastSyncedAt)}`
                    : "Up to date"}
              </span>
              <IconButton label="Reload from disk (F5)" onClick={() => void env.refresh()} disabled={env.loading}>
                <RefreshIcon />
              </IconButton>
            </div>
          </div>
          <ThemeSwitch pref={theme.pref} onChange={theme.choose} />
          <button
            type="button"
            className="self-start px-1 text-xs text-faint underline-offset-4 hover:text-fg hover:underline"
            title="Diagnostic logs, useful when reporting a problem"
            onClick={() => api.openLogsFolder().catch((e) => push("error", `Couldn't open the log folder: ${String(e)}`))}
          >
            Open log folder
          </button>
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
              onDeleteMany={env.deleteVariables}
              onRename={env.renameVariable}
              onEditPath={() => setEditingPath(true)}
              caseInsensitive={env.platform?.os === "windows"}
              onOpenImport={() => setImporting("picking")}
              onOpenExport={setExporting}
              hooksInstalled={hooksInstalled}
              onSync={() => void env.syncTerminals()}
              onSetUpSync={() => setView("shells")}
              activeProfile={env.profiles.active}
              onOpenProfiles={() => setView("profiles")}
            />
          )}
          {view === "profiles" && (
            <ProfilesView
              data={env.profiles}
              busy={env.busy}
              onNew={() => setEditingProfile("new")}
              onEdit={setEditingProfile}
              onDelete={env.deleteProfile}
              onApply={(p, scope) => void env.applyProfile(p, scope)}
              onTurnOff={(name) => void env.deactivateProfile(name)}
            />
          )}
          {view === "shells" && (
            <ShellsView
              shells={env.shells}
              platform={env.platform}
              sync={env.sync}
              busy={env.busy}
              onSetHook={(kind, name, install) => void env.setHook(kind, name, install)}
              startupFiles={env.startupFiles}
              onSaveStartupFiles={env.saveStartupFiles}
              onError={(m) => push("error", m)}
            />
          )}
          {view === "backups" && (
            <BackupsView
              backups={env.backups}
              platform={env.platform}
              busy={env.busy}
              onRestore={env.restoreBackup}
              onDelete={env.deleteBackups}
            />
          )}
        </div>
      </main>

      {editingProfile && (
        <ProfileEditor
          profile={editingProfile === "new" ? null : editingProfile}
          vars={env.vars}
          caseInsensitive={env.platform?.os === "windows"}
          busy={env.busy}
          onClose={() => setEditingProfile(null)}
          onSave={env.saveProfile}
          onError={(m) => push("error", m)}
        />
      )}

      {editingPath && (
        <PathEditor
          windows={env.platform?.os === "windows"}
          busy={env.busy}
          onClose={() => setEditingPath(false)}
          onSave={env.saveUserPath}
        />
      )}

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
          onExport={async (items, dir) => {
            const path = await pickExportPath(dir);
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
