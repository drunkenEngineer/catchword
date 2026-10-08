// What the GitHub build says about updates (APP-2, ADR-24). It asks once
// whether to check, if the first launch did not, and offers a newer version
// when one is found. Nothing is checked or installed without the user's say.
// Release notes come from the release and are shown as plain text only.
import { useState } from "react";
import type { Status } from "./contract/Status";
import { useEngine } from "./engine";
import { strings } from "./strings";

export function UpdateBanner({ status }: { status: Status }) {
  const engine = useEngine();
  // The version the user put off for now.
  const [later, setLater] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const updates = status.updates;
  if (!updates.inBuild || status.firstLaunch) return null;

  if (updates.check === null) {
    return (
      <div className="start-notice" role="region" aria-label={strings.updates.askTitle}>
        <strong>{strings.updates.askTitle}</strong>
        <span>{strings.updates.ask}</span>
        <button type="button" onClick={() => engine.setUpdateCheck(true).catch((e) => setProblem(String(e)))}>
          {strings.updates.yes}
        </button>
        <button type="button" onClick={() => engine.setUpdateCheck(false).catch((e) => setProblem(String(e)))}>
          {strings.updates.no}
        </button>
        {problem && (
          <span className="problem" role="alert" dir="auto">
            {problem}
          </span>
        )}
      </div>
    );
  }

  const offer = updates.offer;
  if (!offer || later === offer.version) return null;
  const title = strings.updates.available(offer.version);
  return (
    <div className="start-notice" role="region" aria-label={title}>
      <strong>{title}</strong>
      {offer.notes && (
        <span className="update-notes" dir="auto">
          {offer.notes}
        </span>
      )}
      {installing ? (
        <span role="status">{strings.updates.installing}</span>
      ) : (
        <>
          <button
            type="button"
            className="primary"
            onClick={() => {
              setInstalling(true);
              setProblem(null);
              engine.installUpdate().catch((error) => {
                setInstalling(false);
                setProblem(String(error));
              });
            }}
          >
            {strings.updates.install}
          </button>
          <button type="button" onClick={() => setLater(offer.version)}>
            {strings.updates.later}
          </button>
        </>
      )}
      {problem && (
        <span className="problem" role="alert" dir="auto">
          {problem}
        </span>
      )}
    </div>
  );
}
