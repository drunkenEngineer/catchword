// The Search screen (UI-2): the search box, results grouped by file with
// their passages beneath, a preview, and a status line.
//
// Document text, file names included, is untrusted: it is only ever placed
// as text, never as HTML, so markup in a document stays inert (SEC-1).
import { useEffect, useMemo, useRef, useState } from "react";
import type { FileKind } from "./contract/FileKind";
import type { SearchFilter } from "./contract/SearchFilter";
import type { FileHit } from "./contract/FileHit";
import type { PassageHit } from "./contract/PassageHit";
import type { SearchResponse } from "./contract/SearchResponse";
import type { Span } from "./contract/Span";
import type { Status } from "./contract/Status";
import { useEngine } from "./engine";
import { strings } from "./strings";

/** How long to wait after the last key before searching. */
const PAUSE_MS = 200;

interface Choice {
  file: FileHit;
  passage: PassageHit;
}

export function Search({ status, openLibrary }: { status: Status | null; openLibrary: () => void }) {
  const engine = useEngine();
  const [query, setQuery] = useState("");
  const [answer, setAnswer] = useState<SearchResponse | null>(null);
  const [filter, setFilter] = useState<SearchFilter>({ folder: null, kind: null });
  const filtered = filter.folder !== null || filter.kind !== null;
  const [selected, setSelected] = useState(0);
  const [preview, setPreview] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const box = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);

  // Search a moment after typing stops; a newer query replaces an older one.
  useEffect(() => {
    const text = query.trim();
    if (text === "") {
      setAnswer(null);
      return;
    }
    let current = true;
    const timer = setTimeout(() => {
      engine.search(text, filter).then(
        (found) => {
          if (current) {
            setAnswer(found);
            setSelected(0);
          }
        },
        (error) => current && setMessage(String(error)),
      );
    }, PAUSE_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [query, engine, filter]);

  const choices: Choice[] = useMemo(
    () => answer?.files.flatMap((file) => file.passages.map((passage) => ({ file, passage }))) ?? [],
    [answer],
  );
  const chosen = choices[selected];
  const chosenId = chosen?.passage.id;

  useEffect(() => {
    if (chosenId === undefined) {
      setPreview(null);
      return;
    }
    let current = true;
    engine.preview(chosenId).then((text) => current && setPreview(text), () => undefined);
    return () => {
      current = false;
    };
  }, [chosenId, engine]);

  // Ctrl+K or Ctrl+L puts the cursor in the search box from anywhere.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey && (event.key === "k" || event.key === "l")) {
        event.preventDefault();
        box.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    box.current?.focus();
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const say = (text: string) => {
    setMessage(text);
    setTimeout(() => setMessage(null), 2000);
  };
  const fail = (error: unknown) => say(String(error));

  const open = (choice: Choice | undefined) => choice && engine.openFile(choice.passage.id).catch(fail);
  const reveal = (choice: Choice | undefined) =>
    choice && engine.revealFile(choice.passage.id).catch(fail);
  const copy = (choice: Choice | undefined) => {
    if (!choice) return;
    const text = preview ?? choice.passage.snippet.map((span) => span.text).join("");
    const source = `${choice.file.name}, ${choice.passage.location}`;
    navigator.clipboard.writeText(`${text}\n— ${source}`).then(() => say(strings.search.copied), fail);
  };

  const onBoxKey = (event: React.KeyboardEvent) => {
    if (event.key === "Escape") {
      setQuery("");
    } else if (event.key === "ArrowDown" && choices.length > 0) {
      event.preventDefault();
      list.current?.focus();
    }
  };

  const onListKey = (event: React.KeyboardEvent) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((at) => Math.min(at + 1, choices.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      if (selected === 0) box.current?.focus();
      setSelected((at) => Math.max(at - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (event.ctrlKey) void reveal(chosen);
      else void open(chosen);
    } else if (event.ctrlKey && event.key === "c") {
      event.preventDefault();
      copy(chosen);
    } else if (event.key === "Escape") {
      setQuery("");
      box.current?.focus();
    }
  };

  if (status && status.folders.length === 0) {
    return (
      <section className="empty">
        <h2>{strings.search.noFoldersTitle}</h2>
        <p>{strings.search.noFoldersText}</p>
        <button type="button" className="primary" onClick={() => void engine.addFolder()}>
          {strings.search.addFolder}
        </button>
      </section>
    );
  }

  return (
    <div className="search">
      <input
        ref={box}
        className="search-box"
        type="search"
        aria-label={strings.search.box}
        placeholder={strings.search.placeholder}
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={onBoxKey}
        aria-keyshortcuts="Control+K"
        spellCheck={false}
      />
      {status && status.folders.length > 0 && (
        <div className="filters">
          <label>
            {strings.search.filterFolder}
            <select
              value={filter.folder ?? ""}
              onChange={(event) =>
                setFilter({ ...filter, folder: event.target.value === "" ? null : Number(event.target.value) })
              }
            >
              <option value="">{strings.search.allFolders}</option>
              {status.folders.map((folder) => (
                <option key={folder.id} value={folder.id}>
                  {folder.path}
                </option>
              ))}
            </select>
          </label>
          <label>
            {strings.search.filterKind}
            <select
              value={filter.kind ?? ""}
              onChange={(event) =>
                setFilter({ ...filter, kind: event.target.value === "" ? null : (event.target.value as FileKind) })
              }
            >
              <option value="">{strings.search.allKinds}</option>
              <option value="pdf">{strings.search.kinds.pdf}</option>
              <option value="text">{strings.search.kinds.text}</option>
            </select>
          </label>
        </div>
      )}
      {status?.work && <p className="notice">{strings.search.partial}</p>}

      {query.trim() === "" && (
        <section className="ready">
          {status && <p>{strings.search.summary(status.files)}</p>}
          <p>{strings.search.examplesTitle}</p>
          <ul className="examples">
            {strings.search.examples.map((example) => (
              <li key={example}>
                <button type="button" className="link" onClick={() => setQuery(example)}>
                  {example}
                </button>
              </li>
            ))}
          </ul>
          <p className="muted">{strings.search.tip}</p>
        </section>
      )}

      {answer && answer.files.length === 0 && query.trim() !== "" && (
        <section className="nothing">
          <h2>{strings.search.nothing(query.trim())}</h2>
          <p>{strings.search.whyTitle}</p>
          <ul>
            {filtered && <li>{strings.search.filtered}</li>}
            {causes(status).map(([cause, count]) => (
              <li key={cause}>{strings.search.causes[cause](count)}</li>
            ))}
            {status?.work && <li>{strings.search.stillIndexing}</li>}
            {answer.notes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
          <div className="actions">
            {filtered && (
              <button type="button" onClick={() => setFilter({ folder: null, kind: null })}>
                {strings.search.clearFilters}
              </button>
            )}
            <button type="button" className="link" onClick={openLibrary}>
              {strings.search.openLibrary}
            </button>
          </div>
        </section>
      )}

      {answer && answer.files.length > 0 && (
        <div className="results-and-preview">
          <div
            ref={list}
            className="results"
            role="listbox"
            tabIndex={0}
            aria-label={strings.search.results}
            aria-activedescendant={chosen ? `passage-${chosen.passage.id}` : undefined}
            onKeyDown={onListKey}
          >
            {answer.files.map((file, fileIndex) => (
              <div key={`${file.folder}/${file.name}`} role="group" aria-labelledby={`file-${fileIndex}`}>
                <div className="file" id={`file-${fileIndex}`}>
                  <span className="file-name" dir="auto">
                    {file.name}
                  </span>
                  <span className="file-folder" dir="auto">
                    {file.folder}
                  </span>
                  {file.modifiedSecs > 0 && <span className="file-date">{strings.search.modified(file.modifiedSecs)}</span>}
                  {file.copies > 1 && <span className="file-copies">{strings.search.copies(file.copies - 1)}</span>}
                </div>
                {file.passages.map((passage) => {
                  const index = choices.findIndex((choice) => choice.passage.id === passage.id);
                  return (
                    <div
                      key={passage.id}
                      id={`passage-${passage.id}`}
                      role="option"
                      aria-selected={index === selected}
                      className="passage"
                      onClick={() => setSelected(index)}
                      onDoubleClick={() => void open(choices[index])}
                    >
                      <span className="location">{passage.location}</span>
                      <Snippet spans={passage.snippet} />
                      <span className="found">{strings.search.found[passage.found]}</span>
                    </div>
                  );
                })}
              </div>
            ))}
          </div>

          <section className="preview" aria-label={strings.search.preview}>
            {chosen && (
              <>
                <h2 dir="auto">{chosen.file.name}</h2>
                <p className="preview-place">
                  <span dir="auto">{chosen.file.folder}</span> · {chosen.passage.location}
                </p>
                <p className="preview-text" dir="auto">
                  {preview}
                </p>
                <div className="actions">
                  <button type="button" onClick={() => void open(chosen)} aria-keyshortcuts="Enter">
                    {strings.search.open}
                  </button>
                  <button type="button" onClick={() => void reveal(chosen)} aria-keyshortcuts="Control+Enter">
                    {strings.search.reveal}
                  </button>
                  <button type="button" onClick={() => copy(chosen)} aria-keyshortcuts="Control+C">
                    {strings.search.copy}
                  </button>
                </div>
              </>
            )}
          </section>
        </div>
      )}

      <p className="status-line" role="status">
        {answer && query.trim() !== "" && strings.search.count(answer.files.length, answer.elapsedMs)}
        {answer && answer.files.length > 0 && answer.notes.map((note) => ` · ${note}`).join("")}
        {message && ` · ${message}`}
      </p>
    </div>
  );
}

/** Matched words are marked; everything is plain text, in its own direction. */
function Snippet({ spans }: { spans: Span[] }) {
  return (
    <span className="snippet" dir="auto">
      {spans.map((span, index) => (span.marked ? <mark key={index}>{span.text}</mark> : <span key={index}>{span.text}</span>))}
    </span>
  );
}

type Cause = keyof typeof strings.search.causes;

/** Files not indexed, counted by cause, the largest first; read failures together. */
export function causes(status: Status | null): [Cause, number][] {
  const counts = new Map<Cause, number>();
  for (const file of status?.notIndexed ?? []) {
    const cause: Cause = file.failed ? "failed" : ((file.code in strings.search.causes ? file.code : "failed") as Cause);
    counts.set(cause, (counts.get(cause) ?? 0) + 1);
  }
  return [...counts.entries()].sort((a, b) => b[1] - a[1]);
}
