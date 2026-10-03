// The first-launch steps (UI-5, APP-1): the privacy promise, then the
// folders to search. Shown once; Search opens after. Indexing starts as soon
// as a folder is added, so the first search can follow within a minute.
import { useState } from "react";
import type { Status } from "./contract/Status";
import { useEngine } from "./engine";
import { strings } from "./strings";

const STEPS = 2;

export function Welcome({ status }: { status: Status }) {
  const engine = useEngine();
  const [step, setStep] = useState(1);
  const [problem, setProblem] = useState<string | null>(null);
  const fail = (error: unknown) => setProblem(String(error));
  const folders = status.folders;

  return (
    <main className="welcome">
      <p className="muted">{strings.welcome.step(step, STEPS)}</p>
      {step === 1 ? (
        <section aria-labelledby="welcome-title">
          <h1 id="welcome-title">{strings.welcome.promiseTitle}</h1>
          <p>{strings.welcome.promise}</p>
          <ul>
            {strings.welcome.points.map((point) => (
              <li key={point}>{point}</li>
            ))}
          </ul>
          <div className="actions">
            <button type="button" className="primary" autoFocus onClick={() => setStep(2)}>
              {strings.welcome.continue}
            </button>
          </div>
        </section>
      ) : (
        <section aria-labelledby="welcome-title">
          <h1 id="welcome-title">{strings.welcome.foldersTitle}</h1>
          <p>{strings.welcome.foldersText}</p>
          {folders.length > 0 && (
            <ul className="folders">
              {folders.map((folder) => (
                <li key={folder.id}>
                  <span dir="auto">{folder.path}</span>
                </li>
              ))}
            </ul>
          )}
          {problem && (
            <p className="problem" role="alert" dir="auto">
              {problem}
            </p>
          )}
          <div className="actions">
            <button
              type="button"
              className={folders.length === 0 ? "primary" : undefined}
              autoFocus
              onClick={() => engine.addFolder().catch(fail)}
            >
              {folders.length === 0 ? strings.welcome.addFolder : strings.welcome.addAnother}
            </button>
            <button
              type="button"
              className={folders.length > 0 ? "primary" : undefined}
              onClick={() => engine.finishFirstLaunch().catch(fail)}
            >
              {folders.length > 0 ? strings.welcome.start : strings.welcome.skip}
            </button>
          </div>
          <p className="muted">{strings.welcome.leftOut}</p>
        </section>
      )}
    </main>
  );
}
