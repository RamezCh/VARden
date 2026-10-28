// Checking for, downloading and installing new versions from GitHub Releases.
// Updates are signed: the app refuses any download whose signature doesn't
// match the public key built into it.
import type { Update } from "@tauri-apps/plugin-updater";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "current"; checkedAt: Date }
  | { kind: "available"; update: Update }
  | { kind: "downloading"; update: Update; done: number; total: number | null }
  | { kind: "ready"; update: Update }
  | { kind: "error"; message: string };

export async function checkForUpdate(): Promise<Update | null> {
  const { check } = await import("@tauri-apps/plugin-updater");
  return check();
}

/** Download and install; reports progress in bytes. */
export async function installUpdate(update: Update, onProgress: (done: number, total: number | null) => void) {
  let done = 0;
  let total: number | null = null;
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") total = event.data.contentLength ?? null;
    else if (event.event === "Progress") done += event.data.chunkLength;
    onProgress(done, total);
  });
}

export async function restartApp() {
  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}

/** Errors from a build without an update key, or offline, in plain words. */
export function describeUpdateError(e: unknown): string {
  const text = String(e);
  if (/pubkey|public key/i.test(text)) return "This build can't check for updates (it wasn't built with an update key).";
  if (/network|dns|connect|timed out|offline|error sending request/i.test(text)) return "Couldn't reach GitHub. Check your internet connection.";
  if (/404|not found|successful status code/i.test(text)) return "No published release was found yet.";
  return `Couldn't check for updates: ${text}`;
}
