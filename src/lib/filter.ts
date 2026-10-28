// Pure helpers for the dashboard: status metadata, filtering, sorting.
import type { EnvVar, Scope, Status } from "./api";

export const STATUS_ORDER: Status[] = ["missingStale", "persistentOnly", "processOnly", "activeSynced"];

export const STATUS_META: Record<Status, { label: string; help: string; color: string }> = {
  activeSynced: {
    label: "Active & synced",
    help: "Saved, and the running environment has the same value.",
    color: "var(--color-synced)",
  },
  persistentOnly: {
    label: "Persistent only",
    help: "Saved, but not loaded yet. New terminals will pick it up.",
    color: "var(--color-persistent)",
  },
  processOnly: {
    label: "Process only",
    help: "Set for this session only. Gone when the app closes.",
    color: "var(--color-process)",
  },
  missingStale: {
    label: "Missing / stale",
    help: "Expected but empty or missing.",
    color: "var(--color-missing)",
  },
};

export type ScopeFilter = "all" | Scope;
export type SortKey = "key" | "scope" | "status";
export interface SortState {
  key: SortKey;
  dir: "asc" | "desc";
}

export interface Filters {
  query: string;
  scope: ScopeFilter;
  /** Empty set = all statuses. */
  statuses: ReadonlySet<Status>;
}

/** Identifies a row: the same key can exist once per scope. */
export const rowId = (v: Pick<EnvVar, "key" | "scope">) => `${v.scope}:${v.key}`;

/**
 * Search matches the key, and the value only when it is visible: a masked
 * secret must not be discoverable by typing guesses into the search box.
 */
export function matchesQuery(v: EnvVar, query: string, valueVisible: boolean): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  if (v.key.toLowerCase().includes(q)) return true;
  return valueVisible && v.value.toLowerCase().includes(q);
}

export function filterVars(
  vars: EnvVar[],
  f: Filters,
  isVisible: (v: EnvVar) => boolean,
): EnvVar[] {
  return vars.filter(
    (v) =>
      (f.scope === "all" || v.scope === f.scope) &&
      (f.statuses.size === 0 || f.statuses.has(v.status)) &&
      matchesQuery(v, f.query, isVisible(v)),
  );
}

export function sortVars(vars: EnvVar[], sort: SortState): EnvVar[] {
  const byKey = (a: EnvVar, b: EnvVar) => a.key.localeCompare(b.key, undefined, { sensitivity: "base" });
  const primary = (a: EnvVar, b: EnvVar): number => {
    switch (sort.key) {
      case "key":
        return byKey(a, b);
      case "scope":
        return a.scope.localeCompare(b.scope);
      case "status":
        return STATUS_ORDER.indexOf(a.status) - STATUS_ORDER.indexOf(b.status);
    }
  };
  const dir = sort.dir === "asc" ? 1 : -1;
  return [...vars].sort((a, b) => dir * primary(a, b) || byKey(a, b));
}

export function countByStatus(vars: EnvVar[]): Record<Status, number> {
  const counts: Record<Status, number> = { activeSynced: 0, persistentOnly: 0, processOnly: 0, missingStale: 0 };
  for (const v of vars) counts[v.status] += 1;
  return counts;
}

/** Mirrors envcore::validate::validate_key. */
export function keyError(key: string): string | null {
  if (!key) return "Enter a name.";
  if (/^[0-9]/.test(key)) return "Names can't start with a digit.";
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) return "Use letters, digits and underscores only.";
  return null;
}

/** Mirrors the backend heuristic so a new value is masked while typing. */
export function looksSensitive(key: string): boolean {
  const k = key.toUpperCase();
  if (["PWD", "OLDPWD", "KEYBOARD", "PATHEXT"].includes(k)) return false;
  const markers = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "PASS", "CREDENTIAL", "AUTH", "PRIVATE", "SESSION", "COOKIE", "DSN"];
  return k.split("_").some((part) => markers.some((m) => part === m || part.endsWith(m)));
}
