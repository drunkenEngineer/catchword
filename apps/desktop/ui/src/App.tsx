// The window: a rail with three destinations, and a status chip that opens
// Library (spec, section 8). Search is home.
import { useCallback, useEffect, useRef, useState } from "react";
import { applyAppearance } from "./appearance";
import type { Status } from "./contract/Status";
import { useEngine } from "./engine";
import { Library } from "./Library";
import { Search } from "./Search";
import { Settings } from "./Settings";
import { UpdateBanner } from "./UpdateBanner";
import { movePane } from "./panes";
import { strings } from "./strings";
import { Welcome } from "./Welcome";

export type Destination = "search" | "library" | "settings";
const DESTINATIONS: Destination[] = ["search", "library", "settings"];

export function App() {
  const engine = useEngine();
  const [destination, setDestination] = useState<Destination>("search");
  const [status, setStatus] = useState<Status | null>(null);
  const title = useRef<HTMLHeadingElement>(null);
  const firstDestination = useRef(true);

  // A new destination takes the focus, so the keyboard carries on there and
  // screen readers say where it is (A11Y-1). Search puts it in its box.
  useEffect(() => {
    if (firstDestination.current) {
      firstDestination.current = false;
      return;
    }
    if (destination !== "search") title.current?.focus();
  }, [destination]);

  // A notice from the start is said once by the engine; it stays here
  // until the user closes it.
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(() => {
    engine.status().then(
      (next) => {
        setStatus(next);
        if (next.notice) setNotice(next.notice);
      },
      () => undefined,
    );
  }, [engine]);

  useEffect(() => {
    refresh();
    return engine.onStatusChanged(refresh);
  }, [engine, refresh]);

  // The saved theme and text size, before anything else is seen.
  useEffect(() => {
    engine.settings().then(
      (view) => applyAppearance(view.theme, view.textSize),
      () => undefined,
    );
  }, [engine]);

  // Ctrl+1, Ctrl+2 and Ctrl+3 move between destinations; F5 scans now;
  // F6 and Shift+F6 move between panes.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const index = ["1", "2", "3"].indexOf(event.key);
      if (event.ctrlKey && index >= 0) {
        event.preventDefault();
        setDestination(DESTINATIONS[index]);
      } else if (event.key === "F5") {
        event.preventDefault();
        void engine.indexNow();
      } else if (event.key === "F6") {
        event.preventDefault();
        movePane(event.shiftKey ? -1 : 1);
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
      <nav className="rail" aria-label={strings.navigation} data-pane>
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
          <h1 className="title" ref={title} tabIndex={-1}>
            {strings.destinations[destination]}
          </h1>
          <button type="button" className="chip" onClick={() => setDestination("library")}>
            {/* Screen readers hear the announcement below instead. */}
            <span aria-hidden="true">{chipText(status)}</span>
            <span className="visually-hidden" role="status">
              {announcement(status)}
            </span>
          </button>
        </header>
        {notice && (
          <div className="start-notice" role="alert">
            <span dir="auto">{notice}</span>
            <button type="button" onClick={() => setNotice(null)}>
              {strings.closeNotice}
            </button>
          </div>
        )}
        {status && <UpdateBanner status={status} />}
        {/* Search marks its own panes; the other destinations are one each. */}
        <main
          className="destination"
          data-pane={destination === "search" ? undefined : ""}
          tabIndex={destination === "search" ? undefined : -1}
        >
          {destination === "search" && (
            <Search status={status} openLibrary={() => setDestination("library")} />
          )}
          {destination === "library" && <Library status={status} />}
          {destination === "settings" && <Settings status={status} />}
        </main>
      </div>
    </div>
  );
}

/**
 * What screen readers hear about the index (A11Y-2): the chip's state, but
 * progress only in steps of ten percent, so they are not read out several
 * times a second.
 */
export function announcement(status: Status | null): string {
  const work = status?.work;
  if (!status || status.paused || !work || work.total === 0) return chipText(status);
  if (work.stage === "words") {
    return strings.chip.readingStep(Math.floor((10 * work.done) / work.total) * 10);
  }
  if (status.passages > 0) {
    return strings.chip.meaning(Math.floor((10 * status.searchableByMeaning) / status.passages) * 10);
  }
  return chipText(status);
}

function chipText(status: Status | null): string {
  if (!status || status.meaning.state === "loading") return strings.chip.starting;
  if (status.folders.length === 0) return strings.chip.noFolders;
  if (status.paused) return strings.chip.paused;
  if (status.work?.stage === "words") {
    return strings.chip.readingFiles(status.work.done, status.work.total);
  }
  if (status.work?.stage === "meaning" && status.passages > 0) {
    return strings.chip.meaning(Math.floor((100 * status.searchableByMeaning) / status.passages));
  }
  return strings.chip.upToDate;
}
