import { useCallback, useEffect, useState } from "react";
import { reportApi, type PlatformInfo, type Report } from "../lib/api";
import { pickReportPath } from "../lib/files";
import { copyText } from "../lib/clipboard";
import { plural } from "../lib/useEnvironment";
import { Button } from "./IconButton";
import { CheckIcon, CopyIcon } from "./icons";
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
 * Reports go to GitHub as issues. VARden prepares a cleaned copy of its log
 * (variable names, your user name and anything that looks like a secret
 * replaced), shows it, and only lets you copy or save it after you confirm
 * you've read it. Nothing is ever uploaded by the app.
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
  const [report, setReport] = useState<Report | null>(null);
  const [keepNames, setKeepNames] = useState(false);
  const [reviewed, setReviewed] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    import("@tauri-apps/api/app")
      .then(({ getVersion }) => getVersion())
      .then(setVersion)
      .catch(() => {});
  }, []);

  const prepare = useCallback(
    async (keep: boolean) => {
      try {
        setReport(await reportApi.prepare(keep));
        // Anything new on screen needs a fresh look.
        setReviewed(false);
      } catch (e) {
        onError(`Couldn't read the log: ${String(e)}`);
      }
    },
    [onError],
  );

  useEffect(() => {
    void prepare(keepNames);
  }, [prepare, keepNames]);

  const copy = async () => {
    if (!report) return;
    await copyText(report.text, false);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };

  const saveFile = async () => {
    if (!report) return;
    try {
      const path = await pickReportPath();
      if (path) await reportApi.save(path, report.text);
    } catch (e) {
      onError(String(e));
    }
  };

  const empty = report !== null && report.lines === 0;

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
      <ol className="flex flex-col gap-5 text-sm">
        <li className="flex gap-3">
          <Step n={1} />
          <div className="min-w-0 flex-1">
            <p className="font-semibold">Check the cleaned log</p>
            <p className="mt-0.5 text-muted">
              The log shows what VARden did before the problem. It never contains values, and this copy also hides
              variable names, your user name and anything that looks like a key or password.
            </p>

            {report && !empty && (
              <p className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted">
                <span>{plural(report.lines, "line")}</span>
                <span>
                  {keepNames ? "Variable names shown" : `${plural(report.namesHidden, "variable name")} replaced with VAR_1, VAR_2…`}
                </span>
                <span>Your user name hidden {report.userHidden === 1 ? "once" : `${report.userHidden} times`}</span>
                <span className={report.secretsHidden > 0 ? "text-persistent" : ""}>
                  {plural(report.secretsHidden, "secret-looking value")} removed
                </span>
              </p>
            )}

            {empty ? (
              <p className="mt-2 rounded-md border border-rule bg-ink/60 px-3 py-2 text-muted">
                The log is empty, so there's nothing to attach. Just describe the problem.
              </p>
            ) : (
              <pre
                aria-label="Cleaned log"
                tabIndex={0}
                className="selectable mt-2 max-h-56 overflow-auto rounded-md border border-rule bg-ink/60 px-3 py-2 font-mono text-[0.75rem] leading-relaxed whitespace-pre-wrap [overflow-wrap:anywhere]"
              >
                {report ? report.text : "Reading the log…"}
              </pre>
            )}

            <label className="mt-2 flex items-start gap-2 text-muted">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 accent-[var(--color-process)]"
                checked={keepNames}
                onChange={(e) => setKeepNames(e.target.checked)}
              />
              <span>
                Show variable names. It can help find the cause; leave it off if a name is private (a client's name, say).
              </span>
            </label>
          </div>
        </li>

        <li className="flex gap-3">
          <Step n={2} />
          <div className="min-w-0 flex-1">
            <p className="font-semibold">Confirm it's safe to share</p>
            <p className="mt-0.5 text-muted">Issues on GitHub are public. Scroll through the log above. If something in it is private, leave the log out, or delete that line after pasting.</p>
            <label className="mt-2 flex items-start gap-2">
              <input
                type="checkbox"
                className="mt-0.5 h-4 w-4 accent-[var(--color-process)]"
                checked={reviewed}
                disabled={!report || empty}
                onChange={(e) => setReviewed(e.target.checked)}
              />
              <span>I've looked through the log and it contains nothing I want to keep private.</span>
            </label>
            <div className="mt-2 flex flex-wrap gap-2">
              <Button variant="secondary" disabled={!reviewed} onClick={() => void copy()}>
                {copied ? <CheckIcon className="text-synced" /> : <CopyIcon />}
                {copied ? "Copied" : "Copy log"}
              </Button>
              <Button variant="secondary" disabled={!reviewed} onClick={() => void saveFile()}>
                Save as file…
              </Button>
            </div>
          </div>
        </li>

        <li className="flex gap-3">
          <Step n={3} />
          <div className="min-w-0 flex-1">
            <p className="font-semibold">Describe what happened</p>
            <p className="mt-0.5 text-muted">
              The form opens with your VARden version{version ? ` (${version})` : ""} and system filled in. Say what you
              did and what you expected, and paste the log into the Log box. Never paste a key or password; write{" "}
              <span className="font-mono">sk-…</span> instead.
            </p>
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
