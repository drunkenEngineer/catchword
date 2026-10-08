// A made-up engine, for working on the interface without the shell
// (`npm run dev:mock`) and for tests (UI-1). Its documents are invented.
import type { FileAction } from "./contract/FileAction";
import type { Folder } from "./contract/Folder";
import type { IndexFolderChoice } from "./contract/IndexFolderChoice";
import type { UpdateOffer } from "./contract/UpdateOffer";
import type { FileHit } from "./contract/FileHit";
import type { PassageHit } from "./contract/PassageHit";
import type { PauseReason } from "./contract/PauseReason";
import type { ResourceMode } from "./contract/ResourceMode";
import type { SearchFilter } from "./contract/SearchFilter";
import type { SearchResponse } from "./contract/SearchResponse";
import type { SettingsView } from "./contract/SettingsView";
import type { Span } from "./contract/Span";
import type { Status } from "./contract/Status";
import type { TextSize } from "./contract/TextSize";
import type { Theme } from "./contract/Theme";
import type { Engine } from "./engine";

interface Document {
  name: string;
  folder: string;
  passages: { id: number; location: string; text: string }[];
}

const DOCUMENTS: Document[] = [
  {
    name: "tax-refund-letter.pdf",
    folder: "C:\\Users\\you\\Documents\\Letters",
    passages: [
      { id: 1, location: "page 1", text: "Dear Ms. Raman, your income tax refund of £412.50 has been approved." },
      { id: 2, location: "page 2", text: "The amount will be paid into your bank account within 10 working days." },
    ],
  },
  {
    name: "tenancy-agreement.txt",
    folder: "C:\\Users\\you\\Documents\\Home",
    passages: [
      { id: 3, location: "lines 4–6", text: "Either party may end this agreement by giving two months' written notice." },
    ],
  },
  {
    name: "عقد-إيجار.txt",
    folder: "C:\\Users\\you\\Documents\\Home",
    passages: [{ id: 4, location: "lines 2–3", text: "يحق لأي من الطرفين إنهاء العقد بإشعار كتابي مدته شهران." }],
  },
];

/** When the made-up documents last changed: March 2025. */
const MODIFIED_SECS = 1_741_000_000;

/** Mark every occurrence of the query's words, as the real engine does. */
function mark(text: string, words: string[]): Span[] {
  const spans: Span[] = [];
  for (const piece of text.split(/(\s+)/)) {
    const bare = piece.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
    spans.push({ text: piece, marked: bare !== "" && words.includes(bare) });
  }
  return spans;
}

/** A few of the real defaults, enough to show. */
const DEFAULT_PATTERNS = ["$RECYCLE.BIN", "Thumbs.db", "node_modules", "*.kdbx", "*.pem", "id_rsa*", "*passwords*"];

