import { useState } from "react";
import type { Profile, ProfilesView as ProfilesData, Scope } from "../lib/api";
import { HELP } from "../lib/helpText";
import { formatShort, plural } from "../lib/useEnvironment";
import { Help } from "./Help";
import { PageHeader } from "./PageHeader";
import { Button, IconButton } from "./IconButton";
import { PencilIcon, PlusIcon, TrashIcon } from "./icons";

interface Props {
  data: ProfilesData;
  busy: boolean;
  onNew: () => void;
  onEdit: (p: Profile) => void;
  onDelete: (p: Profile) => Promise<boolean>;
  onApply: (p: Profile, scope: Scope) => void;
  onTurnOff: (name: string) => void;
}

const scopeWord = (s: Scope) => (s === "user" ? "user variables" : "temporary variables");

export function ProfilesView({ data, busy, onNew, onEdit, onDelete, onApply, onTurnOff }: Props) {
  const [confirming, setConfirming] = useState<string | null>(null);
  const { profiles, active } = data;

  return (
    <div className="flex flex-col gap-5">
      <PageHeader
        title="Profiles"
        help={<Help label="What are profiles?">{HELP.profiles}</Help>}
        description="Named sets of variables, for example one per client or project. Applying one switches them all at once; turning it off puts the previous values back."
        actions={
          <Button variant="primary" onClick={onNew}>
            <PlusIcon /> New profile
          </Button>
        }
      />

      {active && (
        <div className="flex flex-wrap items-center gap-3 rounded-lg border border-process/40 bg-process/[0.07] px-4 py-3">
          <span className="h-2 w-2 rounded-full bg-process" />
          <p className="min-w-0 flex-1 text-sm">
            <strong>{active.name}</strong> is active as {scopeWord(active.scope)} since {formatShort(active.appliedAt)}.
            {active.outdated && <span className="text-persistent"> It was edited since; re-apply to use the changes.</span>}
          </p>
          {active.outdated && (
            <Button
              variant="primary"
              disabled={busy}
              onClick={() => {
                const p = profiles.find((x) => x.id === active.id);
                if (p) onApply(p, active.scope);
              }}
            >
              Re-apply
            </Button>
          )}
          <Button variant="secondary" disabled={busy} onClick={() => onTurnOff(active.name)}>
            Turn off
          </Button>
        </div>
      )}

      {profiles.length === 0 ? (
        <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed border-rule px-6 py-12 text-center">
          <p className="max-w-[48ch] text-sm text-muted">
            No profiles yet. Create one with the variables a project needs; leave a value empty to mark it as required.
          </p>
          <Button variant="primary" onClick={onNew}>
            <PlusIcon /> New profile
          </Button>
        </div>
      ) : (
        <ul className="divide-y divide-rule overflow-hidden rounded-lg border border-rule bg-panel">
          {profiles.map((p) => {
            const isActive = active?.id === p.id;
            const expected = p.vars.filter((v) => v.value === "").length;
            return (
              <li key={p.id} className="flex flex-col gap-3 px-4 py-3">
                <div className="flex flex-wrap items-start gap-3">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="font-semibold">{p.name}</span>
                      {isActive && (
                        <span className="rounded bg-process/15 px-1.5 py-0.5 text-xs text-process">
                          Active, {active!.scope === "user" ? "user" : "temporary"}
                        </span>
                      )}
                    </div>
                    <p className="text-sm text-muted">
                      {plural(p.vars.length, "variable")}
                      {expected > 0 && `, ${expected} required to fill in`}
                    </p>
                    {p.vars.length > 0 && (
                      <p className="mt-1 truncate font-mono text-xs text-faint">
                        {p.vars
                          .slice(0, 5)
                          .map((v) => v.key)
                          .join("  ")}
                        {p.vars.length > 5 && `  +${p.vars.length - 5}`}
                      </p>
                    )}
                  </div>
                  <div className="flex shrink-0 flex-wrap items-center gap-1">
                    {isActive ? (
                      <Button variant="secondary" disabled={busy} onClick={() => onTurnOff(p.name)}>
                        Turn off
                      </Button>
                    ) : (
                      <>
                        <Button
                          variant="primary"
                          disabled={busy}
                          onClick={() => onApply(p, "user")}
                          title="Save these values as user variables (previous values are remembered)"
                        >
                          Apply
                        </Button>
                        <Button
                          variant="secondary"
                          disabled={busy}
                          onClick={() => onApply(p, "process")}
                          title="Only for open terminals, after Sync; nothing is saved"
                        >
                          Apply temporarily
                        </Button>
                      </>
                    )}
                    <IconButton label={`Edit ${p.name}`} onClick={() => onEdit(p)}>
                      <PencilIcon />
                    </IconButton>
                    {!isActive && (
                      <IconButton
                        label={`Delete ${p.name}`}
                        className="hover:text-missing"
                        onClick={() => setConfirming(p.id)}
                      >
                        <TrashIcon />
                      </IconButton>
                    )}
                  </div>
                </div>
                {confirming === p.id && (
                  <div className="flex flex-wrap items-center justify-end gap-2 rounded-md bg-missing/[0.08] px-3 py-2">
                    <span className="mr-auto text-sm">
                      Delete the profile {p.name}? Your variables aren't changed.
                    </span>
                    <Button onClick={() => setConfirming(null)} autoFocus>
                      Cancel
                    </Button>
                    <Button
                      variant="danger"
                      disabled={busy}
                      onClick={async () => {
                        if (await onDelete(p)) setConfirming(null);
                      }}
                    >
                      Delete
                    </Button>
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
