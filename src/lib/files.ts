// Native file pickers and window-level file drops (Tauri only).
import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef } from "react";

const FILTERS = [
  { name: "Environment files", extensions: ["env"] },
  { name: "All files", extensions: ["*"] },
];

export async function pickEnvFile(): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false, filters: FILTERS, title: "Import a .env file" });
  return typeof picked === "string" ? picked : null;
}

/** Any file, e.g. a shell startup file kept in a dotfiles folder. */
export async function pickStartupFile(): Promise<string | null> {
  const picked = await open({ multiple: false, directory: false, title: "Choose a shell startup file" });
  return typeof picked === "string" ? picked : null;
}

/** Join a folder and a file name with the folder's own separator. */
export const joinPath = (dir: string, name: string) => {
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
};

/** Opens the save dialog, starting in `dir` when given. */
export async function pickExportPath(dir?: string): Promise<string | null> {
  return save({ defaultPath: dir ? joinPath(dir, ".env") : ".env", filters: FILTERS, title: "Export variables" });
}

export async function pickFolder(title: string): Promise<string | null> {
  const picked = await open({ multiple: false, directory: true, title });
  return typeof picked === "string" ? picked : null;
}

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/**
 * Calls `onDrop` with the first file dropped anywhere on the window.
 * Tauri delivers OS file drops as paths; outside Tauri this does nothing.
 */
export function useFileDrop(onDrop: (path: string) => void, onHover?: (hovering: boolean) => void) {
  const latest = useRef({ onDrop, onHover });
  latest.current = { onDrop, onHover };

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    import("@tauri-apps/api/webview")
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") latest.current.onHover?.(true);
          else if (p.type === "leave") latest.current.onHover?.(false);
          else if (p.type === "drop") {
            latest.current.onHover?.(false);
            if (p.paths[0]) latest.current.onDrop(p.paths[0]);
          }
        }),
      )
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {
        /* not running inside Tauri */
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}
