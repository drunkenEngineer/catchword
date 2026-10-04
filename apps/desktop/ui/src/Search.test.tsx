import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchResponse } from "./contract/SearchResponse";
import type { Status } from "./contract/Status";
import type { Engine } from "./engine";
import { createMockEngine } from "./mock";
import { indexingNotice, Search } from "./Search";
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

  it("says how far indexing has got, so how complete results are", async () => {
    const status = await createMockEngine().status();
    const words = { stage: "words" as const, done: 1240, total: 8300, perSecond: null, secondsLeft: null };
    // Numbers are written as the computer's language writes them.
    const n = (number: number) => number.toLocaleString();
    // A new index, or one being rebuilt, has no finished scan yet.
    expect(indexingNotice({ ...status, work: words, lastScanSecs: null })).toBe(
      `Reading your files: ${n(1240)} of ${n(8300)}. Results appear as each file is read.`,
    );
    expect(indexingNotice({ ...status, work: words, lastScanSecs: 1_759_000_000 })).toBe(
      `Checking your files for changes: ${n(1240)} of ${n(8300)}. Results may be incomplete.`,
    );
    const meaning = { ...words, stage: "meaning" as const };
    expect(indexingNotice({ ...status, work: meaning, searchableByMeaning: 400, passages: 4000 })).toBe(
      `Searchable by meaning: 400 of ${n(4000)} passages. Results may be incomplete.`,
    );
    expect(indexingNotice({ ...status, work: null })).toBeNull();
  });

  it("says it is searching when a search takes a while", async () => {
    const engine = createMockEngine();
    let finish: () => void = () => undefined;
    const search = engine.search.bind(engine);
    vi.spyOn(engine, "search").mockImplementation(
      (query, filter) => new Promise((resolve) => (finish = () => resolve(search(query, filter)))),
    );
    await show(engine, "tax");
    expect(await screen.findByText("Searching…", {}, { timeout: 2000 })).toBeTruthy();
    await act(async () => finish());
    expect(await screen.findByRole("listbox")).toBeTruthy();
    expect(screen.queryByText("Searching…")).toBeNull();
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

  it("folds a file's passages to its best one with Left, and unfolds them with Right", async () => {
    await show(createMockEngine(), "tax amount");
    const list = await screen.findByRole("listbox");
    list.focus();
    expect(within(list).getAllByRole("option")).toHaveLength(2);
    fireEvent.keyDown(list, { key: "ArrowDown" });
    fireEvent.keyDown(list, { key: "ArrowLeft" });
    const options = within(list).getAllByRole("option");
    expect(options).toHaveLength(1);
    // The selection stays on the file, on its best passage.
    expect(options[0].getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("group", { name: /1 more passage/ })).toBeTruthy();
    fireEvent.keyDown(list, { key: "ArrowRight" });
    expect(within(list).getAllByRole("option")).toHaveLength(2);
    expect(screen.queryByText(/more passage/)).toBeNull();
    // By mouse, on the file's name.
    fireEvent.click(screen.getByText("tax-refund-letter.pdf", { selector: ".file-name" }));
    expect(within(list).getAllByRole("option")).toHaveLength(1);
  });

  it("copies the chosen file's path, by button or Ctrl+Shift+C", async () => {
    await show(createMockEngine(), "tax amount");
    const list = await screen.findByRole("listbox");
    list.focus();
    const path = "C:\\Users\\you\\Documents\\Letters\\tax-refund-letter.pdf";
    await act(async () => {
      fireEvent.keyDown(list, { key: "C", ctrlKey: true, shiftKey: true });
    });
    expect(clipboard).toEqual([path]);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy path" }));
    });
    expect(clipboard).toEqual([path, path]);
  });

  it("says when a result's file has moved, and offers to scan again", async () => {
    const engine = createMockEngine();
    vi.spyOn(engine, "openFile").mockResolvedValue("missing");
    const indexNow = vi.spyOn(engine, "indexNow");
    await show(engine, "notice");
    const results = await screen.findByRole("listbox", { name: "Results" });
    await act(async () => {
      fireEvent.keyDown(results, { key: "Enter" });
    });
    const notice = await screen.findByRole("alert");
    expect(notice.textContent).toContain("no longer where it was");
    fireEvent.click(screen.getByRole("button", { name: "Scan now" }));
    expect(indexNow).toHaveBeenCalled();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps to one folder or one kind of file when asked", async () => {
    const engine = createMockEngine();
    const search = vi.spyOn(engine, "search");
    await show(engine, "notice");
    await screen.findAllByText("tenancy-agreement.txt");
    fireEvent.change(screen.getByRole("combobox", { name: "Kind" }), { target: { value: "pdf" } });
    await waitFor(() => expect(screen.queryAllByText("tenancy-agreement.txt")).toHaveLength(0));
    expect(search).toHaveBeenLastCalledWith("notice", { folder: null, kind: "pdf" });

    fireEvent.change(screen.getByRole("combobox", { name: "Kind" }), { target: { value: "" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Folder" }), { target: { value: "2" } });
    await waitFor(() => expect(search).toHaveBeenLastCalledWith("notice", { folder: 2, kind: null }));
    expect((await screen.findAllByText("tenancy-agreement.txt")).length).toBeGreaterThan(0);
  });

  it("shows when each file was last changed", async () => {
    await show(createMockEngine(), "refund");
    expect((await screen.findAllByText(/^changed /)).length).toBeGreaterThan(0);
  });

  it("explains an empty result with its likely causes, counted", async () => {
    await show(createMockEngine(), "zebra");
    expect(await screen.findByText("Nothing found for “zebra”.")).toBeTruthy();
    expect(screen.getByText(/1 file is a scan with no text layer/)).toBeTruthy();
    expect(screen.getByText(/1 file could not be read/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Search all folders and kinds" })).toBeNull();
  });

  it("offers to search everything when a filter found nothing", async () => {
    const engine = createMockEngine();
    const search = vi.spyOn(engine, "search");
    await show(engine, "zebra");
    fireEvent.change(screen.getByRole("combobox", { name: "Kind" }), { target: { value: "pdf" } });
    expect(await screen.findByText(/because of the filters above/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Search all folders and kinds" }));
    await waitFor(() => expect(search).toHaveBeenLastCalledWith("zebra", { folder: null, kind: null }));
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
          path: "<script>alert(2)</script>/<img src=x onerror=alert(1)>.txt",
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