/** `firstLaunch`: start as a new install, with no folders. */
/** `updates`: as the download from GitHub, which checks for new versions. */
export function createMockEngine({ firstLaunch = false, updates = false } = {}): Engine {
  let folders: Folder[] = firstLaunch
    ? []
    : [
        { id: 1, path: "C:\\Users\\you\\Documents\\Letters" },
        { id: 2, path: "C:\\Users\\you\\Documents\\Home" },
      ];
  let nextId = 3;
  let parked = true;
  let welcomed = !firstLaunch;
  let excludedFolders: Folder[] = [];
  let patterns = [...DEFAULT_PATTERNS];
  let detailedLogs = false;
  let pauseOnBattery = true;
  const usualPlace = "C:\\Users\\you\\AppData\\Local\\Catchword\\data";
  let dataFolder = usualPlace;
  let picked: string | null = null;
  let paused: PauseReason | null = null;
  let resourceMode: ResourceMode = "balanced";
  let theme: Theme = "system";
  let textSize: TextSize = "normal";
  let maxFileMb = 200;
  let maxPages = 5000;
  let report: string | null = null;
  let updateCheck: boolean | null = null;
  let lastUpdateCheck: number | null = null;
  let offer: UpdateOffer | null = null;
  const listeners = new Set<() => void>();
  const changed = () => listeners.forEach((listener) => listener());

  return {
    async status(): Promise<Status> {
      const passages = DOCUMENTS.flatMap((doc) => doc.passages).length;
      return {
        folders: folders.map((folder) => ({ ...folder, state: "ready" as const })),
        work: null,
        files: folders.length === 0 ? 0 : DOCUMENTS.length,
        passages: folders.length === 0 ? 0 : passages,
        searchableByMeaning: folders.length === 0 ? 0 : passages,
        meaning: { state: "ready" },
        notIndexed:
          folders.length === 0
            ? []
            : [
                {
                  name: "scan_0042.pdf",
                  folder: "C:\\Users\\you\\Documents\\Letters",
                  reason: "no text layer (a scan?); needs text recognition, not available yet",
                  code: "needs-ocr",
                  failed: false,
                  parked: false,
                },
                {
                  name: "statement-2019.pdf",
                  folder: "C:\\Users\\you\\Documents\\Bank",
                  reason: "the reader crashed on this file",
                  code: "crashed",
                  failed: true,
                  parked,
                },
              ],
        problem: null,
        notice: null,
        updates: { inBuild: updates, check: updateCheck, lastCheckSecs: lastUpdateCheck, offer },
        firstLaunch: !welcomed,
        paused,
        lastScanSecs: folders.length === 0 ? null : 1_759_500_000,
      };
    },

    async search(query: string, filter?: SearchFilter): Promise<SearchResponse> {
      const folder = folders.find((f) => f.id === filter?.folder)?.path;
      const kindOf = (name: string) => (name.toLowerCase().endsWith(".pdf") ? "pdf" : "text");
      const words = query
        .toLowerCase()
        .split(/\s+/)
        .map((word) => word.replace(/[^\p{L}\p{N}]/gu, ""))
        .filter((word) => word !== "");
      const files: FileHit[] = [];
      for (const doc of folders.length === 0 ? [] : DOCUMENTS) {
        if (folder !== undefined && doc.folder !== folder) continue;
        if (filter?.kind && kindOf(doc.name) !== filter.kind) continue;
        if (filter?.changed) {
          const days = { pastWeek: 7, pastMonth: 31, pastYear: 366 }[filter.changed];
          if (MODIFIED_SECS < Date.now() / 1000 - days * 24 * 60 * 60) continue;
        }
        const passages: PassageHit[] = doc.passages
          .map((passage) => ({ ...passage, snippet: mark(passage.text, words) }))
          .filter((passage) => passage.snippet.some((span) => span.marked))
          .map(({ id, location, snippet }) => ({ id, location, snippet, found: "words" }));
        if (passages.length > 0) {
          const path = `${doc.folder}\\${doc.name}`;
          files.push({ name: doc.name, folder: doc.folder, path, copies: 1, modifiedSecs: MODIFIED_SECS, passages });
        }
      }
      return { files, notes: [], elapsedMs: 4 };
    },

    async addFolder(): Promise<Folder | null> {
      const folder = { id: nextId, path: `C:\\Users\\you\\Documents\\Folder ${nextId}` };
      nextId += 1;
      folders = [...folders, folder];
      changed();
      return folder;
    },

    async removeFolder(id: number): Promise<void> {
      folders = folders.filter((folder) => folder.id !== id);
      changed();
    },

    async indexNow(): Promise<void> {
      changed();
    },

    async pauseIndexing(): Promise<void> {
      paused = "you";
      changed();
    },

    async resumeIndexing(): Promise<void> {
      paused = null;
      changed();
    },

    async setResourceMode(mode: ResourceMode): Promise<void> {
      resourceMode = mode;
    },

    async rebuildIndex(): Promise<void> {
      if (paused === "newerIndex") paused = null;
      changed();
    },

    async checkIndex(): Promise<boolean> {
      return true;
    },

    async setLimits(nextMb: number, nextPages: number): Promise<void> {
      if (nextMb < 1 || nextMb > 2048) throw "The largest file must be between 1 and 2048 MB.";
      if (nextPages < 1 || nextPages > 100000) throw "The most pages must be between 1 and 100000.";
      maxFileMb = nextMb;
      maxPages = nextPages;
    },

    async setAppearance(nextTheme: Theme, nextSize: TextSize): Promise<void> {
      theme = nextTheme;
      textSize = nextSize;
    },

    async notices(): Promise<string | null> {
      return "Third-party notices for Catchword\n\nreact 19.3.0 (MIT)\n...";
    },

    async retryFailed(): Promise<void> {
      parked = false;
      changed();
    },

    async settings(): Promise<SettingsView> {
      return {
        excludedFolders: [...excludedFolders],
        patterns: [...patterns],
        defaultPatterns: [...DEFAULT_PATTERNS],
        dataFolder,
        indexBytes: folders.length === 0 ? 4096 : 18_350_080,
        detailedLogs,
        version: "0.0.1",
        resourceMode,
        dataSyncedBy: null,
        theme,
        textSize,
        maxFileMb,
        maxPages,
        pauseOnBattery,
        indexMoved: dataFolder !== usualPlace,
      };
    },

    async excludeFolder(): Promise<Folder | null> {
      const folder = { id: nextId, path: `C:\\Users\\you\\Documents\\Home\\Private ${nextId}` };
      nextId += 1;
      excludedFolders = [...excludedFolders, folder];
      changed();
      return folder;
    },

    async includeFolder(id: number): Promise<void> {
      excludedFolders = excludedFolders.filter((folder) => folder.id !== id);
      changed();
    },

    async setPatterns(next: string[]): Promise<void> {
      const kept = [...new Set(next.map((pattern) => pattern.trim()).filter((pattern) => pattern !== ""))];
      const bad = kept.find((pattern) => /[\\/]/.test(pattern));
      if (bad) {
        throw `“${bad}”: A pattern matches names, so it cannot contain / or \\. To leave out a folder, choose it instead.`;
      }
      patterns = kept;
      changed();
    },

    async finishFirstLaunch(): Promise<void> {
      welcomed = true;
      changed();
    },

    async setDetailedLogs(on: boolean): Promise<void> {
      detailedLogs = on;
    },

    async setUpdateCheck(on: boolean): Promise<void> {
      updateCheck = on;
      if (!on) offer = null;
      changed();
    },

    async checkForUpdate(): Promise<UpdateOffer | null> {
      lastUpdateCheck = Math.floor(Date.now() / 1000);
      offer = { version: "0.2.0", notes: "Word documents, and faster search." };
      changed();
      return offer;
    },

    async installUpdate(): Promise<void> {},

    async setPauseOnBattery(on: boolean): Promise<void> {
      pauseOnBattery = on;
    },

    async pickIndexFolder(): Promise<IndexFolderChoice | null> {
      picked = "D:\\Private\\Catchword index";
      return { path: picked, syncedBy: null };
    },

    async moveIndex(toUsualPlace: boolean): Promise<void> {
      if (!toUsualPlace && picked === null) throw "choose a folder for the index first";
      dataFolder = toUsualPlace ? usualPlace : (picked as string);
      picked = null;
      changed();
    },

    async diagnostics(includePaths: boolean): Promise<string> {
      report = [
        "Catchword diagnostics report",
        "",
        "Read this before you share it. It holds no document text and no searches.",
        includePaths ? "File and folder names: included, because you asked for them." : "File and folder names: left out.",
        "",
        "Index",
        `  Files: ${folders.length === 0 ? 0 : DOCUMENTS.length}`,
        ...(includePaths ? folders.map((folder) => `    ${folder.path}`) : []),
      ].join("\n");
      return report;
    },

    async saveDiagnostics(): Promise<string | null> {
      if (report === null) throw "Prepare the report first.";
      return "catchword-diagnostics.txt";
    },

    async deleteAllData(): Promise<void> {
      detailedLogs = false;
      report = null;
      folders = [];
      excludedFolders = [];
      patterns = [...DEFAULT_PATTERNS];
      welcomed = false;
      changed();
    },

    async preview(id: number): Promise<string | null> {
      const passage = DOCUMENTS.flatMap((doc) => doc.passages).find((p) => p.id === id);
      return passage?.text ?? null;
    },

    async openFile(): Promise<FileAction> {
      return "done";
    },
    async revealFile(): Promise<FileAction> {
      return "done";
    },

    onStatusChanged(callback) {
      listeners.add(callback);
      return () => listeners.delete(callback);
    },
  };
}
