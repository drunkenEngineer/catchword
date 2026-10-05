// The Library screen (UI-3): folders, progress for both stages, index
// facts, and every file that was not indexed, grouped by reason.
import { useState } from "react";
import { Confirm } from "./Confirm";
import type { NotIndexed } from "./contract/NotIndexed";
import type { Status } from "./contract/Status";
import type { Work } from "./contract/Work";
import { useEngine } from "./engine";
import { strings } from "./strings";

export function Library({ status }: { status: Status | null }) {
  const engine = useEngine();
  const [confirming, setConfirming] = useState<number | null>(null);
  // The folder whose removal was just called off: its button takes the focus back.
  const [keptFolder, setKeptFolder] = useState<number | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const fail = (error: unknown) => setProblem(String(error));

  if (!status) return null;
  const words = status.work?.stage === "words" ? status.work : null;
  const meaning = status.work?.stage === "meaning" ? status.work : null;
  const shownProblem = problem ?? status.problem;

  return (
    <div className="library">
      {shownProblem && (
        <p className="problem" role="alert" dir="auto">
          {shownProblem}
        </p>
      )}

      <section aria-labelledby="folders-title">
        <h2 id="folders-title">{strings.library.folders}</h2>
        {status.folders.length === 0 && <p>{strings.library.noFolders}</p>}
        <ul className="folders">
          {status.folders.map((folder) => (
            <li key={folder.id}>
              <span className="folder-name">
                <span dir="auto">{folder.path}</span>
                {folder.state === "scanning" && <span className="muted"> {strings.library.scanning}</span>}
                {folder.state === "offline" && <span className="folder-offline">{strings.library.offline}</span>}
              </span>
              {confirming === folder.id ? (
                <Confirm
                  question={strings.library.confirmRemove}
                  confirm={strings.library.confirm}
                  keep={strings.library.keep}
                  danger
                  onConfirm={() => {
                    setConfirming(null);
                    engine.removeFolder(folder.id).catch(fail);
                  }}
                  onKeep={() => {
                    setConfirming(null);
                    setKeptFolder(folder.id);
                  }}
                />
              ) : (
                <button
                  type="button"
                  autoFocus={keptFolder === folder.id}
                  onClick={() => {
                    setKeptFolder(null);
                    setConfirming(folder.id);
                  }}
                >
                  {strings.library.remove}
                </button>
              )}
            </li>
          ))}
        </ul>
        <div className="actions">
          <button type="button" className="primary" onClick={() => engine.addFolder().catch(fail)}>
            {strings.library.addFolder}
          </button>
          <button type="button" onClick={() => engine.indexNow().catch(fail)} aria-keyshortcuts="F5">
            {strings.library.scanNow}
          </button>
        </div>
      </section>

      <section aria-labelledby="progress-title">
        <h2 id="progress-title">{strings.library.progress}</h2>
        {status.paused && (
          <p role="status">
            {status.paused === "lowDisk"
              ? strings.library.pausedLowDisk
              : status.paused === "battery"
                ? strings.library.pausedBattery
                : status.paused === "indexAway"
                  ? strings.library.pausedIndexAway
                : status.paused === "newerIndex"
                ? strings.library.pausedNewerIndex
                : status.paused === "safeMode"
                  ? strings.library.pausedSafeMode
                  : strings.library.pausedByYou}
          </p>
        )}
        {words ? (
          <>
            <Bar label={strings.library.reading(words.done, words.total)} value={words.done} max={words.total} />
            <Pace work={words} unit="files" />
          </>
        ) : (
          <p>{strings.library.byWords(status.files)}</p>
        )}
        {status.meaning.state === "off" && <p>{strings.library.meaningOff(status.meaning.reason)}</p>}
        {status.meaning.state === "loading" && <p>{strings.library.meaningLoading}</p>}
        {status.meaning.state === "ready" && (
          <Bar
            label={strings.library.byMeaning(status.searchableByMeaning, status.passages)}
            value={status.searchableByMeaning}
            max={Math.max(status.passages, 1)}
          />
        )}
        {meaning && <Pace work={meaning} unit="passages" />}
        {!status.work && status.lastScanSecs !== null && (
          <p className="muted">{strings.library.lastScan(status.lastScanSecs)}</p>
        )}
        {status.folders.length > 0 && (
          <div className="actions">
            {status.paused === "newerIndex" ? (
              <button type="button" className="primary" onClick={() => engine.rebuildIndex().catch(fail)}>
                {strings.library.rebuild}
              </button>
            ) : status.paused ? (
              <>
                <button type="button" className="primary" onClick={() => engine.resumeIndexing().catch(fail)}>
                  {strings.library.resume}
                </button>
                {status.paused === "safeMode" && (
                  <button type="button" onClick={() => engine.rebuildIndex().catch(fail)}>
                    {strings.library.rebuild}
                  </button>
                )}
              </>
            ) : (
              <button type="button" onClick={() => engine.pauseIndexing().catch(fail)}>
                {strings.library.pause}
              </button>
            )}
          </div>
        )}
      </section>

      <section aria-labelledby="attention-title">
        <h2 id="attention-title">{strings.library.attention}</h2>
        {status.notIndexed.length === 0 ? (
          <p>{strings.library.nothingNeedsAttention}</p>
        ) : (
          byReason(status.notIndexed).map(([reason, files]) => (
            <details key={reason} className="reason">
              <summary>
                {reason} ({files.length}, {files[0].failed ? strings.library.failed : strings.library.skipped})
              </summary>
              <ul>
                {files.map((file) => (
                  <li key={`${file.folder}/${file.name}`}>
                    <span dir="auto">{file.name}</span> <span className="file-folder" dir="auto">{file.folder}</span>
                    {file.parked && <span className="parked"> ({strings.library.parked})</span>}
                  </li>
                ))}
              </ul>
            </details>
          ))
        )}
        {status.notIndexed.some((file) => file.failed) && (
          <div className="actions">
            <button type="button" onClick={() => engine.retryFailed().catch(fail)}>
              {strings.library.retry}
            </button>
            <span className="hint">{strings.library.retryHint}</span>
          </div>
        )}
      </section>
    </div>
  );
}

/** How fast the work goes, and the time left, once both are known. */
function Pace({ work, unit }: { work: Work; unit: "files" | "passages" }) {
  if (work.perSecond === null) return null;
  return (
    <p className="muted">
      {strings.library.pace(work.perSecond, unit)}
      {work.secondsLeft !== null && ` · ${strings.library.left(work.secondsLeft)}`}
    </p>
  );
}

function Bar({ label, value, max }: { label: string; value: number; max: number }) {
  return (
    <div className="bar">
      <span>{label}</span>
      <progress value={value} max={max} aria-label={label} />
    </div>
  );
}

/** Files grouped by reason, the largest group first. */
export function byReason(files: NotIndexed[]): [string, NotIndexed[]][] {
  const groups = new Map<string, NotIndexed[]>();
  for (const file of files) {
    groups.set(file.reason, [...(groups.get(file.reason) ?? []), file]);
  }
  return [...groups.entries()].sort((a, b) => b[1].length - a[1].length);
}
