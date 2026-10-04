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
    expect(screen.getByText(/reader crashed/).textContent).toContain("(1, failed)");
  });

  it("retries failed files on request, and says which ones are parked", async () => {
    const engine = createMockEngine();
    const retryFailed = vi.spyOn(engine, "retryFailed");
    renderWith(engine, <Library status={await engine.status()} />);
    expect(screen.getByText(/not tried again until you ask/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(retryFailed).toHaveBeenCalled();
    cleanup();
    renderWith(engine, <Library status={await engine.status()} />);
    expect(screen.queryByText(/not tried again until you ask/)).toBeNull();
  });

  it("offers no retry when nothing failed", async () => {
    const engine = createMockEngine();
    const status = await engine.status();
    const skippedOnly = { ...status, notIndexed: status.notIndexed.filter((file) => !file.failed) };
    renderWith(engine, <Library status={skippedOnly} />);
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });

  it("pauses and resumes indexing, and says why it is paused", async () => {
    const engine = createMockEngine();
    const pause = vi.spyOn(engine, "pauseIndexing");
    const resume = vi.spyOn(engine, "resumeIndexing");
    renderWith(engine, <Library status={await engine.status()} />);
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(pause).toHaveBeenCalled();

    cleanup();
    renderWith(engine, <Library status={await engine.status()} />);
    expect(screen.getByText(/Indexing is paused\./)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Resume" }));
    expect(resume).toHaveBeenCalled();

    cleanup();
    const lowDisk = { ...(await engine.status()), paused: "lowDisk" as const };
    renderWith(engine, <Library status={lowDisk} />);
    expect(screen.getByText(/less than 1 GB is free/)).toBeTruthy();
  });

  it("says which folder is being scanned and which cannot be reached", async () => {
    const engine = createMockEngine();
    const status = await engine.status();
    const states = {
      ...status,
      folders: [
        { ...status.folders[0], state: "scanning" as const },
        { ...status.folders[1], state: "offline" as const },
      ],
    };
    renderWith(engine, <Library status={states} />);
    expect(screen.getByText(/Scanning/)).toBeTruthy();
    expect(screen.getByText(/drive is not connected/).textContent).toContain("stay searchable");
  });

  it("groups files that were not indexed by reason, largest group first", () => {
    const file = (name: string, reason: string): NotIndexed => ({
      name,
      folder: "/d",
      reason,
      failed: false,
      parked: false,
    });
    const groups = byReason([file("a", "scan"), file("b", "password"), file("c", "scan")]);
    expect(groups.map(([reason, files]) => [reason, files.length])).toEqual([
      ["scan", 2],
      ["password", 1],
    ]);
  });
});
