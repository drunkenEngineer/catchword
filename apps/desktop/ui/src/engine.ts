// The engine client: the only way the interface talks to Catchword.
// It speaks in ids and never sends a file path (spec, section 9).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createContext, useContext } from "react";
import type { FileAction } from "./contract/FileAction";
import type { Folder } from "./contract/Folder";
import type { IndexFolderChoice } from "./contract/IndexFolderChoice";
import type { UpdateOffer } from "./contract/UpdateOffer";
import type { ResourceMode } from "./contract/ResourceMode";
import type { SearchFilter } from "./contract/SearchFilter";
import type { SearchResponse } from "./contract/SearchResponse";
import type { SettingsView } from "./contract/SettingsView";
import type { TextSize } from "./contract/TextSize";
import type { Theme } from "./contract/Theme";
import type { Status } from "./contract/Status";

export interface Engine {
  status(): Promise<Status>;
  /** `wordsOnly`: by words and names alone, quickly, without the model. */
  search(query: string, filter?: SearchFilter, wordsOnly?: boolean): Promise<SearchResponse>;
  /** Opens the native folder dialog; null when the user cancels. */
  addFolder(): Promise<Folder | null>;
  removeFolder(id: number): Promise<void>;
  indexNow(): Promise<void>;
  /** Reads again the files whose reading failed, parked ones included. */
  retryFailed(): Promise<void>;
  /** Stays paused, even after a restart, until resumed. */
  pauseIndexing(): Promise<void>;
  resumeIndexing(): Promise<void>;
  setResourceMode(mode: ResourceMode): Promise<void>;
  /** Reads every file again; folders and settings stay. */
  rebuildIndex(): Promise<void>;
  /** The full integrity check: true if no damage was found. */
  checkIndex(): Promise<boolean>;
  /** The licences of everything shipped with the app; null if not packaged. */
  notices(): Promise<string | null>;
  setAppearance(theme: Theme, textSize: TextSize): Promise<void>;
  /** Rejects with a reason when a limit is out of range. */
  setLimits(maxFileMb: number, maxPages: number): Promise<void>;
  settings(): Promise<SettingsView>;
  /** Opens the native folder dialog; null when the user cancels. */
  excludeFolder(): Promise<Folder | null>;
  includeFolder(id: number): Promise<void>;
  /** Names to leave out; rejects with a reason when one cannot be used. */
  setPatterns(patterns: string[]): Promise<void>;
  finishFirstLaunch(): Promise<void>;
  /** Deletes the index and the settings. The user's files stay. */
  deleteAllData(): Promise<void>;
  setDetailedLogs(on: boolean): Promise<void>;
  /** Indexing waits while the computer runs on battery (IDX-9). */
  setPauseOnBattery(on: boolean): Promise<void>;
  /** Ask, in the system's folder dialog, where the index should go (APP-7). */
  pickIndexFolder(): Promise<IndexFolderChoice | null>;
  /** Move the index to the folder just picked, or back to its usual place. */
  moveIndex(toUsualPlace: boolean): Promise<void>;
  /** Whether to check for new versions once a day (APP-2, GitHub build). */
  setUpdateCheck(on: boolean): Promise<void>;
  /** Check now; null if this is the newest version. */
  checkForUpdate(): Promise<UpdateOffer | null>;
  /** Download, verify and install the newer version; Catchword restarts. */
  installUpdate(): Promise<void>;
  /** The diagnostics report, as plain text, for the user to read. */
  diagnostics(includePaths: boolean): Promise<string>;
  /** Saves the report last shown; the file's name, or null if cancelled. */
  saveDiagnostics(): Promise<string | null>;
  /** The full text of a passage. */
  preview(id: number): Promise<string | null>;
  /** "missing" if the file moved since the last scan; nothing is opened then. */
  openFile(id: number): Promise<FileAction>;
  revealFile(id: number): Promise<FileAction>;
  /** Calls back when the status changed. Returns a function that stops it. */
  onStatusChanged(callback: () => void): () => void;
}

/** The real engine, through the shell's commands (apps/desktop/src-tauri). */
export const tauriEngine: Engine = {
  status: () => invoke("status"),
  search: (query, filter, wordsOnly) =>
    invoke("search", { query, filter: filter ?? null, wordsOnly: wordsOnly ?? false }),
  addFolder: () => invoke("add_folder"),
  removeFolder: (id) => invoke("remove_folder", { id }),
  indexNow: () => invoke("index_now"),
  retryFailed: () => invoke("retry_failed"),
  pauseIndexing: () => invoke("pause_indexing"),
  resumeIndexing: () => invoke("resume_indexing"),
  setResourceMode: (mode) => invoke("set_resource_mode", { mode }),
  rebuildIndex: () => invoke("rebuild_index"),
  checkIndex: () => invoke("check_index"),
  notices: () => invoke("notices"),
  setAppearance: (theme, textSize) => invoke("set_appearance", { theme, textSize }),
  setLimits: (maxFileMb, maxPages) => invoke("set_limits", { maxFileMb, maxPages }),
  settings: () => invoke("settings"),
  excludeFolder: () => invoke("exclude_folder"),
  includeFolder: (id) => invoke("include_folder", { id }),
  setPatterns: (patterns) => invoke("set_patterns", { patterns }),
  finishFirstLaunch: () => invoke("finish_first_launch"),
  deleteAllData: () => invoke("delete_all_data"),
  setDetailedLogs: (on) => invoke("set_detailed_logs", { on }),
  setPauseOnBattery: (on) => invoke("set_pause_on_battery", { on }),
  pickIndexFolder: () => invoke("pick_index_folder"),
  moveIndex: (toUsualPlace) => invoke("move_index", { toUsualPlace }),
  setUpdateCheck: (on) => invoke("set_update_check", { on }),
  checkForUpdate: () => invoke("check_for_update"),
  installUpdate: () => invoke("install_update"),
  diagnostics: (includePaths) => invoke("diagnostics", { includePaths }),
  saveDiagnostics: () => invoke("save_diagnostics"),
  preview: (id) => invoke("preview", { id }),
  openFile: (id) => invoke("open_file", { id }),
  revealFile: (id) => invoke("reveal_file", { id }),
  onStatusChanged(callback) {
    const stopping = listen("status-changed", () => callback());
    return () => {
      void stopping.then((stop) => stop());
    };
  },
};

export const EngineContext = createContext<Engine | null>(null);

export function useEngine(): Engine {
  const engine = useContext(EngineContext);
  if (!engine) throw new Error("No engine: wrap the app in EngineContext.");
  return engine;
}
