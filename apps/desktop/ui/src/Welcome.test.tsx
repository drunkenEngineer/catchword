import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { createMockEngine } from "./mock";
import { Welcome } from "./Welcome";
import { renderWith } from "./test-utils";

afterEach(cleanup);

const click = async (name: string) => {
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name }));
  });
};

describe("the first launch", () => {
  it("reaches a working search in three steps: promise, folder, start", async () => {
    const engine = createMockEngine({ firstLaunch: true });
    const finish = vi.spyOn(engine, "finishFirstLaunch");
    renderWith(engine, <App />);

    expect(await screen.findByRole("heading", { name: "Your files never leave this computer" })).toBeTruthy();
    await click("Continue");
    expect(screen.getByRole("heading", { name: "Choose the folders to search" })).toBeTruthy();
    await click("Add a folder");
    expect(await screen.findByText(/Folder 3/)).toBeTruthy();
    await click("Start searching");

    expect(finish).toHaveBeenCalled();
    expect(await screen.findByRole("heading", { level: 1, name: "Search" })).toBeTruthy();
  });

  it("can be skipped without choosing a folder", async () => {
    renderWith(createMockEngine({ firstLaunch: true }), <App />);
    await screen.findByRole("button", { name: "Continue" });
    await click("Continue");
    await click("Skip for now");
    expect(await screen.findByRole("heading", { level: 1, name: "Search" })).toBeTruthy();
  });

  it("is not shown once it is done", async () => {
    renderWith(createMockEngine(), <App />);
    await screen.findByRole("button", { name: "Up to date" });
    expect(screen.queryByRole("button", { name: "Continue" })).toBeNull();
  });

  it("asks, in the download from GitHub, whether to check for new versions", async () => {
    const engine = createMockEngine({ firstLaunch: true, updates: true });
    const setUpdateCheck = vi.spyOn(engine, "setUpdateCheck");
    const finish = vi.spyOn(engine, "finishFirstLaunch");
    renderWith(engine, <Welcome status={await engine.status()} />);
    expect(screen.getByText("Step 1 of 3")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("heading", { name: "Check for new versions?" })).toBeTruthy();
    expect(finish).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Don't check" }));
    });
    expect(setUpdateCheck).toHaveBeenCalledWith(false);
    expect(finish).toHaveBeenCalled();
  });
});
