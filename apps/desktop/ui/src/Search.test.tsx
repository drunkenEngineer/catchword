import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchResponse } from "./contract/SearchResponse";
import type { Status } from "./contract/Status";
import type { Engine } from "./engine";
import { createMockEngine } from "./mock";
import { Search } from "./Search";
import { renderWith } from "./test-utils";

afterEach(cleanup);

let clipboard: string[];
beforeEach(() => {
  clipboard = [];
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: async (text: string) => void clipboard.push(text) },
  });
});

async function show(engine: Engine, query?: string) {
  const status: Status = await engine.status();
  renderWith(engine, <Search status={status} openLibrary={() => undefined} />);
  if (query !== undefined) {
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: query } });
  }
  return status;
}

describe("the search screen", () => {
  it("asks for a folder when there is none", async () => {
    const engine = createMockEngine();
    await engine.removeFolder(1);
    await engine.removeFolder(2);
    const addFolder = vi.spyOn(engine, "addFolder");
    await show(engine);
    fireEvent.click(screen.getByRole("button", { name: "Add a folder" }));
    expect(addFolder).toHaveBeenCalled();
  });

  it("starts ready, with examples and a summary", async () => {
    await show(createMockEngine());
    expect(screen.getByText("3 files ready to search.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "notice period in the lease" }));
    expect((screen.getByRole("searchbox") as HTMLInputElement).value).toBe("notice period in the lease");
  });

  it("shows results grouped by file, with the matching words marked", async () => {
    await show(createMockEngine(), "tax refund");
    const group = await screen.findByRole("group", { name: /tax-refund-letter\.pdf/ });
    expect(group).toBeTruthy();
    const marked = [...document.querySelectorAll("mark")].map((mark) => mark.textContent);
    expect(marked).toEqual(["tax", "refund"]);
    expect(screen.getByText("1 file in 4 ms")).toBeTruthy();
  });

  it("opens, reveals and copies the chosen passage from the keyboard", async () => {
    const engine = createMockEngine();
    const openFile = vi.spyOn(engine, "openFile");
    const revealFile = vi.spyOn(engine, "revealFile");
    await show(engine, "tax amount");
    const list = await screen.findByRole("listbox");
    list.focus();
    fireEvent.keyDown(list, { key: "Enter" });
    expect(openFile).toHaveBeenLastCalledWith(1);
    fireEvent.keyDown(list, { key: "ArrowDown" });
    fireEvent.keyDown(list, { key: "Enter", ctrlKey: true });
    expect(revealFile).toHaveBeenLastCalledWith(2);
    await screen.findByText(/within 10 working days/, { selector: ".preview-text" });
    await act(async () => {
      fireEvent.keyDown(list, { key: "c", ctrlKey: true });
    });
    expect(clipboard[0]).toContain("within 10 working days");
    expect(clipboard[0]).toContain("— tax-refund-letter.pdf, page 2");
  });

  it("shows when each file was last changed", async () => {
    await show(createMockEngine(), "refund");
    expect((await screen.findAllByText(/^changed /)).length).toBeGreaterThan(0);
  });

  it("explains an empty result", async () => {
    await show(createMockEngine(), "zebra");
    expect(await screen.findByText("Nothing found for “zebra”.")).toBeTruthy();
    expect(screen.getByText(/2 files were not indexed/)).toBeTruthy();
  });

  it("shows right-to-left text in its own direction", async () => {
    await show(createMockEngine(), "العقد");
    const snippet = (await screen.findByText("العقد")).closest(".snippet");
    expect(snippet?.getAttribute("dir")).toBe("auto");
  });

  it("never turns document text into markup", async () => {
    const engine = createMockEngine();
    const hostile: SearchResponse = {
      files: [
        {
          name: "<img src=x onerror=alert(1)>.txt",
          folder: "<script>alert(2)</script>",
          copies: 1,
          modifiedSecs: 0,
          passages: [
            {
              id: 9,
              location: "line 1",
              snippet: [{ text: "<b onclick=alert(3)>bold</b>", marked: true }],
              found: "words",
            },
          ],
        },
      ],
      notes: [],
      elapsedMs: 1,
    };
    vi.spyOn(engine, "search").mockResolvedValue(hostile);
    await show(engine, "anything");
    await screen.findByText("<img src=x onerror=alert(1)>.txt", { selector: ".file-name" });
    expect(document.querySelector("img, script, b")).toBeNull();
    expect(screen.getByText("<b onclick=alert(3)>bold</b>").tagName).toBe("MARK");
  });
});
