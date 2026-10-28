// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { EnvVar } from "../lib/api";
import { VariablePanel, type PanelState } from "./VariablePanel";

const v = (key: string, over: Partial<EnvVar> = {}): EnvVar => ({
  key,
  value: "",
  liveValue: null,
  scope: "user",
  status: "activeSynced",
  kind: "string",
  sensitive: false,
  pendingSync: false,
  expanded: null,
  checkedPaths: 0,
  missingPaths: [],
  ...over,
});

function setup(state: PanelState, allVars: EnvVar[] = []) {
  const props = {
    onClose: vi.fn(),
    onSet: vi.fn(async () => true),
    onRename: vi.fn(async () => true),
    onDelete: vi.fn(async () => true),
  };
  render(<VariablePanel state={state} allVars={allVars} caseInsensitive={false} busy={false} {...props} />);
  return { user: userEvent.setup(), ...props };
}

describe("VariablePanel", () => {
  it("adds a variable with Enter and closes", async () => {
    const { user, onSet, onClose } = setup({ kind: "new" });
    expect(screen.getByLabelText("Name")).toHaveFocus();
    await user.type(screen.getByLabelText("Name"), "API_URL");
    await user.type(screen.getByLabelText("Value"), "https://example.test{Enter}");
    expect(onSet).toHaveBeenCalledWith("API_URL", "https://example.test", "user", false);
    expect(onClose).toHaveBeenCalled();
  });

  it("refuses a bad name and keeps the panel open", async () => {
    const { user, onSet, onClose } = setup({ kind: "new" });
    await user.type(screen.getByLabelText("Name"), "1 BAD NAME");
    await user.click(screen.getByRole("button", { name: "Add variable" }));
    expect(onSet).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Name")).toHaveAttribute("aria-invalid", "true");
  });

  it("warns that adding an existing name replaces it", async () => {
    const { user, onSet } = setup({ kind: "new" }, [v("EDITOR", { value: "vim" })]);
    await user.type(screen.getByLabelText("Name"), "EDITOR");
    expect(screen.getByText(/already exists\. Saving replaces its value/)).toBeInTheDocument();
    await user.type(screen.getByLabelText("Value"), "code{Enter}");
    expect(onSet).toHaveBeenCalledWith("EDITOR", "code", "user", true);
  });

  it("hides secrets until shown, and Tab goes from name to value", async () => {
    const { user } = setup({ kind: "new" });
    await user.type(screen.getByLabelText("Name"), "OPENAI_API_KEY");
    const value = screen.getByLabelText("Value");
    expect(value.className).toContain("text-security:disc");

    await user.tab();
    expect(value).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "Show" }));
    expect(value.className).not.toContain("text-security:disc");
    expect(value).toBeInTheDocument();
  });

  it("renames and saves a new value in one go", async () => {
    const editing = v("OLD_NAME", { value: "1" });
    const { user, onRename, onSet, onClose } = setup({ kind: "edit", v: editing }, [editing]);
    const name = screen.getByLabelText("Name");
    await user.clear(name);
    await user.type(name, "NEW_NAME");
    await user.clear(screen.getByLabelText("Value"));
    await user.type(screen.getByLabelText("Value"), "2");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onRename).toHaveBeenCalledWith("OLD_NAME", "NEW_NAME", "user");
    expect(onSet).toHaveBeenCalledWith("NEW_NAME", "2", "user", true);
    expect(onClose).toHaveBeenCalled();
  });

  it("won't quietly turn a saved variable into a temporary one", () => {
    setup({ kind: "edit", v: v("SAVED", { value: "x" }) });
    expect(screen.getByRole("radio", { name: /Temporary/ })).toBeDisabled();
  });

  it("asks before deleting", async () => {
    const { user, onDelete } = setup({ kind: "edit", v: v("GONE", { value: "x" }) });
    await user.click(screen.getByRole("button", { name: "Delete…" }));
    expect(onDelete).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(onDelete).toHaveBeenCalledWith("GONE", "user");
  });
});
