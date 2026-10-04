import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { createMockEngine } from "./mock";
import { Settings } from "./Settings";
import { renderWith } from "./test-utils";

afterEach(cleanup);

const click = async (name: string) => {
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name }));
  });
};

const patternsBox = () => screen.getByRole("textbox", { name: /Names to leave out/ }) as HTMLTextAreaElement;

describe("the settings screen", () => {
  it("saves the names to leave out, one per line", async () => {
    const engine = createMockEngine();
    const setPatterns = vi.spyOn(engine, "setPatterns");
    renderWith(engine, <Settings />);
    await screen.findByRole("heading", { name: "What to leave out" });
    expect(patternsBox().value).toContain("*.kdbx");

    fireEvent.change(patternsBox(), { target: { value: "*.bak\nnode_modules" } });
    await click("Save names");
    expect(setPatterns).toHaveBeenCalledWith(["*.bak", "node_modules"]);
    expect(screen.getByRole("status").textContent).toContain("Saved");
    expect(patternsBox().value).toBe("*.bak\nnode_modules");
  });

  it("says why a name cannot be used, and keeps what was typed", async () => {
    renderWith(createMockEngine(), <Settings />);
    await screen.findByRole("heading", { name: "What to leave out" });
    fireEvent.change(patternsBox(), { target: { value: "docs/private" } });
    await click("Save names");
    expect(screen.getByRole("alert").textContent).toContain("choose it instead");
    expect(patternsBox().value).toBe("docs/private");
  });

  it("restores the default names", async () => {
    renderWith(createMockEngine(), <Settings />);
    await screen.findByRole("heading", { name: "What to leave out" });
    fireEvent.change(patternsBox(), { target: { value: "" } });
    await click("Restore the defaults");
    expect(patternsBox().value).toContain("*passwords*");
  });

  it("leaves out a folder and includes it again", async () => {
    renderWith(createMockEngine(), <Settings />);
    await screen.findByText("No folders are left out.");
    await click("Leave out a folder…");
    expect(await screen.findByText(/Private 3/)).toBeTruthy();
    await click("Include again");
    expect(await screen.findByText("No folders are left out.")).toBeTruthy();
  });

  it("warns when the index is in a synced folder", async () => {
    const engine = createMockEngine();
    const view = await engine.settings();
    vi.spyOn(engine, "settings").mockResolvedValue({ ...view, dataSyncedBy: "OneDrive" });
    renderWith(engine, <Settings />);
    expect((await screen.findByRole("alert")).textContent).toContain("OneDrive copies to the internet");
  });

  it("shows where the index is and how large", async () => {
    renderWith(createMockEngine(), <Settings />);
    expect(await screen.findByText(/AppData.*Catchword.*17[.,]5 MB/)).toBeTruthy();
  });

  it("chooses how much of the processor indexing uses", async () => {
    const engine = createMockEngine();
    const setResourceMode = vi.spyOn(engine, "setResourceMode");
    renderWith(engine, <Settings />);
    const balanced = (await screen.findByRole("radio", { name: /Balanced/ })) as HTMLInputElement;
    expect(balanced.checked).toBe(true);
    await act(async () => {
      fireEvent.click(screen.getByRole("radio", { name: /Light One core/ }));
    });
    expect(setResourceMode).toHaveBeenCalledWith("light");
    expect(((await screen.findByRole("radio", { name: /Light One core/ })) as HTMLInputElement).checked).toBe(true);
  });

  it("checks the index, and rebuilds it after one confirmation", async () => {
    const engine = createMockEngine();
    const rebuild = vi.spyOn(engine, "rebuildIndex");
    renderWith(engine, <Settings />);
    await screen.findByRole("heading", { name: "Your data" });
    await click("Check the index");
    expect(screen.getByText("No damage found.")).toBeTruthy();

    await click("Rebuild the index");
    await click("Keep");
    expect(rebuild).not.toHaveBeenCalled();
    await click("Rebuild the index");
    expect(screen.getByText(/Your folders and settings stay/)).toBeTruthy();
    await click("Rebuild the index");
    expect(rebuild).toHaveBeenCalledTimes(1);
  });

  it("shows the licences of the parts it uses, as plain text", async () => {
    const engine = createMockEngine();
    renderWith(engine, <Settings />);
    await screen.findByRole("heading", { name: "About" });
    expect(screen.getByText(/Apache License 2.0/)).toBeTruthy();
    await click("Show the licences of the parts Catchword uses");
    expect(screen.getByLabelText("Licences of the parts Catchword uses").textContent).toContain("react");

    cleanup();
    vi.spyOn(engine, "notices").mockResolvedValue(null);
    renderWith(engine, <Settings />);
    await screen.findByRole("heading", { name: "About" });
    await click("Show the licences of the parts Catchword uses");
    expect(screen.getByText(/added when the app is packaged/)).toBeTruthy();
  });

  it("changes the theme and text size at once, and saves them", async () => {
    const engine = createMockEngine();
    const setAppearance = vi.spyOn(engine, "setAppearance");
    renderWith(engine, <Settings />);
    await screen.findByRole("heading", { name: "Appearance" });
    await act(async () => {
      fireEvent.click(screen.getByRole("radio", { name: "Dark" }));
    });
    expect(setAppearance).toHaveBeenLastCalledWith("dark", "normal");
    expect(document.documentElement.dataset.theme).toBe("dark");
    await act(async () => {
      fireEvent.click(screen.getByRole("radio", { name: "Larger (130%)" }));
    });
    expect(setAppearance).toHaveBeenLastCalledWith("dark", "larger");
    expect(document.documentElement.dataset.textSize).toBe("larger");
    await act(async () => {
      fireEvent.click(screen.getByRole("radio", { name: "As Windows is set" }));
    });
    expect(document.documentElement.dataset.theme).toBeUndefined();
  });

  it("turns detailed logs on and off", async () => {
    const engine = createMockEngine();
    const setDetailedLogs = vi.spyOn(engine, "setDetailedLogs");
    renderWith(engine, <Settings />);
    const box = (await screen.findByRole("checkbox", { name: /Detailed logs/ })) as HTMLInputElement;
    expect(box.checked).toBe(false);
    await act(async () => {
      fireEvent.click(box);
    });
    expect(setDetailedLogs).toHaveBeenCalledWith(true);
    expect(((await screen.findByRole("checkbox", { name: /Detailed logs/ })) as HTMLInputElement).checked).toBe(true);
  });

  it("shows the diagnostics report before saving it, without paths unless asked", async () => {
    const engine = createMockEngine();
    const diagnostics = vi.spyOn(engine, "diagnostics");
    const save = vi.spyOn(engine, "saveDiagnostics");
    renderWith(engine, <Settings />);
    await screen.findByRole("heading", { name: "Diagnostics" });
    expect(screen.queryByRole("button", { name: "Save the report…" })).toBeNull();

    await click("Prepare a diagnostics report");
    expect(diagnostics).toHaveBeenLastCalledWith(false);
    const report = screen.getByLabelText("The report, exactly as it will be saved");
    expect(report.textContent).toContain("File and folder names: left out.");
    expect(report.textContent).not.toContain("Documents");

    await click("Save the report…");
    expect(save).toHaveBeenCalled();
    expect(screen.getByText(/Saved as catchword-diagnostics.txt/)).toBeTruthy();

    // Asking for paths makes a new report; the old one is not saved by mistake.
    await act(async () => {
      fireEvent.click(screen.getByRole("checkbox", { name: /Include file and folder names/ }));
    });
    expect(screen.queryByRole("button", { name: "Save the report…" })).toBeNull();
    await click("Prepare a diagnostics report");
    expect(diagnostics).toHaveBeenLastCalledWith(true);
    expect(screen.getByLabelText("The report, exactly as it will be saved").textContent).toContain("Documents");
  });

  it("deletes all data only after one confirmation, then starts again as new", async () => {
    const engine = createMockEngine();
    const deleteAllData = vi.spyOn(engine, "deleteAllData");
    renderWith(engine, <App />);
    await screen.findByRole("button", { name: "Up to date" });
    await click("Settings");
    await screen.findByRole("heading", { name: "Your data" });

    await click("Delete all data");
    await click("Keep");
    expect(deleteAllData).not.toHaveBeenCalled();

    await click("Delete all data");
    expect(screen.getByText(/Your own files are not touched/)).toBeTruthy();
    const confirm = screen.getAllByRole("button", { name: "Delete all data" });
    await act(async () => {
      fireEvent.click(confirm[0]);
    });
    expect(deleteAllData).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("heading", { name: "Your files never leave this computer" })).toBeTruthy();
  });
});
