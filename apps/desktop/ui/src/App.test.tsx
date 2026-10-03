import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
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

  it("reaches its three destinations by keyboard", async () => {
    renderWith(createMockEngine(), <App />);
    fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    expect(heading()).toBe("Library");
    fireEvent.keyDown(window, { key: "3", ctrlKey: true });
    expect(heading()).toBe("Settings");
    fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(heading()).toBe("Search");
  });

  it("shows the index status, which opens Library", async () => {
    renderWith(createMockEngine(), <App />);
    const chip = await screen.findByText("Up to date");
    fireEvent.click(chip);
    expect(heading()).toBe("Library");
  });

  it("scans again on F5, and refreshes when the status changes", async () => {
    const engine = createMockEngine();
    const indexNow = vi.spyOn(engine, "indexNow");
    const status = vi.spyOn(engine, "status");
    renderWith(engine, <App />);
    await screen.findByText("Up to date");
    const before = status.mock.calls.length;
    await act(async () => {
      fireEvent.keyDown(window, { key: "F5" });
    });
    expect(indexNow).toHaveBeenCalledTimes(1);
    expect(status.mock.calls.length).toBeGreaterThan(before);
  });
});
