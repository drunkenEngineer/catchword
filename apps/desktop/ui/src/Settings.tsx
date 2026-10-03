// The Settings screen (UI-4, APP-3): what to leave out, where the data is
// and how to delete it, and the privacy statement. Every change is saved at
// once and takes effect without a restart.
import { useCallback, useEffect, useState } from "react";
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

      <section aria-labelledby="data-title">
        <h2 id="data-title">{strings.settings.dataTitle}</h2>
        <p dir="auto">{strings.settings.dataPlace(view.dataFolder, strings.bytes(view.indexBytes))}</p>
        {confirmingDelete ? (
          <div className="confirm">
            <span>{strings.settings.confirmDeleteAll}</span>
            <button
              type="button"
              className="danger"
              onClick={() => {
                setConfirmingDelete(false);
                engine.deleteAllData().catch(fail);
              }}
            >
              {strings.settings.deleteAll}
            </button>
            <button type="button" onClick={() => setConfirmingDelete(false)}>
              {strings.settings.keep}
            </button>
          </div>
        ) : (
          <button type="button" className="danger" onClick={() => setConfirmingDelete(true)}>
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

      <section aria-labelledby="privacy-title">
        <h2 id="privacy-title">{strings.settings.privacyTitle}</h2>
        <p>{strings.settings.privacy}</p>
      </section>
      <p className="muted">{strings.settings.moreLater}</p>
    </div>
  );
}
