// The Library screen (UI-3): folders, progress for both stages, index
// facts, and every file that was not indexed, grouped by reason.
import { useState } from "react";
import type { NotIndexed } from "./contract/NotIndexed";
import type { Status } from "./contract/Status";
import { useEngine } from "./engine";
import { strings } from "./strings";

export function Library({ status }: { status: Status | null }) {
  const engine = useEngine();
  const [confirming, setConfirming] = useState<number | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const fail = (error: unknown) => setProblem(String(error));

  if (!status) return null;
  const words = status.work?.stage === "words" ? status.work : null;
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
              <span dir="auto">{folder.path}</span>
              {confirming === folder.id ? (
                <span className="confirm">
                  <span>{strings.library.confirmRemove}</span>
                  <button
                    type="button"
                    className="danger"
                    onClick={() => {
                      setConfirming(null);
                      engine.removeFolder(folder.id).catch(fail);
                    }}
                  >
                    {strings.library.confirm}
                  </button>
                  <button type="button" onClick={() => setConfirming(null)}>
                    {strings.library.keep}
                  </button>
                </span>
              ) : (
                <button type="button" onClick={() => setConfirming(folder.id)}>
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
        {words ? (
          <Bar label={strings.library.reading(words.done, words.total)} value={words.done} max={words.total} />
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
                  </li>
                ))}
              </ul>
            </details>
          ))
        )}
      </section>
    </div>
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
