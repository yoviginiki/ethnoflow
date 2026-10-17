import { useEffect, useRef, useState } from "react";
import { api, describeError, type LibraryKind, type LibrarySortBy, type LibrarySummary } from "./api";
import { ConfirmDialog } from "./ConfirmDialog";
import { messages } from "./i18n";
import { useStore } from "./store";

interface Props {
  kind: LibraryKind;
  /** The settings to save, as the panel's own JSON. */
  getPayload: () => unknown;
  /** Apply a saved payload to the panel. */
  apply: (payload: unknown) => void;
  /** Smaller buttons for crowded headers. */
  compact?: boolean;
}

export function formatDate(ms: number, lang: string): string {
  try {
    return new Date(ms).toLocaleString(lang === "bg" ? "bg-BG" : "en-GB", { dateStyle: "short", timeStyle: "short" });
  } catch {
    return new Date(ms).toISOString();
  }
}

export function parseTags(text: string): string[] {
  const out: string[] = [];
  for (const raw of text.split(/[,;]/)) {
    const tag = raw.trim().toLowerCase();
    if (tag && !out.includes(tag)) out.push(tag);
  }
  return out;
}

/**
 * Save / load presets of one panel in the user's library (shell `library_cmds.rs`):
 * name, tags, notes (searchable), project of origin, date. Shared by every panel that
 * has sliders; the Library screen shows all kinds together.
 */
