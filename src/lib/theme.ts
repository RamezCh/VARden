// Light/dark theme. "system" follows the OS through CSS alone; a manual
// choice sets data-theme on <html> and themes the native title bar too.
import { useCallback, useEffect, useState } from "react";

export type ThemePref = "system" | "light" | "dark";
const KEY = "varden.theme";

function load(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(pref: ThemePref) {
  const root = document.documentElement;
  if (pref === "system") delete root.dataset.theme;
  else root.dataset.theme = pref;
  import("@tauri-apps/api/window")
    .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(pref === "system" ? null : pref))
    .catch(() => {
      /* not running inside Tauri */
    });
}

/** Apply the saved choice before React renders, so there's no flash. */
export function initTheme() {
  const pref = load();
  if (pref !== "system") document.documentElement.dataset.theme = pref;
}

export function useTheme() {
  const [pref, setPref] = useState<ThemePref>(load);
  useEffect(() => applyTheme(pref), [pref]);
  const choose = useCallback((p: ThemePref) => {
    setPref(p);
    try {
      if (p === "system") localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, p);
    } catch {
      /* storage unavailable: the choice lasts for this session */
    }
  }, []);
  return { pref, choose };
}
