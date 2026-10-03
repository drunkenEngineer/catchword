// The window: a rail with three destinations, and a status chip that opens
// Library (spec, section 8). Search is home.
import { useCallback, useEffect, useState } from "react";
import type { Status } from "./contract/Status";
import { useEngine } from "./engine";
import { Library } from "./Library";
import { Search } from "./Search";
import { Settings } from "./Settings";
import { strings } from "./strings";
import { Welcome } from "./Welcome";

export type Destination = "search" | "library" | "settings";
const DESTINATIONS: Destination[] = ["search", "library", "settings"];

export function App() {
  const engine = useEngine();
  const [destination, setDestination] = useState<Destination>("search");
  const [status, setStatus] = useState<Status | null>(null);

  const refresh = useCallback(() => {
    engine.status().then(setStatus, () => undefined);
  }, [engine]);

  useEffect(() => {
    refresh();
    return engine.onStatusChanged(refresh);
  }, [engine, refresh]);

  // Ctrl+1, Ctrl+2 and Ctrl+3 move between destinations; F5 scans now.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const index = ["1", "2", "3"].indexOf(event.key);
      if (event.ctrlKey && index >= 0) {
        event.preventDefault();
        setDestination(DESTINATIONS[index]);
      } else if (event.key === "F5") {
        event.preventDefault();
        void engine.indexNow();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [engine]);

  // After the first-launch steps, Search opens, even when they follow a
  // "delete all data" in Settings.
  const firstLaunch = status?.firstLaunch ?? false;
  useEffect(() => {
    if (firstLaunch) setDestination("search");
  }, [firstLaunch]);

  if (status?.firstLaunch) return <Welcome status={status} />;

  return (
    <div className="app">
      <nav className="rail" aria-label={strings.navigation}>
        <div className="brand" aria-hidden="true">
          C
        </div>
        {DESTINATIONS.map((name, index) => (
          <button
            key={name}
            type="button"
            className="rail-button"
            aria-current={destination === name ? "page" : undefined}
            aria-keyshortcuts={`Control+${index + 1}`}
            onClick={() => setDestination(name)}
          >
            {strings.destinations[name]}
          </button>
        ))}
      </nav>
      <div className="content">
        <header className="titlebar">
          <h1 className="title">{strings.destinations[destination]}</h1>
          <button type="button" className="chip" onClick={() => setDestination("library")}>
            <span role="status">{chipText(status)}</span>
          </button>
        </header>
        <main className="destination">
          {destination === "search" && (
            <Search status={status} openLibrary={() => setDestination("library")} />
          )}
          {destination === "library" && <Library status={status} />}
          {destination === "settings" && <Settings />}
        </main>
      </div>
    </div>
  );
}

function chipText(status: Status | null): string {
  if (!status || status.meaning.state === "loading") return strings.chip.starting;
  if (status.folders.length === 0) return strings.chip.noFolders;
  if (status.work?.stage === "words") {
    return strings.chip.readingFiles(status.work.done, status.work.total);
  }
  if (status.work?.stage === "meaning" && status.passages > 0) {
    return strings.chip.meaning(Math.floor((100 * status.searchableByMeaning) / status.passages));
  }
  return strings.chip.upToDate;
}
