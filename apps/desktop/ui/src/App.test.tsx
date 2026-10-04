import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { announcement, App } from "./App";
import { createMockEngine } from "./mock";
import { renderWith } from "./test-utils";

afterEach(cleanup);

const heading = () => screen.getByRole("heading", { level: 1 }).textContent;

describe("the window", () => {
  it("reaches its three destinations by mouse", async () => {
    renderWith(createMockEngine(), <App />);
    expect(heading()).toBe("Search");
    fireEvent.click(screen.getByRole("button", { name: "Library" }));
    expect(heading()).toBe("Library");
    expect(screen.getByRole("button", { name: "Library" }).getAttribute("aria-current")).toBe("page");
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(heading()).toBe("Settings");
  });

  it("moves the focus to each destination it opens", async () => {
    renderWith(createMockEngine(), <App />);
    await screen.findByRole("button", { name: "Up to date" });
    fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1, name: "Library" }));
    fireEvent.keyDown(window, { key: "3", ctrlKey: true });
    expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1, name: "Settings" }));
    fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(document.activeElement).toBe(screen.getByRole("searchbox"));
  });

  it("reaches its three destinations by keyboard", async () => {
    renderWith(createMockEngine(), <App />);
    fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    expect(heading()).toBe("Library");
    fireEvent.keyDown(window, { key: "3", ctrlKey: true });
    expect(heading()).toBe("Settings");
    fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(heading()).toBe("Search");
  });

  it("announces progress in steps of ten percent, and shows it exactly", async () => {
    const engine = createMockEngine();
    const idle = await engine.status();
    const reading = (done: number) => ({
      ...idle,
      work: { stage: "words" as const, done, total: 1200, perSecond: null, secondsLeft: null },
    });
    expect(announcement(reading(341))).toBe("Reading files: 20%");
    expect(announcement(reading(359))).toBe("Reading files: 20%");
    expect(announcement(reading(360))).toBe("Reading files: 30%");
    expect(announcement(idle)).toBe("Up to date");

    vi.spyOn(engine, "status").mockResolvedValue(reading(341));
    renderWith(engine, <App />);
    expect(await screen.findByText("Reading files: 341 of 1200")).toBeTruthy();
    expect(screen.getByText("Reading files: 20%").getAttribute("role")).toBe("status");
  });

  it("applies the saved appearance when it starts", async () => {
    const engine = createMockEngine();
    await engine.setAppearance("dark", "large");
    renderWith(engine, <App />);
    await screen.findByRole("button", { name: "Up to date" });
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.dataset.textSize).toBe("large");
    await engine.setAppearance("system", "normal");
    delete document.documentElement.dataset.theme;
    delete document.documentElement.dataset.textSize;
  });

  it("shows when indexing is paused", async () => {
    const engine = createMockEngine();
    renderWith(engine, <App />);
    await screen.findByRole("button", { name: "Up to date" });
    await act(async () => {
      await engine.pauseIndexing();
    });
    expect(await screen.findByRole("button", { name: "Paused" })).toBeTruthy();
  });

  it("shows the index status, which opens Library", async () => {
    renderWith(createMockEngine(), <App />);
    const chip = await screen.findByRole("button", { name: "Up to date" });
    fireEvent.click(chip);
    expect(heading()).toBe("Library");
  });

  it("scans again on F5, and refreshes when the status changes", async () => {
    const engine = createMockEngine();
    const indexNow = vi.spyOn(engine, "indexNow");
    const status = vi.spyOn(engine, "status");
    renderWith(engine, <App />);
    await screen.findByRole("button", { name: "Up to date" });
    const before = status.mock.calls.length;
    await act(async () => {
      fireEvent.keyDown(window, { key: "F5" });
    });
    expect(indexNow).toHaveBeenCalledTimes(1);
    expect(status.mock.calls.length).toBeGreaterThan(before);
  });
});
