import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { NotIndexed } from "./contract/NotIndexed";
import { byReason, Library } from "./Library";
import { createMockEngine } from "./mock";
import { renderWith } from "./test-utils";

afterEach(cleanup);

describe("the library screen", () => {
  it("removes a folder only after one confirmation", async () => {
    const engine = createMockEngine();
    const removeFolder = vi.spyOn(engine, "removeFolder");
    renderWith(engine, <Library status={await engine.status()} />);

    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Keep" }));
    expect(removeFolder).not.toHaveBeenCalled();

    fireEvent.click(screen.getAllByRole("button", { name: "Remove" })[0]);
    fireEvent.click(screen.getByRole("button", { name: "Remove folder" }));
    expect(removeFolder).toHaveBeenCalledWith(1);
  });

  it("adds a folder and scans on request", async () => {
    const engine = createMockEngine();
    const addFolder = vi.spyOn(engine, "addFolder");
    const indexNow = vi.spyOn(engine, "indexNow");
    renderWith(engine, <Library status={await engine.status()} />);
    fireEvent.click(screen.getByRole("button", { name: "Add a folder" }));
    fireEvent.click(screen.getByRole("button", { name: "Scan now" }));
    expect(addFolder).toHaveBeenCalled();
    expect(indexNow).toHaveBeenCalled();
  });

  it("shows progress for both stages and what needs attention", async () => {
    const engine = createMockEngine();
    const status = { ...(await engine.status()), work: { stage: "words" as const, done: 3, total: 8 } };
    renderWith(engine, <Library status={status} />);
    expect(screen.getByRole("progressbar", { name: "Reading files: 3 of 8" })).toBeTruthy();
    expect(screen.getByRole("progressbar", { name: /Searchable by meaning: 4 of 4/ })).toBeTruthy();
    expect(screen.getByText(/no text layer/).textContent).toContain("(1, skipped)");
  });

  it("groups files that were not indexed by reason, largest group first", () => {
    const file = (name: string, reason: string): NotIndexed => ({ name, folder: "/d", reason, failed: false });
    const groups = byReason([file("a", "scan"), file("b", "password"), file("c", "scan")]);
    expect(groups.map(([reason, files]) => [reason, files.length])).toEqual([
      ["scan", 2],
      ["password", 1],
    ]);
  });
});
