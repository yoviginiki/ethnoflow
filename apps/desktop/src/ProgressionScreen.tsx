import { useState } from "react";
import { ConfirmDialog } from "./ConfirmDialog";
import { messages } from "./i18n";
import { chordNotes, DEFAULT_PROGRESSION, generateProgression, GENRE_SPECS, genreSpec, QUALITIES, stepAtBar, stepLabel, totalBars, type ChordQuality, type Genre, type ProgStep, type Progression } from "./progression";
import { DEFAULT_RHYTHM, useStore } from "./store";
import { noteParts } from "./format";

const BARS_CHOICES = [4, 8, 16, 32, 64];
const PER_CHOICES = [1, 2, 4, 8];

/**
 * Chord progressions by genre (owner 1.11.2026): a generator (genre + seed) that you can
 * edit by hand afterwards, and a follower that drives the pad, the drone root and the
 * rhythm section with the chord of the current bar while the song or the rhythm runs.
 */
export function ProgressionScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const scale = s.project?.scale ?? null;
  const prog: Progression = s.prefs.progression ?? DEFAULT_PROGRESSION;
  const [genre, setGenre] = useState<Genre>(prog.genre);
  const [bars, setBars] = useState(prog.bars || 8);
  const [per, setPer] = useState<number | null>(null);
  const [seed, setSeed] = useState(prog.seed || 1 + Math.floor(Math.random() * 1_000_000));
  const [drumsToo, setDrumsToo] = useState(true);
  const [confirm, setConfirm] = useState(false);
  const spec = genreSpec(genre) ?? GENRE_SPECS[0];

  const save = (next: Progression) => s.setPref("progression", next);
  const doGenerate = () => {
    const next = generateProgression(genre, seed, bars, per ?? undefined);
    save({ ...next, follow: prog.follow });
    if (drumsToo) {
      const rp = s.prefs.rhythm ?? DEFAULT_RHYTHM;
      s.setPref("rhythm", { ...rp, drums: { ...rp.drums, style: spec.drumStyle } });
    }
  };
  const generate = () => (prog.edited && prog.steps.length > 0 ? setConfirm(true) : doGenerate());
  const patchStep = (i: number, patch: Partial<ProgStep>) => save({ ...prog, edited: true, steps: prog.steps.map((st, k) => (k === i ? { ...st, ...patch } : st)) });
  const removeStep = (i: number) => save({ ...prog, edited: true, steps: prog.steps.filter((_, k) => k !== i) });
  const addStep = () => save({ ...prog, edited: true, steps: [...prog.steps, { degree: 1, quality: "triad", bars: spec.barsPerChord }] });

  const bar = (s.transport?.playing ? s.transport.bar : s.rhythmStatus?.bar) ?? null;
  const now = bar !== null ? stepAtBar(prog, Math.max(0, bar - 1)) : null;
  const total = totalBars(prog);
  const qualityName = (q: ChordQuality) => t.qualityNames[q];

  return (
    <main className="compose progression">
      <section className="panel">
        <h2>{t.progTitle}</h2>
        <div className="muted small">{t.progIntro}</div>
        {!scale && <div className="badge warn">{t.progNoScale}</div>}
      </section>

      <section className="panel">
        <h3>{t.progGenres}</h3>
        <div className="scapes-worlds" role="radiogroup" aria-label={t.progGenres}>
          {GENRE_SPECS.map((g) => (
            <button key={g.id} role="radio" aria-checked={g.id === genre} className={`world-card ${g.id === genre ? "on" : ""}`} onClick={() => setGenre(g.id)} data-genre={g.id}>
              <span className="world-title">{t[g.title]}</span>
              <span className="muted small">{t[g.desc]}</span>
              <span className="muted small">{t.progTempoHint(g.tempoHint[0], g.tempoHint[1])} · {t.progDrumStyle}: {g.drumStyle}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="panel scapes-transport" data-coach="progression-generate">
        <div className="analysis-row">
          <label className="inline">
            {t.progBars}
            <select value={bars} onChange={(e) => setBars(Number(e.target.value))}>
              {BARS_CHOICES.map((b) => (
                <option key={b} value={b}>
                  {b}
                </option>
              ))}
            </select>
          </label>
          <label className="inline">
            {t.progBarsPerChord}
            <select value={per ?? ""} onChange={(e) => setPer(e.target.value === "" ? null : Number(e.target.value))}>
              <option value="">{t.progAuto} ({spec.barsPerChord})</option>
              {PER_CHOICES.map((b) => (
                <option key={b} value={b}>
                  {b}
                </option>
              ))}
            </select>
          </label>
          <label className="inline">
            {t.scapesSeed}
            <span className="mono-num">{seed}</span>
            <button className="ghost small" onClick={() => setSeed(1 + Math.floor(Math.random() * 1_000_000))}>
              🎲 {t.scapesShuffle}
            </button>
          </label>
          <label className="inline">
            <input type="checkbox" checked={drumsToo} onChange={(e) => setDrumsToo(e.target.checked)} />
            {t.progDrumsToo}
          </label>
          <button className="primary" onClick={generate} disabled={!scale}>
            {t.progGenerate}
          </button>
        </div>
      </section>

      <section className="panel" data-coach="progression-steps">
        <div className="analysis-row">
          <h3>{t.progSteps}</h3>
          <span className="muted small">
            {total} {t.barsShort}
            {prog.edited && ` · ${t.progEdited}`}
          </span>
          <label className="inline">
            <input type="checkbox" checked={prog.follow} onChange={(e) => save({ ...prog, follow: e.target.checked })} disabled={prog.steps.length === 0 || !scale} />
            {t.progFollow}
          </label>
          {now && scale && (
            <span className="badge ok">
              {t.progNow} {stepLabel(scale, now.step)} · {chordNotes(scale, now.step).map((n) => `${t.noteNames[noteParts(n.midi).pc]}${noteParts(n.midi).octave}`).join(" ")}
            </span>
          )}
        </div>
        <div className="muted small">{t.progFollowHint}</div>
        {prog.steps.length === 0 ? (
          <div className="muted small">{t.progEmpty}</div>
        ) : (
          <div className="prog-grid">
            {prog.steps.map((st, i) => (
              <div key={i} className={`prog-step ${now?.index === i ? "on" : ""}`}>
                <div className="prog-label">{stepLabel(scale, st)}</div>
                <div className="muted small">{scale ? chordNotes(scale, st).map((n) => `${t.noteNames[noteParts(n.midi).pc]}${noteParts(n.midi).octave}`).join(" ") : "—"}</div>
                <label className="inline small">
                  {t.progDegree}
                  <select value={st.degree} onChange={(e) => patchStep(i, { degree: Number(e.target.value) })}>
                    {[1, 2, 3, 4, 5, 6, 7].map((d) => (
                      <option key={d} value={d}>
                        {d}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="inline small">
                  {t.progQuality}
                  <select value={st.quality} onChange={(e) => patchStep(i, { quality: e.target.value as ChordQuality })}>
                    {QUALITIES.map((q) => (
                      <option key={q} value={q}>
                        {qualityName(q)}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="inline small">
                  {t.progStepBars}
                  <input type="number" min={1} max={32} value={st.bars} onChange={(e) => patchStep(i, { bars: Math.max(1, Math.min(32, Number(e.target.value) || 1)) })} style={{ width: 52 }} />
                </label>
                <button className="ghost small" onClick={() => removeStep(i)} aria-label={t.progRemoveStep} title={t.progRemoveStep}>
                  ✕
                </button>
              </div>
            ))}
            <button className="ghost" onClick={addStep}>
              {t.progAddStep}
            </button>
          </div>
        )}
      </section>

      {confirm && <ConfirmDialog title={t.progGenerate} body={t.progGenerateConfirm} confirmLabel={t.progGenerate} cancelLabel={t.cancel} onConfirm={() => (setConfirm(false), doGenerate())} onCancel={() => setConfirm(false)} />}
    </main>
  );
}
