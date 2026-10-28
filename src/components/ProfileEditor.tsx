import { useState } from "react";
import { syncApi, type EnvEntry, type EnvVar, type Profile } from "../lib/api";
import { keyError, looksSensitive } from "../lib/filter";
import { pickEnvFile } from "../lib/files";
import { plural } from "../lib/useEnvironment";
import { HELP } from "../lib/helpText";
import { Help } from "./Help";
import { Button, IconButton } from "./IconButton";
import { PlusIcon, XIcon } from "./icons";
import { Modal } from "./Modal";
import { input, ValueInput } from "./VariableTable";

interface Props {
  /** null: create a new profile. */
  profile: Profile | null;
  vars: EnvVar[];
  caseInsensitive: boolean;
  busy: boolean;
  onClose: () => void;
  onSave: (profile: Profile) => Promise<boolean>;
  onError: (message: string) => void;
}

interface Row extends EnvEntry {
  /** Stable React key while keys are being typed. */
  rid: number;
}

let nextRid = 1;
const toRows = (vars: EnvEntry[]): Row[] => vars.map((v) => ({ ...v, rid: nextRid++ }));

export function ProfileEditor({ profile, vars, caseInsensitive, busy, onClose, onSave, onError }: Props) {
  const [name, setName] = useState(profile?.name ?? "");
  const [rows, setRows] = useState<Row[]>(toRows(profile?.vars ?? []));
  const [touched, setTouched] = useState(false);

  const norm = (k: string) => (caseInsensitive ? k.trim().toUpperCase() : k.trim());
  const keyCount = new Map<string, number>();
  for (const r of rows) keyCount.set(norm(r.key), (keyCount.get(norm(r.key)) ?? 0) + 1);
  const rowError = (r: Row) => keyError(r.key.trim()) ?? ((keyCount.get(norm(r.key)) ?? 0) > 1 ? "Listed twice." : null);
  const invalid = !name.trim() || rows.some(rowError);
  const expected = rows.filter((r) => r.key.trim() && r.value === "").length;

  const update = (rid: number, patch: Partial<EnvEntry>) =>
    setRows((rs) => rs.map((r) => (r.rid === rid ? { ...r, ...patch } : r)));

  /** Add entries, replacing values of keys that are already listed. */
  const merge = (entries: EnvEntry[]) =>
    setRows((rs) => {
      const next = [...rs];
      for (const e of entries) {
        const i = next.findIndex((r) => norm(r.key) === norm(e.key));
        if (i >= 0) next[i] = { ...next[i], value: e.value };
        else next.push({ ...e, rid: nextRid++ });
      }
      return next;
    });

  const available = vars.filter((v) => !rows.some((r) => norm(r.key) === norm(v.key)));

  const importEnv = async () => {
    try {
      const path = await pickEnvFile();
      if (!path) return;
      const parsed = await syncApi.readEnvFile(path);
      merge(parsed.entries);
      if (parsed.skipped.length > 0) onError(`${plural(parsed.skipped.length, "line")} in that file couldn't be read.`);
    } catch (e) {
      onError(String(e));
    }
  };

  const save = async () => {
    setTouched(true);
    if (invalid) return;
    const cleaned = rows.filter((r) => r.key.trim()).map(({ key, value }) => ({ key: key.trim(), value }));
    if (await onSave({ id: profile?.id ?? "", name: name.trim(), vars: cleaned })) onClose();
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={profile ? `Edit ${profile.name}` : "New profile"}
      description={
        <span className="flex items-center gap-1.5">
          A named set of variables you can switch on in one step. <Help label="What are profiles?">{HELP.profiles}</Help>
        </span>
      }
      footer={
        <>
          {expected > 0 && (
            <p className="mr-auto text-sm text-muted">
              {plural(expected, "variable")} left empty: required, filled in later.
            </p>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={busy} onClick={() => void save()}>
            Save profile
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <label className="flex flex-col gap-1.5 text-sm">
          <span className="text-muted">Name</span>
          <input
            className={`${input} font-sans`}
            value={name}
            autoFocus
            placeholder="For example: Client A"
            onChange={(e) => setName(e.target.value)}
          />
          {touched && !name.trim() && <span className="text-xs text-missing">Give the profile a name.</span>}
        </label>

        <div className="flex flex-col gap-2">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-sm text-muted">Variables</span>
            <div className="flex flex-wrap items-center gap-1">
              {available.length > 0 && (
                <select
                  aria-label="Add a current variable"
                  className="h-8 rounded-md border border-rule bg-ink px-2 text-sm text-muted focus:border-process focus:outline-none"
                  value=""
                  onChange={(e) => {
                    const v = available.find((x) => `${x.scope}:${x.key}` === e.target.value);
                    if (v) merge([{ key: v.key, value: v.value }]);
                  }}
                >
                  <option value="">Add a current variable…</option>
                  {available.map((v) => (
                    <option key={`${v.scope}:${v.key}`} value={`${v.scope}:${v.key}`}>
                      {v.key}
                      {v.scope === "process" ? " (temporary)" : ""}
                    </option>
                  ))}
                </select>
              )}
              <Button onClick={() => void importEnv()}>Import .env…</Button>
              <Button onClick={() => setRows((rs) => [...rs, { key: "", value: "", rid: nextRid++ }])}>
                <PlusIcon /> Add
              </Button>
            </div>
          </div>

          {rows.length === 0 ? (
            <p className="rounded-lg border border-dashed border-rule px-4 py-6 text-center text-sm text-muted">
              No variables yet. Add them one by one, pick current ones, or import a .env file.
            </p>
          ) : (
            <ul className="flex flex-col gap-2">
              {rows.map((r) => {
                const err = touched || r.key ? rowError(r) : null;
                return (
                  <li key={r.rid} className="grid grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)_auto] items-start gap-2">
                    <div>
                      <input
                        aria-label="Name"
                        aria-invalid={Boolean(err)}
                        className={input}
                        placeholder="NAME"
                        value={r.key}
                        spellCheck={false}
                        autoComplete="off"
                        onChange={(e) => update(r.rid, { key: e.target.value })}
                      />
                      {err && r.key !== "" && <p className="mt-1 text-xs text-missing">{err}</p>}
                    </div>
                    <ValueInput
                      label={`Value of ${r.key || "new variable"}`}
                      value={r.value}
                      sensitive={looksSensitive(r.key.trim())}
                      placeholder="Leave empty to require it"
                      onChange={(value) => update(r.rid, { value })}
                      onKeyDown={() => {}}
                    />
                    <IconButton label={`Remove ${r.key || "row"}`} onClick={() => setRows((rs) => rs.filter((x) => x.rid !== r.rid))}>
                      <XIcon />
                    </IconButton>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>
    </Modal>
  );
}
