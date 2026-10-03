// The engine client: the only way the interface talks to Catchword.
// It speaks in ids and never sends a file path (spec, section 9).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createContext, useContext } from "react";
import type { Folder } from "./contract/Folder";
import type { SearchResponse } from "./contract/SearchResponse";
import type { Status } from "./contract/Status";

export interface Engine {
  status(): Promise<Status>;
  search(query: string): Promise<SearchResponse>;
  /** Opens the native folder dialog; null when the user cancels. */
  addFolder(): Promise<Folder | null>;
  removeFolder(id: number): Promise<void>;
  indexNow(): Promise<void>;
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
