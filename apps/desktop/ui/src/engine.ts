// The engine client: the only way the interface talks to Catchword.
// It speaks in ids and never sends a file path (spec, section 9).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createContext, useContext } from "react";
import type { Folder } from "./contract/Folder";
import type { ResourceMode } from "./contract/ResourceMode";
import type { SearchResponse } from "./contract/SearchResponse";
import type { SettingsView } from "./contract/SettingsView";
import type { Status } from "./contract/Status";

export interface Engine {
  status(): Promise<Status>;
  search(query: string): Promise<SearchResponse>;
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
  /** The diagnostics report, as plain text, for the user to read. */
  diagnostics(includePaths: boolean): Promise<string>;
  /** Saves the report last shown; the file's name, or null if cancelled. */
  saveDiagnostics(): Promise<string | null>;
  /** The full text of a passage. */
  preview(id: number): Promise<string | null>;
  openFile(id: number): Promise<void>;
  revealFile(id: number): Promise<void>;
  /** Calls back when the status changed. Returns a function that stops it. */
  onStatusChanged(callback: () => void): () => void;
}

/** The real engine, through the shell's commands (apps/desktop/src-tauri). */
export const tauriEngine: Engine = {
  status: () => invoke("status"),
  search: (query) => invoke("search", { query }),
  addFolder: () => invoke("add_folder"),
  removeFolder: (id) => invoke("remove_folder", { id }),
  indexNow: () => invoke("index_now"),
  retryFailed: () => invoke("retry_failed"),
  pauseIndexing: () => invoke("pause_indexing"),
  resumeIndexing: () => invoke("resume_indexing"),
  setResourceMode: (mode) => invoke("set_resource_mode", { mode }),
  settings: () => invoke("settings"),
  excludeFolder: () => invoke("exclude_folder"),
  includeFolder: (id) => invoke("include_folder", { id }),
  setPatterns: (patterns) => invoke("set_patterns", { patterns }),
  finishFirstLaunch: () => invoke("finish_first_launch"),
  deleteAllData: () => invoke("delete_all_data"),
  setDetailedLogs: (on) => invoke("set_detailed_logs", { on }),
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
