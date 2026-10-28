// @vitest-environment jsdom
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { issueUrl, ReportProblem } from "./ReportProblem";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: async () => "1.2.3" }));

const report = (keepNames: boolean) => ({
  text: keepNames ? "Set API_URL (User)" : "Set VAR_1 (User)",
  namesHidden: keepNames ? 0 : 1,
  userHidden: 1,
  secretsHidden: 2,
  lines: 1,
});

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string, args?: { keepNames?: boolean }) => {
    if (cmd === "prepare_report") return report(Boolean(args?.keepNames));
    if (cmd === "copy_text") return undefined;
    throw new Error(`unexpected ${cmd}`);
  });
});

describe("ReportProblem", () => {
  it("shows the cleaned log and what was hidden", async () => {
    render(<ReportProblem platform={null} onClose={() => {}} onError={() => {}} />);
    expect(await screen.findByText("Set VAR_1 (User)")).toBeInTheDocument();
    expect(screen.getByText("2 secret-looking values removed")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("prepare_report", { keepNames: false });
  });

  it("only lets you copy after confirming the log is safe to share", async () => {
    const user = userEvent.setup();
    render(<ReportProblem platform={null} onClose={() => {}} onError={() => {}} />);
    await screen.findByText("Set VAR_1 (User)");

    const copy = screen.getByRole("button", { name: /Copy log/ });
    expect(copy).toBeDisabled();
    await user.click(screen.getByRole("checkbox", { name: /contains nothing I want to keep private/ }));
    expect(copy).toBeEnabled();
    await user.click(copy);
    expect(invoke).toHaveBeenCalledWith("copy_text", { text: "Set VAR_1 (User)", secret: false });
  });

  it("asks for a fresh look when names are switched on", async () => {
    const user = userEvent.setup();
    render(<ReportProblem platform={null} onClose={() => {}} onError={() => {}} />);
    await screen.findByText("Set VAR_1 (User)");
    const confirm = screen.getByRole("checkbox", { name: /contains nothing I want to keep private/ });
    await user.click(confirm);
    expect(confirm).toBeChecked();

    await user.click(screen.getByRole("checkbox", { name: /Show variable names/ }));
    expect(await screen.findByText("Set API_URL (User)")).toBeInTheDocument();
    await waitFor(() => expect(confirm).not.toBeChecked());
    expect(screen.getByRole("button", { name: /Copy log/ })).toBeDisabled();
  });

  it("fills in the version and system on GitHub", () => {
    const url = new URL(issueUrl("1.2.3", "windows"));
    expect(url.pathname).toBe("/RamezCh/VARden/issues/new");
    expect(url.searchParams.get("template")).toBe("bug_report.yml");
    expect(url.searchParams.get("version")).toBe("1.2.3");
    expect(url.searchParams.get("os")).toBe("Windows");
  });
});
