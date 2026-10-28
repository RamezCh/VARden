import { describe, expect, it } from "vitest";
import { BASE, diffPath, entryError, fromSlots, moveSlot, toSlots } from "./pathEdit";

describe("PATH slots", () => {
  it("round-trips entries and the system position", () => {
    const slots = toSlots(["/a", "/b", "/c"], 1);
    expect(slots).toEqual(["/a", BASE, "/b", "/c"]);
    expect(fromSlots(slots)).toEqual({ entries: ["/a", "/b", "/c"], baseAt: 1 });
    expect(toSlots(["/a"], 9)).toEqual(["/a", BASE]);
  });

  it("moves entries across the system PATH only when allowed", () => {
    const slots = toSlots(["/a", "/b"], 0);
    expect(moveSlot(slots, 1, -1, true)).toEqual(["/a", BASE, "/b"]);
    expect(moveSlot(slots, 1, -1, false)).toBeNull();
    expect(moveSlot(slots, 0, 1, true)).toBeNull(); // the system slot itself
    expect(moveSlot(slots, 2, 1, true)).toBeNull(); // already last
    expect(moveSlot(slots, 1, 1, false)).toEqual([BASE, "/b", "/a"]);
  });
});

describe("entryError", () => {
  it("rejects entries that would break PATH", () => {
    expect(entryError("/usr/local/bin", ":", true)).toBeNull();
    expect(entryError("", ":", true)).toMatch(/Empty/);
    expect(entryError(" /a", ":", true)).toMatch(/space/);
    expect(entryError("/a:/b", ":", true)).toMatch(/separates/);
    expect(entryError("C:\\tools", ";", false)).toBeNull();
    expect(entryError("$(brew --prefix)/bin", ":", true)).toMatch(/commands/);
  });
});

describe("diffPath", () => {
  it("reports additions, removals and moves", () => {
    const before = toSlots(["/a", "/b"], 2);
    expect(diffPath(before, before, false).changed).toBe(false);
    const d = diffPath(before, toSlots(["/b", "/c"], 2), false);
    expect(d).toMatchObject({ added: ["/c"], removed: ["/a"], reordered: false });
    expect(diffPath(before, toSlots(["/b", "/a"], 2), false)).toMatchObject({ added: [], removed: [], reordered: true });
    expect(diffPath(before, toSlots(["/a", "/b"], 0), false).reordered).toBe(true);
    expect(diffPath(toSlots(["C:\\A"], 0), toSlots(["c:\\a"], 0), true).changed).toBe(false);
  });
});
