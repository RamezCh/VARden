// Pure helpers for the PATH editor: your entries plus one fixed "system"
// slot, moved and compared as one list.

/** The system PATH's place in the combined list. */
export const BASE = Symbol("system PATH");
export type Slot = string | typeof BASE;

export function toSlots(entries: string[], baseAt: number): Slot[] {
  const at = Math.min(Math.max(baseAt, 0), entries.length);
  return [...entries.slice(0, at), BASE, ...entries.slice(at)];
}

export function fromSlots(slots: Slot[]): { entries: string[]; baseAt: number } {
  const baseAt = slots.indexOf(BASE);
  return { entries: slots.filter((s): s is string => s !== BASE), baseAt: Math.max(baseAt, 0) };
}

/**
 * Move the slot at `index` one step. Returns null when it can't move: the
 * system slot never moves by itself, and when `baseMovable` is false (Windows)
 * nothing can go above it.
 */
export function moveSlot<T>(
  slots: (T | typeof BASE)[],
  index: number,
  dir: -1 | 1,
  baseMovable: boolean,
): (T | typeof BASE)[] | null {
  const target = index + dir;
  if (slots[index] === BASE || target < 0 || target >= slots.length) return null;
  if (!baseMovable && slots[target] === BASE) return null;
  const next = [...slots];
  next[index] = slots[target];
  next[target] = slots[index];
  return next;
}

/** Problems that would stop the PATH from being saved. */
export function entryError(entry: string, separator: string, unix: boolean): string | null {
  if (entry.trim() === "") return "Empty entry";
  if (entry.trim() !== entry) return "Starts or ends with a space";
  if (entry.includes(separator)) return `Contains "${separator}", which separates PATH entries. Add each folder on its own.`;
  if (unix && /["'`\\]|\$\(/.test(entry)) return "Quotes, backslashes and commands aren't allowed";
  return null;
}

export interface PathDiff {
  added: string[];
  removed: string[];
  /** Same entries, different order (or the system PATH moved). */
  reordered: boolean;
  changed: boolean;
}

export function diffPath(before: Slot[], after: Slot[], caseInsensitive: boolean): PathDiff {
  const norm = (s: string) => (caseInsensitive ? s.toLowerCase() : s);
  const count = (slots: Slot[]) => {
    const m = new Map<string, number>();
    for (const s of slots) if (s !== BASE) m.set(norm(s), (m.get(norm(s)) ?? 0) + 1);
    return m;
  };
  const b = count(before);
  const a = count(after);
  const added: string[] = [];
  const removed: string[] = [];
  const left = new Map(b);
  for (const s of after) {
    if (s === BASE) continue;
    const n = left.get(norm(s)) ?? 0;
    if (n > 0) left.set(norm(s), n - 1);
    else added.push(s);
  }
  const right = new Map(a);
  for (const s of before) {
    if (s === BASE) continue;
    const n = right.get(norm(s)) ?? 0;
    if (n > 0) right.set(norm(s), n - 1);
    else removed.push(s);
  }
  const key = (slots: Slot[]) => slots.map((s) => (s === BASE ? "\u0000" : norm(s))).join("\u0001");
  const keep = (slots: Slot[], drop: string[]) => {
    const d = new Map<string, number>();
    for (const s of drop) d.set(norm(s), (d.get(norm(s)) ?? 0) + 1);
    return slots.filter((s) => {
      if (s === BASE) return true;
      const n = d.get(norm(s)) ?? 0;
      if (n > 0) {
        d.set(norm(s), n - 1);
        return false;
      }
      return true;
    });
  };
  const reordered = key(keep(before, removed)) !== key(keep(after, added));
  return { added, removed, reordered, changed: added.length > 0 || removed.length > 0 || reordered };
}
