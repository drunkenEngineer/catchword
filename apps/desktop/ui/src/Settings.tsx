// The Settings screen (UI-4, APP-3): what to leave out, where the data is
// and how to delete it, and the privacy statement. Every change is saved at
// once and takes effect without a restart.
import { useCallback, useEffect, useState } from "react";
import { applyAppearance } from "./appearance";
import { Confirm } from "./Confirm";
import type { IndexFolderChoice } from "./contract/IndexFolderChoice";
import type { ResourceMode } from "./contract/ResourceMode";
import type { TextSize } from "./contract/TextSize";
import type { Theme } from "./contract/Theme";
import type { SettingsView } from "./contract/SettingsView";
import { useEngine } from "./engine";
import { strings } from "./strings";

export function Settings() {
  const engine = useEngine();
  const [view, setView] = useState<SettingsView | null>(null);
  // The names being edited; null while they match what is saved.
  const [draft, setDraft] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [confirmingRebuild, setConfirmingRebuild] = useState(false);
  // Which question was just called off: its button takes the focus back.
  const [kept, setKept] = useState<"rebuild" | "delete" | "move" | "moveBack" | null>(null);
  // Where the index would go, while the user reads the warning (APP-7).
  const [moving, setMoving] = useState<IndexFolderChoice | "usualPlace" | null>(null);
  const [moveNote, setMoveNote] = useState<string | null>(null);
  const [checked, setChecked] = useState<boolean | null>(null);
  // The limits being edited; null while they match what is saved.
  const [limits, setLimits] = useState<{ mb: string; pages: string } | null>(null);
  const [limitsSaved, setLimitsSaved] = useState(false);
  // undefined: not asked for yet; null: none shipped with this build.
  const [notices, setNotices] = useState<string | null | undefined>(undefined);
  const [includePaths, setIncludePaths] = useState(false);
  const [report, setReport] = useState<string | null>(null);
  const [reportSaved, setReportSaved] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const fail = (error: unknown) => setProblem(String(error));

  const load = useCallback(() => {
    engine.settings().then(setView, (error: unknown) => setProblem(String(error)));
  }, [engine]);

  useEffect(() => {
    load();
    return engine.onStatusChanged(load);
  }, [engine, load]);

  if (!view) return null;
  const patterns = draft ?? view.patterns.join("\n");

  const savePatterns = () => {
    setProblem(null);
    engine.setPatterns(patterns.split("\n")).then(() => {
      setDraft(null);
      setSaved(true);
      load();
    }, fail);
  };

  return (
    <div className="settings">
      {problem && (
        <p className="problem" role="alert" dir="auto">
          {problem}
        </p>
      )}

      <section aria-labelledby="leave-out-title">
        <h2 id="leave-out-title">{strings.settings.leaveOutTitle}</h2>
        <p>{strings.settings.leaveOutText}</p>

        <h3>{strings.settings.excludedFolders}</h3>
        {view.excludedFolders.length === 0 && <p className="muted">{strings.settings.noExcludedFolders}</p>}
        <ul className="folders">
          {view.excludedFolders.map((folder) => (
            <li key={folder.id}>
              <span dir="auto">{folder.path}</span>
              <button type="button" onClick={() => engine.includeFolder(folder.id).then(load, fail)}>
                {strings.settings.includeFolder}
              </button>
            </li>
          ))}
        </ul>
        <div className="actions">
          <button type="button" onClick={() => engine.excludeFolder().then(load, fail)}>
            {strings.settings.excludeFolder}
          </button>
        </div>

        <label htmlFor="patterns" className="label">
          {strings.settings.patterns}
        </label>
        <textarea
          id="patterns"
          className="patterns"
          rows={8}
          spellCheck={false}
          value={patterns}
          onChange={(event) => {
            setDraft(event.target.value);
            setSaved(false);
          }}
        />
        <div className="actions">
          <button type="button" className="primary" disabled={draft === null} onClick={savePatterns}>
            {strings.settings.save}
          </button>
          <button
            type="button"
            onClick={() => {
              setDraft(view.defaultPatterns.join("\n"));
              setSaved(false);
            }}
          >
            {strings.settings.restoreDefaults}
          </button>
          {saved && <span role="status">{strings.settings.saved}</span>}
        </div>
      </section>

      <section aria-labelledby="appearance-title">
        <h2 id="appearance-title">{strings.settings.appearanceTitle}</h2>
        <fieldset className="modes">
          <legend>{strings.settings.theme}</legend>
          {(["system", "light", "dark"] as Theme[]).map((theme) => (
            <label key={theme} className="check">
              <input
                type="radio"
                name="theme"
                checked={view.theme === theme}
                onChange={() => {
                  applyAppearance(theme, view.textSize);
                  engine.setAppearance(theme, view.textSize).then(load, fail);
                }}
              />
              {strings.settings.themes[theme]}
            </label>
          ))}
        </fieldset>
        <fieldset className="modes">
          <legend>{strings.settings.textSize}</legend>
          {(["normal", "large", "larger"] as TextSize[]).map((size) => (
            <label key={size} className="check">
              <input
                type="radio"
                name="text-size"
                checked={view.textSize === size}
                onChange={() => {
                  applyAppearance(view.theme, size);
                  engine.setAppearance(view.theme, size).then(load, fail);
                }}
              />
              {strings.settings.textSizes[size]}
            </label>
          ))}
        </fieldset>
      </section>

      <section aria-labelledby="indexing-title">
        <h2 id="indexing-title">{strings.settings.indexingTitle}</h2>
        <fieldset className="modes">
          <legend>{strings.settings.resourceMode}</legend>
          {(["light", "balanced", "fast"] as ResourceMode[]).map((mode) => (
            <label key={mode} className="check">
              <input
                type="radio"
                name="resource-mode"
                value={mode}
                checked={view.resourceMode === mode}
                onChange={() => engine.setResourceMode(mode).then(load, fail)}
              />
              <span>
                <strong>{strings.settings.modes[mode][0]}</strong> {strings.settings.modes[mode][1]}
              </span>
            </label>
          ))}
        </fieldset>
        <p className="muted">{strings.settings.modeNote}</p>
        <label className="check">
          <input
            type="checkbox"
            checked={view.pauseOnBattery}
            onChange={(event) => engine.setPauseOnBattery(event.target.checked).then(load, fail)}
          />
          {strings.settings.pauseOnBattery}
        </label>
        <div className="limits">
          <label>
            {strings.settings.maxFileMb}
            <input
              type="number"
              min={1}
              max={2048}
              value={limits?.mb ?? String(view.maxFileMb)}
              onChange={(event) => {
                setLimits({ mb: event.target.value, pages: limits?.pages ?? String(view.maxPages) });
                setLimitsSaved(false);
              }}
            />
          </label>
          <label>
            {strings.settings.maxPages}
            <input
              type="number"
              min={1}
              max={100000}
              value={limits?.pages ?? String(view.maxPages)}
              onChange={(event) => {
                setLimits({ mb: limits?.mb ?? String(view.maxFileMb), pages: event.target.value });
                setLimitsSaved(false);
              }}
            />
          </label>
        </div>
        <div className="actions">
          <button
            type="button"
            disabled={limits === null}
            onClick={() => {
              if (!limits) return;
              setProblem(null);
              engine.setLimits(Number(limits.mb), Number(limits.pages)).then(() => {
                setLimits(null);
                setLimitsSaved(true);
                load();
              }, fail);
            }}
          >
            {strings.settings.saveLimits}
          </button>
          {limitsSaved && <span role="status">{strings.settings.limitsSaved}</span>}
        </div>
      </section>

      <section aria-labelledby="data-title">
        <h2 id="data-title">{strings.settings.dataTitle}</h2>
        <p dir="auto">{strings.settings.dataPlace(view.dataFolder, strings.bytes(view.indexBytes))}</p>
        {view.dataSyncedBy && (
          <p className="problem" role="alert">
            {strings.settings.syncedWarning(view.dataSyncedBy)}
          </p>
        )}
        {moving ? (
          <Confirm
            question={
              moving === "usualPlace"
                ? strings.settings.confirmMoveBack
                : strings.settings.confirmMove(moving.path, moving.syncedBy)
            }
            confirm={strings.settings.move}
            keep={strings.settings.keep}
            onConfirm={() => {
              const toUsualPlace = moving === "usualPlace";
              setMoving(null);
              setProblem(null);
              setMoveNote(strings.settings.moving);
              engine.moveIndex(toUsualPlace).then(
                () => {
                  setMoveNote(strings.settings.moved);
                  load();
                },
                (error: unknown) => {
                  setMoveNote(null);
                  fail(error);
                },
              );
            }}
            onKeep={() => {
              setKept(moving === "usualPlace" ? "moveBack" : "move");
              setMoving(null);
            }}
          />
        ) : (
          <div className="actions">
            <button
              type="button"
              autoFocus={kept === "move"}
              onClick={() => {
                setKept(null);
                setMoveNote(null);
                engine.pickIndexFolder().then((choice) => choice && setMoving(choice), fail);
              }}
            >
              {strings.settings.moveIndex}
            </button>
            {view.indexMoved && (
              <button
                type="button"
                autoFocus={kept === "moveBack"}
                onClick={() => {
                  setKept(null);
                  setMoveNote(null);
                  setMoving("usualPlace");
                }}
              >
                {strings.settings.moveBack}
              </button>
            )}
            {moveNote && <span role="status">{moveNote}</span>}
          </div>
        )}
        <div className="actions">
          <button type="button" onClick={() => engine.checkIndex().then(setChecked, fail)}>
            {strings.settings.checkIndex}
          </button>
          {checked !== null && (
            <span role="status" className={checked ? undefined : "problem"}>
              {checked ? strings.settings.indexSound : strings.settings.indexDamaged}
            </span>
          )}
        </div>
        {confirmingRebuild ? (
          <Confirm
            question={strings.settings.confirmRebuild}
            confirm={strings.settings.rebuild}
            keep={strings.settings.keep}
            onConfirm={() => {
              setConfirmingRebuild(false);
              setChecked(null);
              engine.rebuildIndex().then(load, fail);
            }}
            onKeep={() => {
              setConfirmingRebuild(false);
              setKept("rebuild");
            }}
          />
        ) : (
          <div className="actions">
            <button
              type="button"
              autoFocus={kept === "rebuild"}
              onClick={() => {
                setKept(null);
                setConfirmingRebuild(true);
              }}
            >
              {strings.settings.rebuild}
            </button>
          </div>
        )}
        {confirmingDelete ? (
          <Confirm
            question={strings.settings.confirmDeleteAll}
            confirm={strings.settings.deleteAll}
            keep={strings.settings.keep}
            danger
            onConfirm={() => {
              setConfirmingDelete(false);
              engine.deleteAllData().catch(fail);
            }}
            onKeep={() => {
              setConfirmingDelete(false);
              setKept("delete");
            }}
          />
        ) : (
          <button
            type="button"
            className="danger"
            autoFocus={kept === "delete"}
            onClick={() => {
              setKept(null);
              setConfirmingDelete(true);
            }}
          >
            {strings.settings.deleteAll}
          </button>
        )}
      </section>

      <section aria-labelledby="diagnostics-title">
        <h2 id="diagnostics-title">{strings.settings.diagnosticsTitle}</h2>
        <p>{strings.settings.version(view.version)}</p>
        <label className="check">
          <input
            type="checkbox"
            checked={view.detailedLogs}
            onChange={(event) => engine.setDetailedLogs(event.target.checked).then(load, fail)}
          />
          {strings.settings.detailedLogs}
        </label>
        <p className="muted">{strings.settings.logsNote}</p>
        <label className="check">
          <input
            type="checkbox"
            checked={includePaths}
            onChange={(event) => {
              setIncludePaths(event.target.checked);
              setReport(null);
            }}
          />
          {strings.settings.includePaths}
        </label>
        <div className="actions">
          <button
            type="button"
            onClick={() => {
              setReportSaved(null);
              engine.diagnostics(includePaths).then(setReport, fail);
            }}
          >
            {strings.settings.prepareReport}
          </button>
        </div>
        {report !== null && (
          <>
            <h3 id="report-title">{strings.settings.reportTitle}</h3>
            <pre className="report" aria-labelledby="report-title" tabIndex={0}>
              {report}
            </pre>
            <div className="actions">
              <button type="button" className="primary" onClick={() => engine.saveDiagnostics().then(setReportSaved, fail)}>
                {strings.settings.saveReport}
              </button>
              {reportSaved && <span role="status">{strings.settings.reportSaved(reportSaved)}</span>}
            </div>
          </>
        )}
      </section>

      <section aria-labelledby="about-title">
        <h2 id="about-title">{strings.settings.aboutTitle}</h2>
        <p>
          {strings.settings.version(view.version)}. {strings.settings.licence}
        </p>
        {notices === undefined ? (
          <div className="actions">
            <button type="button" onClick={() => engine.notices().then(setNotices, fail)}>
              {strings.settings.showNotices}
            </button>
          </div>
        ) : notices === null ? (
          <p className="muted">{strings.settings.noNotices}</p>
        ) : (
          <>
            <h3 id="notices-title">{strings.settings.noticesTitle}</h3>
            <pre className="report" aria-labelledby="notices-title" tabIndex={0}>
              {notices}
            </pre>
          </>
        )}
      </section>

      <section aria-labelledby="privacy-title">
        <h2 id="privacy-title">{strings.settings.privacyTitle}</h2>
        <p>{strings.settings.privacy}</p>
      </section>
      <p className="muted">{strings.settings.moreLater}</p>
    </div>
  );
}
