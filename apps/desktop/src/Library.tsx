import { useEffect, useState } from "react";
import { api, describeError, LIBRARY_KINDS, type LibraryFacets, type LibraryKind, type LibrarySortBy, type LibrarySummary } from "./api";
import { ConfirmDialog } from "./ConfirmDialog";
import { formatDate, parseTags } from "./PresetLibrary";
import { messages, type StringMessageKey } from "./i18n";
import { applyLibraryPayload } from "./session";
import { useStore } from "./store";

export const LIB_KIND_LABEL: Record<LibraryKind, StringMessageKey> = {
  drone: "drone",
  pad: "pad",
  rhythm: "rhythmTitle",
  harmony: "harmonyTitle",
  intent: "intentTitle",
  ambient: "areaAmbient",
  background: "libBackground",
  session: "libSession",
};

/**
 * The Library screen: every saved preset and session of every panel, across projects,
 * with search (name, tags, notes), filters by kind / project / tag, and sort.
 * Loading applies the entry and opens the screen it belongs to.
 */
export function Library() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [facets, setFacets] = useState<LibraryFacets | null>(null);
  const [kind, setKind] = useState<LibraryKind | null>(null);
  const [project, setProject] = useState<string>("");
  const [tag, setTag] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [sortBy, setSortBy] = useState<LibrarySortBy>("date_desc");
  const [list, setList] = useState<LibrarySummary[]>([]);
  const [editing, setEditing] = useState<LibrarySummary | null>(null);
  const [toDelete, setToDelete] = useState<LibrarySummary | null>(null);
  const [busy, setBusy] = useState(false);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  const refresh = () =>
    Promise.all([api.libraryFacets(), api.libraryList(kind, project || null, query.trim() || null, sortBy)])
      .then(([f, l]) => {
        setFacets(f);
        setList(l);
      })
      .catch(fail);
  useEffect(() => {
    const id = window.setTimeout(() => void refresh(), 150);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind, project, query, sortBy]);

  const shown = tag ? list.filter((e) => e.tags.includes(tag)) : list;
  const count = (k: LibraryKind) => facets?.counts.find(([kk]) => kk === k)?.[1] ?? 0;
  const total = facets?.counts.reduce((a, [, n]) => a + n, 0) ?? 0;

  const load = async (e: LibrarySummary) => {
    if (!s.project) {
      useStore.getState().setError(t.libNeedProject);
      return;
    }
    setBusy(true);
    try {
      const full = await api.libraryGet(e.id);
      const screen = await applyLibraryPayload(e.kind as LibraryKind, full.payload);
      useStore.getState().setMessage(`${t.presetLoaded}: ${e.name}`);
      s.setScreen(screen);
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

  return (
    <section className="library">
      <div className="muted small">{t.libraryHint}</div>
      <div className="style-row">
        <button className={`ghost ${kind === null ? "on" : ""}`} onClick={() => setKind(null)}>
          {t.libKindAll} ({total})
        </button>
        {LIBRARY_KINDS.map((k) => (
          <button key={k} className={`ghost ${kind === k ? "on" : ""}`} onClick={() => setKind(kind === k ? null : k)} disabled={count(k) === 0 && kind !== k}>
            {t[LIB_KIND_LABEL[k]]} ({count(k)})
          </button>
        ))}
      </div>
      <div className="analysis-row">
        <input className="grow" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t.libSearchHint} aria-label={t.search} />
        <select value={project} onChange={(e) => setProject(e.target.value)} aria-label={t.libProject}>
          <option value="">{t.libProjectAll}</option>
          {(facets?.projects ?? []).map((p) => (
            <option key={p} value={p}>
              {p}
            </option>
          ))}
        </select>
        <select value={sortBy} onChange={(e) => setSortBy(e.target.value as LibrarySortBy)} aria-label={t.libSort}>
          <option value="date_desc">{t.libSortNewest}</option>
          <option value="date_asc">{t.libSortOldest}</option>
          <option value="name">{t.libSortName}</option>
          <option value="kind">{t.libSortKind}</option>
        </select>
      </div>
      {(facets?.tags.length ?? 0) > 0 && (
        <div className="style-row">
          <span className="muted small">{t.libTags}:</span>
          {facets!.tags.map((tg) => (
            <button key={tg} className={`ghost small ${tag === tg ? "on" : ""}`} onClick={() => setTag(tag === tg ? null : tg)}>
              #{tg}
            </button>
          ))}
        </div>
      )}
      {shown.length === 0 && <div className="muted">{t.libEmpty}</div>}
      <ul className="library-list">
        {shown.map((e) => (
          <li key={e.id} className="panel">
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
                <textarea value={editing.notes} onChange={(ev) => setEditing({ ...editing, notes: ev.target.value })} rows={3} aria-label={t.libNotes} />
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
                  <span className="badge">{t[LIB_KIND_LABEL[e.kind as LibraryKind] ?? "libSession"]}</span>
                  <strong className="grow">{e.name}</strong>
                  <button className="primary small" onClick={() => void load(e)} disabled={busy}>
                    {t.libLoadThis}
                  </button>
                  <button className="ghost small" onClick={() => setEditing(e)} title={t.libEditHint}>
                    ✎
                  </button>
                  <button className="ghost small" onClick={() => setToDelete(e)} title={t.delete}>
                    🗑
                  </button>
                </div>
                <div className="muted small">
                  {formatDate(e.updated_ms, s.prefs.lang)}
                  {e.project && ` · ${t.libProject}: ${e.project}`}
                  {e.tags.length > 0 && ` · ${e.tags.map((x) => "#" + x).join(" ")}`}
                </div>
                {e.notes && <div className="small library-notes">{e.notes}</div>}
              </>
            )}
          </li>
        ))}
      </ul>
      {toDelete && <ConfirmDialog title={t.delete} body={`${t.libDeleteExplain} ${toDelete.name}`} confirmLabel={t.delete} cancelLabel={t.cancel} onConfirm={() => void confirmDelete()} onCancel={() => setToDelete(null)} />}
    </section>
  );
}
