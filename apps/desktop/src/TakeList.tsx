import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, describeError, IMPORT_EXTENSIONS, type TakeView } from "./api";
import { ConfirmDialog } from "./ConfirmDialog";
import { formatTime, takeName } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

interface Props {
  onSelect: (id: string) => void;
}

export function TakeList({ onSelect }: Props) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const project = useStore((s) => s.project)!;
  const loaded = useStore((s) => s.loaded);
  const mode = useStore((s) => s.mode);
  const setProject = useStore((s) => s.setProject);
  const setLoaded = useStore((s) => s.setLoaded);
  const setError = useStore((s) => s.setError);
  const pending = useStore((s) => s.pending);
  const setPending = useStore((s) => s.setPending);
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [toDelete, setToDelete] = useState<TakeView | null>(null);
  const busy = mode !== "idle" || pending !== null;

  const run = async (f: () => Promise<void>, label?: string) => {
    try {
      setError(null);
      if (label) setPending(label);
      await f();
    } catch (e) {
      setError(describeError(e));
    } finally {
      if (label) setPending(null);
    }
  };

  const parentName = (id: string) => {
    const p = project.takes.find((x) => x.id === id);
    return p ? takeName(p, t.take) : id;
  };

  const saveName = (id: string) =>
    run(async () => {
      setProject(await api.renameTake(id, draft.trim() || null));
      setEditing(null);
    });

  const relink = (id: string) =>
    run(async () => {
      const path = await open({
        title: t.relink,
        multiple: false,
        filters: [{ name: t.audioFiles, extensions: IMPORT_EXTENSIONS }],
      });
      if (path && !Array.isArray(path)) setProject(await api.relinkTake(id, path));
    });

  const confirmDelete = () =>
    run(async () => {
      const id = toDelete!.id;
      setToDelete(null);
      setProject(await api.deleteTake(id));
      if (loaded?.id === id) setLoaded(null, null);
    }, t.saving);

  return (
    <aside className="takes" aria-label={t.takes}>
      <div className="takes-head">
        <h2>{t.takes}</h2>
        <button className="ghost small" onClick={() => run(async () => setProject(await api.verifyTakes()), t.verifying)} disabled={busy}>
          {t.verify}
        </button>
      </div>
      {project.takes.length === 0 && <div className="muted small">{t.noTakes}</div>}
      <ul>
        {project.takes
          .slice()
          .reverse()
          .map((tk) => {
            const name = takeName(tk, t.take);
            const selected = loaded?.id === tk.id;
            const missing = tk.health === "missing";
            const changed = tk.health === "checksum_mismatch";
            return (
              <li key={tk.id} className={selected ? "selected" : ""}>
                {editing === tk.id ? (
                  <form
                    className="rename"
                    onSubmit={(e) => {
                      e.preventDefault();
                      void saveName(tk.id);
                    }}
                  >
                    <input
                      value={draft}
                      onChange={(e) => setDraft(e.target.value)}
                      aria-label={t.rename}
                      maxLength={120}
                      autoFocus
                      onKeyDown={(e) => e.key === "Escape" && setEditing(null)}
                    />
                    <button type="submit">{t.save}</button>
                  </form>
                ) : (
                  <button
                    className="take-main ghost"
                    disabled={busy || missing}
                    onClick={() => onSelect(tk.id)}
                    aria-pressed={selected}
                  >
                    <span className="take-name">{name}</span>
                    <span className="muted small mono-num">
                      {tk.duration_seconds != null ? formatTime(tk.duration_seconds) : "—"} ·{" "}
                      {tk.source === "imported_file"
                        ? `${t.imported}${tk.original_name ? `: ${tk.original_name}` : ""}`
                        : tk.source === "derived"
                          ? `${t.derivedFrom} ${tk.derived_from ? parentName(tk.derived_from) : "?"}`
                          : tk.source === "midi_file"
                            ? `${t.midiFile}${tk.original_name ? `: ${tk.original_name}` : ""}`
                            : t.microphone}
                    </span>
                  </button>
                )}
                <div className="take-badges">
                  {tk.state === "recovered" && <span className="badge warn">{t.recovered}</span>}
                  {missing && <span className="badge error">{t.missing}</span>}
                  {changed && <span className="badge error">{t.changed}</span>}
                  {(tk.trim_in_seconds > 0 || tk.trim_out_seconds != null) && <span className="badge">{t.trimmed}</span>}
                </div>
                <div className="take-actions">
                  {missing && (
                    <button className="ghost small" onClick={() => relink(tk.id)} disabled={busy}>
                      {t.relink}
                    </button>
                  )}
                  <button
                    className="ghost small"
                    disabled={busy}
                    onClick={() => {
                      setEditing(tk.id);
                      setDraft(tk.label ?? "");
                    }}
                    aria-label={`${t.rename}: ${name}`}
                  >
                    {t.rename}
                  </button>
                  <button
                    className="ghost small"
                    disabled={busy}
                    onClick={() => setToDelete(tk)}
                    aria-label={`${t.delete}: ${name}`}
                  >
                    {t.delete}
                  </button>
                </div>
              </li>
            );
          })}
      </ul>
      {toDelete && (
        <ConfirmDialog
          title={`${t.deleteTakeQ} — ${takeName(toDelete, t.take)}`}
          body={t.deleteTakeExplain}
          confirmLabel={t.delete}
          cancelLabel={t.cancel}
          onConfirm={() => void confirmDelete()}
          onCancel={() => setToDelete(null)}
        />
      )}
    </aside>
  );
}
