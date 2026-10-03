// A made-up engine, for working on the interface without the shell
// (`npm run dev:mock`) and for tests (UI-1). Its documents are invented.
import type { Folder } from "./contract/Folder";
import type { FileHit } from "./contract/FileHit";
import type { PassageHit } from "./contract/PassageHit";
import type { SearchResponse } from "./contract/SearchResponse";
import type { SettingsView } from "./contract/SettingsView";
import type { Span } from "./contract/Span";
import type { Status } from "./contract/Status";
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
export function createMockEngine({ firstLaunch = false } = {}): Engine {
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
  const listeners = new Set<() => void>();
  const changed = () => listeners.forEach((listener) => listener());

  return {
    async status(): Promise<Status> {
      const passages = DOCUMENTS.flatMap((doc) => doc.passages).length;
      return {
        folders: [...folders],
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
                  failed: false,
                  parked: false,
                },
                {
                  name: "statement-2019.pdf",
                  folder: "C:\\Users\\you\\Documents\\Bank",
                  reason: "the reader crashed on this file",
                  failed: true,
                  parked,
                },
              ],
        problem: null,
        firstLaunch: !welcomed,
      };
    },

    async search(query: string): Promise<SearchResponse> {
      const words = query
        .toLowerCase()
        .split(/\s+/)
        .map((word) => word.replace(/[^\p{L}\p{N}]/gu, ""))
        .filter((word) => word !== "");
      const files: FileHit[] = [];
      for (const doc of folders.length === 0 ? [] : DOCUMENTS) {
        const passages: PassageHit[] = doc.passages
          .map((passage) => ({ ...passage, snippet: mark(passage.text, words) }))
          .filter((passage) => passage.snippet.some((span) => span.marked))
          .map(({ id, location, snippet }) => ({ id, location, snippet, found: "words" }));
        if (passages.length > 0) {
          files.push({ name: doc.name, folder: doc.folder, copies: 1, passages });
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

    async retryFailed(): Promise<void> {
      parked = false;
      changed();
    },

    async settings(): Promise<SettingsView> {
      return {
        excludedFolders: [...excludedFolders],
        patterns: [...patterns],
        defaultPatterns: [...DEFAULT_PATTERNS],
        dataFolder: "C:\\Users\\you\\AppData\\Local\\Catchword\\data",
        indexBytes: folders.length === 0 ? 4096 : 18_350_080,
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

    async deleteAllData(): Promise<void> {
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

    async openFile(): Promise<void> {},
    async revealFile(): Promise<void> {},

    onStatusChanged(callback) {
      listeners.add(callback);
      return () => listeners.delete(callback);
    },
  };
}
