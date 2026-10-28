import type { ThemePref } from "../lib/theme";
import { MonitorIcon, MoonIcon, SunIcon } from "./icons";

const OPTIONS: { value: ThemePref; label: string; icon: React.ReactNode }[] = [
  { value: "system", label: "Match system", icon: <MonitorIcon /> },
  { value: "light", label: "Light", icon: <SunIcon /> },
  { value: "dark", label: "Dark", icon: <MoonIcon /> },
];

export function ThemeSwitch({ pref, onChange }: { pref: ThemePref; onChange: (p: ThemePref) => void }) {
  return (
    <div className="flex items-center justify-between gap-2 pl-2.5">
      <span className="text-xs text-muted">Theme</span>
      <div role="radiogroup" aria-label="Theme" className="flex rounded-md border border-rule bg-ink/50 p-0.5">
        {OPTIONS.map((o) => (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={pref === o.value}
            aria-label={o.label}
            title={o.label}
            onClick={() => onChange(o.value)}
            className={`grid h-6 w-7 place-items-center rounded transition-colors ${
              pref === o.value ? "bg-raised text-fg" : "text-faint hover:text-fg"
            }`}
          >
            {o.icon}
          </button>
        ))}
      </div>
    </div>
  );
}
