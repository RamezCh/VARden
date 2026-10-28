import { describe, expect, it } from "vitest";
import type { EnvVar } from "./api";
import { countByStatus, filterVars, keyError, looksSensitive, sortVars } from "./filter";

const v = (key: string, over: Partial<EnvVar> = {}): EnvVar => ({
  key,
  value: "",
  liveValue: null,
  scope: "user",
  status: "activeSynced",
  kind: "string",
  sensitive: false,
  pendingSync: false,
  ...over,
});

const vars = [
  v("OPENAI_API_KEY", { value: "sk-secret", sensitive: true, status: "persistentOnly" }),
  v("EDITOR", { value: "code --wait" }),
  v("DEBUG", { value: "1", scope: "process", status: "processOnly" }),
  v("STRIPE_SECRET", { sensitive: true, status: "missingStale" }),
];
const none = new Set<never>();

describe("filterVars", () => {
  const visible = (x: EnvVar) => !x.sensitive;

  it("matches keys case-insensitively", () => {
    expect(filterVars(vars, { query: "openai", scope: "all", statuses: none }, visible).map((x) => x.key)).toEqual([
      "OPENAI_API_KEY",
    ]);
  });

  it("searches visible values but never masked ones", () => {
    expect(filterVars(vars, { query: "code", scope: "all", statuses: none }, visible)).toHaveLength(1);
    expect(filterVars(vars, { query: "sk-", scope: "all", statuses: none }, visible)).toHaveLength(0);
    expect(filterVars(vars, { query: "sk-", scope: "all", statuses: none }, () => true)).toHaveLength(1);
  });

  it("filters by scope and status", () => {
    expect(filterVars(vars, { query: "", scope: "process", statuses: none }, visible)).toHaveLength(1);
    const statuses = new Set(["missingStale", "persistentOnly"] as const);
    expect(filterVars(vars, { query: "", scope: "all", statuses }, visible)).toHaveLength(2);
  });
});

describe("sortVars", () => {
  it("sorts by status with attention-needing states first, then by key", () => {
    expect(sortVars(vars, { key: "status", dir: "asc" }).map((x) => x.key)).toEqual([
      "STRIPE_SECRET",
      "OPENAI_API_KEY",
      "DEBUG",
      "EDITOR",
    ]);
  });

  it("reverses direction and does not mutate input", () => {
    const copy = [...vars];
    expect(sortVars(vars, { key: "key", dir: "desc" })[0].key).toBe("STRIPE_SECRET");
    expect(vars).toEqual(copy);
  });
});

describe("helpers", () => {
  it("counts statuses", () => {
    expect(countByStatus(vars)).toEqual({ activeSynced: 1, persistentOnly: 1, processOnly: 1, missingStale: 1 });
  });

  it("validates keys like the backend", () => {
    expect(keyError("OK_1")).toBeNull();
    expect(keyError("")).not.toBeNull();
    expect(keyError("1A")).not.toBeNull();
    expect(keyError("A-B")).not.toBeNull();
  });

  it("guesses sensitivity like the backend", () => {
    expect(looksSensitive("GITHUB_TOKEN")).toBe(true);
    expect(looksSensitive("PWD")).toBe(false);
    expect(looksSensitive("EDITOR")).toBe(false);
  });
});
