import { useEffect, useState, type ReactNode } from "react";
import { api, settingsApi, type AppPrefs, type AutostartInfo, type PlatformInfo } from "../lib/api";
import type { ThemePref } from "../lib/theme";
import type { Updates } from "../lib/useUpdates";
import { formatWhen } from "../lib/useEnvironment";
import { Button } from "./IconButton";
import { MonitorIcon, MoonIcon, SunIcon } from "./icons";
import { PageHeader } from "./PageHeader";
import { Switch } from "./Switch";

const REPO = "https://github.com/RamezCh/VARden";

interface Props {
  platform: PlatformInfo | null;
  theme: { pref: ThemePref; choose: (p: ThemePref) => void };
  updates: Updates;
  onError: (message: string) => void;
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h2 className="text-[0.95rem] font-semibold">{title}</h2>
      <div className="divide-y divide-rule overflow-hidden rounded-lg border border-rule bg-panel">{children}</div>
    </section>
  );
}

/** One setting: what it is and what it does on the left, the control on the right. */
function Row({ title, body, control }: { title: string; body: ReactNode; control: ReactNode }) {
  return (
    <div className="flex items-center gap-6 px-4 py-3.5">
      <div className="min-w-0 flex-1">
        <p className="text-sm font-semibold">{title}</p>
        <div className="mt-0.5 text-sm text-muted">{body}</div>
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  );
}

const THEMES: { value: ThemePref; label: string; icon: ReactNode }[] = [
  { value: "system", label: "Match system", icon: <MonitorIcon /> },
  { value: "light", label: "Light", icon: <SunIcon /> },
  { value: "dark", label: "Dark", icon: <MoonIcon /> },
];

/** Opens in the default browser (in a plain browser, a new tab). */
const openLink = async (url: string) => {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener");
  }
};

