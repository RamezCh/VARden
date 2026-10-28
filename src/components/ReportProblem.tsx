import { useEffect, useState } from "react";
import { api, type PlatformInfo } from "../lib/api";
import { Button } from "./IconButton";
import { Modal } from "./Modal";

const REPO = "https://github.com/RamezCh/VARden";

const OS_NAME: Record<string, string> = { windows: "Windows", macos: "macOS", linux: "Linux" };

/** The new-issue page with version and OS already filled in. */
export function issueUrl(version: string, os: string | undefined): string {
  const params = new URLSearchParams({ template: "bug_report.yml", version, os: os ? (OS_NAME[os] ?? os) : "" });
  return `${REPO}/issues/new?${params}`;
}

export async function openExternal(url: string) {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener");
  }
}

/**
 * Reports go to GitHub as issues. The log file is attached by the user, not
 * uploaded by the app: it never sends anything on its own, and the user can
 * look at the log before sharing it.
 */
export function ReportProblem({
  platform,
  onClose,
  onError,
}: {
  platform: PlatformInfo | null;
  onClose: () => void;
  onError: (m: string) => void;
}) {
  const [version, setVersion] = useState("");
  const [openedLogs, setOpenedLogs] = useState(false);
  useEffect(() => {
    import("@tauri-apps/api/app")
      .then(({ getVersion }) => getVersion())
      .then(setVersion)
      .catch(() => {});
  }, []);

  const openLogs = () =>
    api
      .openLogsFolder()
      .then(() => setOpenedLogs(true))
      .catch((e) => onError(`Couldn't open the log folder: ${String(e)}`));

  return (
    <Modal
      open
      onClose={onClose}
      title="Report a problem"
      description="Problems are tracked on GitHub, where you can follow the fix. You need a free GitHub account."
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            onClick={() => {
              void openExternal(issueUrl(version, platform?.os));
              onClose();
            }}
          >
            Open the report on GitHub
          </Button>
        </>
      }
    >
      <ol className="flex flex-col gap-4 text-sm">
        <li className="flex gap-3">
          <Step n={1} />
          <div className="min-w-0 flex-1">
            <p className="font-semibold">Find the log file</p>
            <p className="mt-0.5 text-muted">
              It shows what VARden did just before the problem. It lists variable <em>names</em> and actions, never values;
              if some names are private, delete those lines before sharing.
            </p>
            <Button variant="secondary" className="mt-2" onClick={() => void openLogs()}>
              Open log folder
            </Button>
            {openedLogs && <p className="mt-1.5 text-xs text-muted">The newest file is <span className="font-mono">varden.log</span>.</p>}
          </div>
        </li>
        <li className="flex gap-3">
          <Step n={2} />
          <div className="min-w-0 flex-1">
            <p className="font-semibold">Describe what happened</p>
            <p className="mt-0.5 text-muted">
              The form opens with your VARden version{version ? ` (${version})` : ""} and system already filled in. Say what
              you did and what you expected, and drag <span className="font-mono">varden.log</span> into the Log file box.
            </p>
          </div>
        </li>
        <li className="flex gap-3">
          <Step n={3} />
          <div className="min-w-0 flex-1">
            <p className="font-semibold">Never paste secret values</p>
            <p className="mt-0.5 text-muted">Issues on GitHub are public. Replace keys and passwords with something like <span className="font-mono">sk-…</span> instead.</p>
          </div>
        </li>
      </ol>
    </Modal>
  );
}

function Step({ n }: { n: number }) {
  return (
    <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full border border-rule font-mono text-xs text-muted" aria-hidden>
      {n}
    </span>
  );
}