export function PresetLibrary({ kind, getPayload, apply, compact = false }: Props) {
  const lang = useStore((s) => s.prefs.lang);
  const projectName = useStore((s) => s.project?.name ?? "");
  const t = messages[lang];
  const [mode, setMode] = useState<"closed" | "save" | "load">("closed");
  const [name, setName] = useState("");
  const [tags, setTags] = useState("");
  const [notes, setNotes] = useState("");
  const [list, setList] = useState<LibrarySummary[]>([]);
  const [query, setQuery] = useState("");
  const [sortBy, setSortBy] = useState<LibrarySortBy>("date_desc");
  const [onlyProject, setOnlyProject] = useState(true);
  const [editing, setEditing] = useState<LibrarySummary | null>(null);
  const [toDelete, setToDelete] = useState<LibrarySummary | null>(null);
  const [busy, setBusy] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  const refresh = () =>
    api
      .libraryList(kind, onlyProject && projectName ? projectName : null, query.trim() || null, sortBy)
      .then(setList)
      .catch(fail);

  useEffect(() => {
    if (mode !== "load") return;
    const id = window.setTimeout(() => void refresh(), 150);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, query, sortBy, onlyProject, projectName]);

  useEffect(() => {
    if (mode === "closed") return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node) && !toDelete) setMode("closed");
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setMode("closed");
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [mode, toDelete]);

  const save = async () => {
    const n = name.trim();
    if (!n) return;
    setBusy(true);
    try {
      const e = await api.librarySave({ kind, name: n, tags: parseTags(tags), notes: notes.trim(), payload: getPayload() });
      useStore.getState().setMessage(`${t.presetSaved}: ${e.name}`);
      setName("");
      setTags("");
      setNotes("");
      setMode("closed");
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };
  const load = async (e: LibrarySummary) => {
    setBusy(true);
    try {
      const full = await api.libraryGet(e.id);
      apply(full.payload);
      useStore.getState().setMessage(`${t.presetLoaded}: ${e.name}`);
      setMode("closed");
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };
  const overwrite = async (e: LibrarySummary) => {
    setBusy(true);
    try {
      await api.libraryReplacePayload(e.id, getPayload());
      useStore.getState().setMessage(`${t.presetUpdated}: ${e.name}`);
      await refresh();
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };
  const saveEdit = async () => {
    if (!editing) return;
    setBusy(true);
    try {
      await api.libraryUpdate(editing.id, editing.name.trim() || t.untitled, editing.tags, editing.notes);
      setEditing(null);
      await refresh();
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };
  const confirmDelete = async () => {
    if (!toDelete) return;
    const id = toDelete.id;
    setToDelete(null);
    try {
      await api.libraryDelete(id);
      await refresh();
    } catch (err) {
      fail(err);
    }
  };

  const btn = compact ? "ghost small" : "ghost";
  return (
    <div className="preset-lib" ref={ref}>
      <button className={btn} onClick={() => setMode(mode === "save" ? "closed" : "save")} aria-expanded={mode === "save"} title={t.libSaveHint}>
        💾 {t.libSave}
      </button>
      <button className={btn} onClick={() => setMode(mode === "load" ? "closed" : "load")} aria-expanded={mode === "load"} title={t.libLoadHint}>
        📚 {t.libLoad}
      </button>
      {mode === "save" && (
        <form
          className="panel preset-lib-pop"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <label>
            {t.presetName}
            <input autoFocus value={name} onChange={(e) => setName(e.target.value)} maxLength={80} />
          </label>
          <label>
            {t.libTags}
            <input value={tags} onChange={(e) => setTags(e.target.value)} placeholder={t.libTagsHint} />
          </label>
          <label>
            {t.libNotes}
            <textarea value={notes} onChange={(e) => setNotes(e.target.value)} rows={3} placeholder={t.libNotesHint} />
          </label>
          <div className="muted small">
            {t.libProject}: {projectName || t.noProjectOpen}
          </div>
          <div className="style-row">
            <button type="submit" className="primary" disabled={busy || !name.trim()}>
              {t.save}
            </button>
            <button type="button" className="ghost" onClick={() => setMode("closed")}>
              {t.cancel}
            </button>
          </div>
        </form>
      )}
      {mode === "load" && (
        <div className="panel preset-lib-pop wide" role="dialog" aria-label={t.libLoad}>
          <div className="analysis-row">
            <input className="grow" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t.libSearchHint} autoFocus aria-label={t.search} />
            <select value={sortBy} onChange={(e) => setSortBy(e.target.value as LibrarySortBy)} aria-label={t.libSort}>
              <option value="date_desc">{t.libSortNewest}</option>
              <option value="date_asc">{t.libSortOldest}</option>
              <option value="name">{t.libSortName}</option>
            </select>
            <label className="inline small">
              <input type="checkbox" checked={onlyProject} onChange={(e) => setOnlyProject(e.target.checked)} disabled={!projectName} />
              {t.libOnlyProject}
            </label>
          </div>
          {list.length === 0 && <div className="muted small">{t.libEmpty}</div>}
          <ul className="preset-lib-list">
            {list.map((e) => (
              <li key={e.id}>
                {editing?.id === e.id ? (
                  <form
                    className="preset-lib-edit"
                    onSubmit={(ev) => {
                      ev.preventDefault();
                      void saveEdit();
                    }}
                  >
                    <input value={editing.name} onChange={(ev) => setEditing({ ...editing, name: ev.target.value })} maxLength={80} aria-label={t.presetName} />
                    <input value={editing.tags.join(", ")} onChange={(ev) => setEditing({ ...editing, tags: parseTags(ev.target.value) })} placeholder={t.libTagsHint} aria-label={t.libTags} />
                    <textarea value={editing.notes} onChange={(ev) => setEditing({ ...editing, notes: ev.target.value })} rows={2} aria-label={t.libNotes} />
                    <div className="style-row">
                      <button type="submit" className="small" disabled={busy}>
                        {t.save}
                      </button>
                      <button type="button" className="ghost small" onClick={() => setEditing(null)}>
                        {t.cancel}
                      </button>
                    </div>
                  </form>
                ) : (
                  <>
                    <div className="preset-lib-row">
                      <button className="ghost grow" onClick={() => void load(e)} disabled={busy} title={t.libLoadThis}>
                        <strong>{e.name}</strong>
                        {e.tags.length > 0 && <span className="muted small"> · {e.tags.join(", ")}</span>}
                      </button>
                      <button className="ghost small" onClick={() => void overwrite(e)} disabled={busy} title={t.libOverwriteHint}>
                        ⟳
                      </button>
                      <button className="ghost small" onClick={() => setEditing(e)} title={t.libEditHint}>
                        ✎
                      </button>
                      <button className="ghost small" onClick={() => setToDelete(e)} title={t.delete}>
                        🗑
                      </button>
                    </div>
                    <div className="muted small">
                      {formatDate(e.updated_ms, lang)}
                      {e.project && !onlyProject && ` · ${e.project}`}
                      {e.notes && ` · ${e.notes.length > 120 ? e.notes.slice(0, 120) + "…" : e.notes}`}
                    </div>
                  </>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      {toDelete && <ConfirmDialog title={t.delete} body={`${t.libDeleteExplain} ${toDelete.name}`} confirmLabel={t.delete} cancelLabel={t.cancel} onConfirm={() => void confirmDelete()} onCancel={() => setToDelete(null)} />}
    </div>
  );
}