export function SettingsView({ platform, theme, updates, onError }: Props) {
  const [prefs, setPrefs] = useState<AppPrefs | null>(null);
  const [autostart, setAutostart] = useState<AutostartInfo | null>(null);
  const [version, setVersion] = useState<string>("");

  useEffect(() => {
    settingsApi.prefs().then(setPrefs).catch(onError);
    settingsApi.autostart().then(setAutostart).catch(() => {});
    import("@tauri-apps/api/app")
      .then(({ getVersion }) => getVersion())
      .then(setVersion)
      .catch(() => {});
    // The tray menu can change autostart too.
    let off: (() => void) | undefined;
    import("@tauri-apps/api/event")
      .then(({ listen }) => listen("autostart-changed", () => settingsApi.autostart().then(setAutostart)))
      .then((fn) => (off = fn))
      .catch(() => {});
    return () => off?.();
  }, [onError]);

  const savePrefs = async (next: AppPrefs) => {
    setPrefs(next);
    try {
      setPrefs(await settingsApi.setPrefs(next));
    } catch (e) {
      onError(String(e));
    }
  };

  const toggleAutostart = async (on: boolean) => {
    try {
      setAutostart(await settingsApi.setAutostart(on));
    } catch (e) {
      onError(String(e));
    }
  };

  const s = updates.state;
  const os = platform?.os;
  const trayName = os === "macos" ? "menu bar" : "tray";

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Settings" description="How VARden looks, starts and stays up to date." />

      <Section title="Appearance">
        <Row
          title="Theme"
          body="Match system follows your computer's light or dark setting."
          control={
            <div role="radiogroup" aria-label="Theme" className="flex h-8 items-center rounded-md border border-rule bg-ink/50 p-0.5">
              {THEMES.map((t) => (
                <button
                  key={t.value}
                  type="button"
                  role="radio"
                  aria-checked={theme.pref === t.value}
                  onClick={() => theme.choose(t.value)}
                  className={`flex h-full items-center gap-1.5 rounded px-2.5 text-sm transition-colors ${
                    theme.pref === t.value ? "bg-raised font-semibold text-fg" : "text-muted hover:text-fg"
                  }`}
                >
                  {t.icon}
                  {t.label}
                </button>
              ))}
            </div>
          }
        />
      </Section>

      <Section title="Startup and tray">
        <Row
          title={autostart?.label ?? "Start with your computer"}
          body={
            autostart?.tray === false
              ? "Opens VARden when you log in."
              : `Starts quietly in the ${trayName} when you log in, so Sync to terminals is always one click away.`
          }
          control={
            <Switch
              label={autostart?.label ?? "Start with your computer"}
              checked={autostart?.enabled ?? false}
              disabled={!autostart}
              onChange={(on) => void toggleAutostart(on)}
            />
          }
        />
        <Row
          title={`Keep running in the ${trayName} when closed`}
          body={
            autostart?.tray === false
              ? `Your desktop has no ${trayName}, so closing the window quits VARden.`
              : `Closing the window hides it; click the ${trayName} icon to bring it back, or use Quit in its menu. Turn this off to quit when you close the window.`
          }
          control={
            <Switch
              label={`Keep running in the ${trayName} when closed`}
              checked={prefs?.closeToTray ?? true}
              disabled={!prefs || autostart?.tray === false}
              onChange={(on) => prefs && void savePrefs({ ...prefs, closeToTray: on })}
            />
          }
        />
      </Section>

      <Section title="Updates">
        <Row
          title="Check for updates when VARden starts"
          body="Looks for a new version on GitHub. Nothing is downloaded until you choose to update, and only updates signed by VARden are accepted."
          control={
            <Switch
              label="Check for updates when VARden starts"
              checked={prefs?.checkForUpdates ?? true}
              disabled={!prefs}
              onChange={(on) => prefs && void savePrefs({ ...prefs, checkForUpdates: on })}
            />
          }
        />
        <div className="flex flex-wrap items-center gap-x-6 gap-y-3 px-4 py-3.5">
          <div className="min-w-0 flex-1 text-sm">
            <p className="font-semibold">
              {version ? `You have version ${version}` : "Version"}
            </p>
            <p className="mt-0.5 text-muted" aria-live="polite">
              {s.kind === "idle" && "Check now to see whether a newer version is out."}
              {s.kind === "checking" && "Checking…"}
              {s.kind === "current" && `You're up to date (checked ${formatWhen(s.checkedAt.toISOString())}).`}
              {s.kind === "available" && (
                <>
                  <span className="text-fg">Version {s.update.version} is available.</span> It downloads, installs and restarts
                  VARden; your variables and backups are kept.
                </>
              )}
              {s.kind === "downloading" &&
                `Downloading version ${s.update.version}… ${
                  s.total ? `${Math.round((s.done / s.total) * 100)}%` : `${(s.done / 1_000_000).toFixed(1)} MB`
                }`}
              {s.kind === "ready" && `Version ${s.update.version} is installed. Restart VARden to use it.`}
              {s.kind === "error" && <span className="text-missing">{s.message}</span>}
            </p>
            {s.kind === "available" && s.update.body && (
              <details className="mt-2">
                <summary className="cursor-default text-muted hover:text-fg">What's new</summary>
                <p className="selectable mt-1 whitespace-pre-wrap text-muted">{s.update.body}</p>
              </details>
            )}
          </div>
          <div className="flex shrink-0 gap-2">
            {s.kind === "available" ? (
              <Button variant="primary" onClick={() => void updates.install()}>
                Update to {s.update.version}
              </Button>
            ) : s.kind === "ready" ? (
              <Button variant="primary" onClick={() => void updates.restart()}>
                Restart now
              </Button>
            ) : (
              <Button
                variant="secondary"
                disabled={s.kind === "checking" || s.kind === "downloading"}
                onClick={() => void updates.check()}
              >
                Check now
              </Button>
            )}
          </div>
        </div>
      </Section>

      <Section title="About">
        <Row
          title="VARden"
          body={
            <>
              Manages your own environment variables, never system ones. Backups are kept in{" "}
              <span className="selectable font-mono text-xs">{platform?.backupDir ?? "…"}</span>.
            </>
          }
          control={
            <div className="flex gap-2">
              <Button variant="secondary" onClick={() => void openLink(REPO)}>
                Website
              </Button>
              <Button variant="secondary" onClick={() => void openLink(`${REPO}/issues/new`)}>
                Report a problem
              </Button>
            </div>
          }
        />
        <Row
          title="Diagnostic logs"
          body="Useful when reporting a problem. They list variable names and actions, never values."
          control={
            <Button variant="secondary" onClick={() => api.openLogsFolder().catch((e) => onError(`Couldn't open the log folder: ${String(e)}`))}>
              Open log folder
            </Button>
          }
        />
      </Section>
    </div>
  );
}
