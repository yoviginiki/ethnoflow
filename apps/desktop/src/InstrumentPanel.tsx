import { useEffect, useState } from "react";
import { api, describeError, INSTRUMENT_TYPES, type InstrumentType, type ProfileSummary } from "./api";
import { Calibrate } from "./Calibrate";
import { ConfirmDialog } from "./ConfirmDialog";
import { messages } from "./i18n";
import { useStore } from "./store";

/** Which instrument this project is played on: pick, create, calibrate, detach. */
export function InstrumentPanel() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const busy = s.mode !== "idle" || s.pending !== null;
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [creating, setCreating] = useState(false);
  const [calibrating, setCalibrating] = useState<string | null>(null);
  const [toDelete, setToDelete] = useState<ProfileSummary | null>(null);
  const [draft, setDraft] = useState<{ instrument: InstrumentType; name: string; tuning: string; transpose: string }>({
    instrument: "kaval",
    name: "",
    tuning: "",
    transpose: "0",
  });

  const refresh = () => api.listProfiles().then(setProfiles).catch((e) => s.setError(describeError(e)));
  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const attach = async (id: string | null) => {
    try {
      s.setError(null);
      s.setProject(await api.setProjectInstrument(id));
    } catch (e) {
      s.setError(describeError(e));
    }
  };

  const create = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      s.setError(null);
      const v = await api.createProfile({
        instrument: draft.instrument,
        name: draft.name,
        tuning: draft.tuning,
        transpose_semitones: Number(draft.transpose) || 0,
        a4_hz: null,
      });
      setCreating(false);
      await refresh();
      await attach(v.id);
      setCalibrating(v.id);
    } catch (err) {
      s.setError(describeError(err));
    }
  };

  const confirmDelete = async () => {
    if (!toDelete) return;
    try {
      setProfiles(await api.deleteProfile(toDelete.id));
      s.setProject(await api.projectState());
    } catch (e) {
      s.setError(describeError(e));
    } finally {
      setToDelete(null);
    }
  };

  const cur = project.instrument;
  return (
    <details className="panel instrument">
      <summary>
        {t.instrument}:{" "}
        <b>
          {cur
            ? `${cur.name} (${t.instrumentTypes[cur.instrument]}${cur.tuning ? `, ${cur.tuning}` : ""}) · ${cur.notes.length} ${t.notes}`
            : t.instrumentNone}
        </b>
      </summary>
      <div className="analysis-row">
        <label className="inline">
          {t.chooseInstrument}
          <select value={cur?.id ?? ""} disabled={busy} onChange={(e) => void attach(e.target.value || null)}>
            <option value="">—</option>
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} ({t.instrumentTypes[p.instrument]}{p.tuning ? `, ${p.tuning}` : ""}, {p.notes} {t.notes})
              </option>
            ))}
          </select>
        </label>
        <button className="ghost small" onClick={() => setCreating((c) => !c)} disabled={busy}>
          + {t.newInstrument}
        </button>
        {cur && (
          <>
            <button onClick={() => setCalibrating(cur.id)} disabled={busy}>
              {t.meetInstrument}
            </button>
            <button className="ghost small" onClick={() => void attach(null)} disabled={busy}>
              {t.detach}
            </button>
            <button
              className="ghost small"
              onClick={() => setToDelete(profiles.find((p) => p.id === cur.id) ?? { id: cur.id, version: cur.version, instrument: cur.instrument, name: cur.name, tuning: cur.tuning, notes: cur.notes.length })}
              disabled={busy}
            >
              {t.delete}
            </button>
          </>
        )}
      </div>
      {creating && (
        <form className="timing-form" onSubmit={create}>
          <label>
            {t.instrument}
            <select value={draft.instrument} onChange={(e) => setDraft({ ...draft, instrument: e.target.value as InstrumentType })}>
              {INSTRUMENT_TYPES.map((k) => (
                <option key={k} value={k}>
                  {t.instrumentTypes[k]}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t.instrumentName}
            <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} maxLength={60} autoFocus />
          </label>
          <label>
            {t.instrumentTuning}
            <input value={draft.tuning} onChange={(e) => setDraft({ ...draft, tuning: e.target.value })} placeholder={t.instrumentTuningHint} size={8} />
          </label>
          <label title={t.transpositionHint}>
            {t.transposition}
            <input inputMode="numeric" value={draft.transpose} onChange={(e) => setDraft({ ...draft, transpose: e.target.value })} size={3} />
          </label>
          <button type="submit" disabled={busy || !draft.name.trim()}>
            {t.create}
          </button>
        </form>
      )}
      {calibrating && (
        <Calibrate
          profileId={calibrating}
          onClose={async (changed) => {
            setCalibrating(null);
            if (changed) {
              await refresh();
              s.setProject(await api.projectState());
            }
          }}
        />
      )}
      {toDelete && (
        <ConfirmDialog
          title={`${t.deleteProfileQ} — ${toDelete.name}`}
          body={t.deleteProfileExplain}
          confirmLabel={t.delete}
          cancelLabel={t.cancel}
          onConfirm={() => void confirmDelete()}
          onCancel={() => setToDelete(null)}
        />
      )}
    </details>
  );
}
