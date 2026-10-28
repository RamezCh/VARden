// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PathEntry, PathInfo } from "../lib/api";
import { PathEditor } from "./PathEditor";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: async () => null }));

const entry = (value: string, over: Partial<PathEntry> = {}): PathEntry => ({
  value,
  expanded: null,
  checked: true,
  missing: false,
  duplicate: false,
  ...over,
});

const INFO: PathInfo = {
  separator: ";",
  entries: [entry("%USERPROFILE%\\bin", { expanded: "C:\\Users\\me\\bin" }), entry("C:\\gone", { missing: true })],
  baseAt: 0,
  baseMovable: false,
  base: [entry("C:\\Windows\\system32")],
  note: null,
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string, args?: { entries: string[]; baseAt: number }) => {
    if (cmd === "get_path_info") return INFO;
    if (cmd === "check_path_entries") return { ...INFO, entries: args!.entries.map((e) => entry(e)) };
    throw new Error(`unexpected ${cmd}`);
  });
});

function setup() {
  const onSave = vi.fn(async () => true);
  const onClose = vi.fn();
  render(<PathEditor windows busy={false} onClose={onClose} onSave={onSave} />);
  return { user: userEvent.setup(), onSave, onClose };
}

describe("PathEditor", () => {
  it("lists your entries and flags folders that don't exist", async () => {
    setup();
    expect(await screen.findByLabelText("PATH entry 1")).toHaveValue("%USERPROFILE%\\bin");
    expect(screen.getByText("C:\\Users\\me\\bin")).toBeInTheDocument();
    expect(screen.getByText("This folder doesn't exist.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Review changes" })).toBeDisabled();
  });

  it("refuses a duplicate folder and an entry with the separator", async () => {
    const { user } = setup();
    const add = await screen.findByLabelText("Folder to add");
    await user.type(add, "%userprofile%\\BIN{Enter}");
    expect(screen.getByText("That folder is already in your PATH.")).toBeInTheDocument();

    await user.clear(add);
    await user.type(add, "C:\\a;C:\\b{Enter}");
    expect(screen.queryByLabelText("PATH entry 3")).not.toBeInTheDocument();
  });

  it("adds, removes and reorders, then saves only after review", async () => {
    const { user, onSave, onClose } = setup();
    await user.type(await screen.findByLabelText("Folder to add"), "C:\\tools{Enter}");
    expect(screen.getByLabelText("PATH entry 3")).toHaveValue("C:\\tools");

    await user.click(screen.getByRole("button", { name: "Remove C:\\gone" }));
    await user.click(screen.getAllByRole("button", { name: "Move up" })[1]);
    expect(screen.getByLabelText("PATH entry 1")).toHaveValue("C:\\tools");

    await user.click(screen.getByRole("button", { name: "Review changes" }));
    expect(onSave).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Save PATH" }));
    expect(onSave).toHaveBeenCalledWith(["C:\\tools", "%USERPROFILE%\\bin"], 0);
    expect(onClose).toHaveBeenCalled();
  });
});
