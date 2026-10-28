// Explanations shown behind the "?" icons, in one place so wording stays consistent.
import type { ReactNode } from "react";
import { STATUS_META, STATUS_ORDER } from "./filter";

export const HELP: Record<string, ReactNode> = {
  scope: (
    <>
      <p>
        <strong>User</strong> variables are saved for your account. Every terminal and program started from now on gets
        them, also after a restart.
      </p>
      <p>
        <strong>Temporary</strong> variables are never saved. They're pushed into terminals that are already open when you
        sync, and disappear when those terminals close. New terminals and other programs don't get them.
      </p>
    </>
  ),
  status: (
    <>
      <p>Whether the environment that's currently running already has the value:</p>
      <ul className="mt-1.5 space-y-1">
        {STATUS_ORDER.map((s) => (
          <li key={s} className="flex gap-2">
            <span className="mt-[0.45rem] h-2 w-2 shrink-0 rounded-full" style={{ background: STATUS_META[s].color }} />
            <span>
              <strong>{STATUS_META[s].label}:</strong> {STATUS_META[s].help}
            </span>
          </li>
        ))}
      </ul>
    </>
  ),
  sync: (
    <>
      <p>
        Programs read environment variables once, when they start, so terminals that are already open keep the old values.
      </p>
      <p>
        Terminals with terminal sync installed (see Shells) pick up your saved and temporary variables before their next
        prompt after you click this. Other programs need a restart.
      </p>
    </>
  ),
  store: (
    <>
      <p>
        Where your user variables are saved. <span className="font-mono">HKEY_CURRENT_USER\Environment</span> is the part of
        the Windows registry that belongs to your account, so changing it never needs administrator rights.
      </p>
    </>
  ),
  hook: (
    <>
      <p>
        Installing adds a short block, clearly marked, to this shell's profile: the script the shell runs when it starts. A
        backup of the profile is saved first.
      </p>
      <p>
        From then on, each terminal of this shell checks before every prompt whether you've clicked{" "}
        <em>Sync to terminals</em>, and applies the change if so. Remove takes the block out again.
      </p>
    </>
  ),
  original: (
    <p>
      The state from before Environment Manager changed anything. It can't be deleted, so you can always go back to how
      things were.
    </p>
  ),
  backups: (
    <>
      <p>A backup is saved before every change, so any change can be undone.</p>
      <p>
        If a change leads back to a state that is already saved, no duplicate is made. Besides the original, the 50 newest
        backups per file are kept.
      </p>
    </>
  ),
  dotenv: (
    <p>
      A <span className="font-mono">.env</span> file is a plain text list of <span className="font-mono">KEY=value</span>{" "}
      lines. Many tools (Node, Python, Docker Compose) load it from a project folder, so save it in the project that uses
      it.
    </p>
  ),
};
