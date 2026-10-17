import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, describeError, type Scale } from "./api";
import { formatBytes, noteParts, takeName } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

interface Props {
  /** Run a slow command with a busy label (errors go to the banner). */
  withPending: <T>(label: string, fn: () => Promise<T>) => Promise<T | undefined>;
  /** Load a take into the studio (used after "to the player"). */
  selectTake: (id: string) => Promise<void>;
}

/** Analyse button, scale suggestions, chosen scale, tuning reference, MIDI export. */
export function AnalysisPanel({ withPending, selectTake }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const analysis = s.analysis;
  const loadedTake = project.takes.find((tk) => tk.id === s.loaded?.id) ?? null;
  const busy = s.mode !== "idle" || s.pending !== null;
  const [a4Draft, setA4Draft] = useState(String(project.a4_hz));
  useEffect(() => setA4Draft(String(project.a4_hz)), [project.a4_hz]);

  const scaleName = (sc: Scale) =>
    `${t.noteNames[sc.tonic_pc]} ${sc.kind ? t.scaleKinds[sc.kind] : t.customScale}`;

  const analyze = async () => {
    const a = await withPending(t.analyzing, () => api.analyzeTake());
    if (a) s.setAnalysis(a);
  };

  const chooseScale = async (sc: Scale | null) => {
    try {
      s.setError(null);
      s.setProject(await api.setScale(sc));
    } catch (e) {
      s.setError(describeError(e));
    }
  };

  const applyA4 = async (e: React.FormEvent) => {
    e.preventDefault();
    const v = a4Draft.trim() === "" ? null : Number(a4Draft.replace(",", "."));
    try {
      s.setError(null);
      s.setProject(await api.setReference(v));
      // The reference changes note names: re-read the analysis view.
      s.setAnalysis(await api.getAnalysis());
    } catch (err) {
      s.setError(describeError(err));
    }
  };

  const exportMidi = async () => {
    if (!loadedTake) return;
    const base = takeName(loadedTake, t.take).replace(/[<>:"/\\|?*]/g, "_");
    const path = await save({
      title: t.exportMidi,
      defaultPath: `${project.name} - ${base}.mid`,
      filters: [{ name: "MIDI", extensions: ["mid"] }],
    });
    if (!path) return;
    const r = await withPending(t.exporting, () => api.exportMidi(path));
    if (!r) return;
    s.setMessage(
      `${t.midiExported}: ${r.path} (${r.notes} ${t.notes}, ${formatBytes(r.bytes)}, ${r.tempo_bpm} BPM)` +
        (r.technical_tempo ? ` — ${t.technicalTempoNote}` : ""),
    );
  };

  const toPlayer = async () => {
    if (!loadedTake) return;
    const label = `${takeName(loadedTake, t.take)} → MIDI`;
    const r = await withPending(t.saving, () => api.sendNotesToPlayer(label));
    if (!r) return;
    s.setProject(r.project);
    s.setPref("notesView", "falling");
    await selectTake(r.take_id);
    s.setMessage(t.sentToPlayer);
  };

  const resetNotes = async () => {
    const a = await withPending(t.saving, () => api.resetNotes());
    if (a) s.setAnalysis(a);
  };

  return (
    <details className="panel analysis" open>
      <summary>
        {t.analysis}
        {analysis && (
          <span className="muted small">
            {" "}
            · {analysis.notes.length} {t.notes}
            {analysis.edited && <span className="badge warn"> {t.editedNotes}</span>}
          </span>
        )}
      </summary>
      <div className="analysis-row">
        <button onClick={analyze} disabled={busy || !s.loaded}>
          {analysis ? t.reanalyze : t.analyze}
        </button>
        {!s.midi && (
          <button onClick={toPlayer} disabled={busy || !analysis || analysis.notes.length === 0} title={t.toPlayerHint}>
            ⇢ {t.toPlayer}
          </button>
        )}
        <button onClick={exportMidi} disabled={busy || !analysis || analysis.notes.length === 0}>
          {t.exportMidi}
        </button>
        {analysis?.edited && (
          <button className="ghost small" onClick={resetNotes} disabled={busy}>
            {t.resetNotes}
          </button>
        )}
        <span className="spacer" />
        <form className="inline-form" onSubmit={applyA4}>
          <label className="inline">
            {t.reference}
            <input inputMode="decimal" size={5} value={a4Draft} onChange={(e) => setA4Draft(e.target.value)} placeholder={t.referenceHint} disabled={busy} />
          </label>
          <button type="submit" className="ghost small" disabled={busy}>
            {t.apply}
          </button>
        </form>
      </div>

      <div className="analysis-row">
        <span className="small muted">{t.chosenScale}:</span>
        <b>{project.scale ? scaleName(project.scale) : t.noScale}</b>
        {project.scale && (
          <button className="ghost small" onClick={() => chooseScale(null)} disabled={busy}>
            {t.noScale}
          </button>
        )}
      </div>

      {!analysis && s.loaded && <div className="muted small">{t.notAnalyzed}</div>}
      {analysis && analysis.hypotheses.length > 0 && (
        <div className="hypotheses">
          <div className="small muted">{t.scaleSuggestions}</div>
          <ul>
            {analysis.hypotheses.slice(0, 6).map((h) => {
              const sc: Scale = { tonic_pc: h.tonic_pc, degrees_cents: [], kind: h.kind };
              const chosen = project.scale?.tonic_pc === h.tonic_pc && project.scale?.kind === h.kind;
              return (
                <li key={`${h.tonic_pc}-${h.kind}`} className={chosen ? "chosen" : ""}>
                  <span className="hyp-bar" style={{ width: `${Math.round(h.probability * 100)}%` }} />
                  <span className="hyp-label">
                    {t.noteNames[h.tonic_pc]} {t.scaleKinds[h.kind]}
                  </span>
                  <span className="mono-num small">{Math.round(h.probability * 100)} %</span>
                  <button className="ghost small" onClick={() => chooseScale(scaleFromKind(sc))} disabled={busy || chosen}>
                    {t.choose}
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      {analysis && s.selectedNote !== null && analysis.notes[s.selectedNote] && (
        <div className="small muted">
          {t.noteSelected}: {(() => {
            const n = analysis.notes[s.selectedNote];
            const p = noteParts(n.midi);
            return `${t.noteNames[p.pc]}${p.octave} ${n.cents >= 0 ? "+" : ""}${n.cents.toFixed(0)} c, ${n.start.toFixed(2)}–${n.end.toFixed(2)} s`;
          })()}
        </div>
      )}
    </details>
  );
}

/** Fill in the degree pattern for a tempered kind (mirrors music-core's tables). */
const KIND_SEMITONES: Record<NonNullable<Scale["kind"]>, number[]> = {
  ionian: [0, 2, 4, 5, 7, 9, 11],
  dorian: [0, 2, 3, 5, 7, 9, 10],
  phrygian: [0, 1, 3, 5, 7, 8, 10],
  lydian: [0, 2, 4, 6, 7, 9, 11],
  mixolydian: [0, 2, 4, 5, 7, 9, 10],
  aeolian: [0, 2, 3, 5, 7, 8, 10],
  locrian: [0, 1, 3, 5, 6, 8, 10],
  harmonic_minor: [0, 2, 3, 5, 7, 8, 11],
  melodic_minor: [0, 2, 3, 5, 7, 9, 11],
  phrygian_dominant: [0, 1, 4, 5, 7, 8, 10],
  double_harmonic: [0, 1, 4, 5, 7, 8, 11],
  major_pentatonic: [0, 2, 4, 7, 9],
  minor_pentatonic: [0, 3, 5, 7, 10],
};

export function scaleFromKind(sc: Scale): Scale {
  if (!sc.kind) return sc;
  return { ...sc, degrees_cents: KIND_SEMITONES[sc.kind].map((s) => s * 100) };
}
