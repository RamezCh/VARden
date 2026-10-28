import { useEffect, useRef, useState } from "react";
import { BackupsView } from "./components/BackupsView";
import { RefreshIcon } from "./components/icons";
import { IconButton } from "./components/IconButton";
import { ShellsView } from "./components/ShellsView";
import { Toasts } from "./components/Toasts";
import { VariablesView } from "./components/VariablesView";
import type { PlatformInfo } from "./lib/api";
import { useEnvironment } from "./lib/useEnvironment";
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
  const env = useEnvironment(push);
  const [view, setView] = useState<View>("variables");
  const [creating, setCreating] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

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
          <div className="mt-1 flex items-center justify-between">
            <span className="text-xs text-faint">{env.loading ? "Reading…" : "Up to date"}</span>
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
            />
          )}
          {view === "shells" && <ShellsView shells={env.shells} platform={env.platform} />}
          {view === "backups" && (
            <BackupsView backups={env.backups} platform={env.platform} busy={env.busy} onRestore={env.restoreBackup} />
          )}
        </div>
      </main>

      <Toasts toasts={toasts} onDismiss={dismiss} />
    </div>
  );
}
