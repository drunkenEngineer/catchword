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

  it("shows where the index is and how large", async () => {
    renderWith(createMockEngine(), <Settings />);
    expect(await screen.findByText(/AppData.*Catchword.*17[.,]5 MB/)).toBeTruthy();
  });

  it("deletes all data only after one confirmation, then starts again as new", async () => {
    const engine = createMockEngine();
    const deleteAllData = vi.spyOn(engine, "deleteAllData");
    renderWith(engine, <App />);
    await screen.findByText("Up to date");
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
