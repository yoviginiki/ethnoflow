import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, describeError, type ChordEvent, type HarmonyProposal, type HarmonyReason, type HarmonyUiParams, type Strategy } from "./api";
import { messages, type Messages } from "./i18n";
import { DEFAULT_HARMONY, DEFAULT_INTENT, useStore, type HarmonyPrefs } from "./store";
import { noteParts } from "./format";

function uiParams(h: HarmonyPrefs, locks: HarmonyUiParams["locks"]): HarmonyUiParams {
  return { tension: h.tension, movement: h.movement, richness: h.richness, seed: h.seed, locks, register_low: h.registerLow, register_high: h.registerHigh };
}

export function strategyLabel(t: Messages, s: Strategy): string {
  return s === "drone_only" ? t.strategyDrone : s === "modal_pads" ? t.strategyModal : t.strategyLiquid;
}

/** Chord label such as "Dm7", "G (open 5th)", "Csus4", "E quartal". */
export function chordLabel(t: Messages, c: ChordEvent): string {
  const root = t.noteNames[c.root_pc];
  if (c.kind) return `${root}${t.chordSuffix[c.kind]}`;
  return `${root} ${t.chordShapes[c.shape]}`;
}

export function reasonText(t: Messages, r: HarmonyReason): string {
  switch (r.kind) {
    case "tonic":
      return t.hrTonic;
    case "covers_melody":
      return t.hrCovers(r.tones.map((m) => `${t.noteNames[noteParts(m).pc]}`).join(", "), Math.round(r.share * 100));
    case "smooth_voice_leading":
      return t.hrSmooth(r.semitones.toFixed(1));
    case "kept_low_confidence":
      return t.hrKeptLowConfidence(Math.round(r.confidence * 100));
    case "kept_chord_lock":
      return t.hrKeptLock;
    case "kept_hold":
      return t.hrKeptHold(r.phrases_left);
    case "changed_for_clash":
      return t.hrChangedForClash(Math.round(r.share * 100));
    case "tension_target":
      return t.hrTension(Math.round(r.tension * 100));
    case "avoids_clash":
      return t.hrAvoids(t.noteNames[r.pc]);
    case "first_chord":
      return t.hrFirst;
  }
}

/** Recompute the three proposals when the analysis or the sliders change. */
export function useHarmonyProposals() {
  const perf = useStore((s) => s.performance);
  const intent = useStore((s) => s.prefs.intent ?? DEFAULT_INTENT);
  const h = useStore((s) => s.prefs.harmony ?? DEFAULT_HARMONY);
  useEffect(() => {
    if (!perf || perf.phrases.length === 0) {
      useStore.getState().setHarmony(null);
      return;
    }
    let cancelled = false;
    api
      .harmonyProposals(intent, uiParams(h, intent.locks))
      .then((p) => !cancelled && useStore.getState().setHarmony(p))
      .catch((e) => useStore.getState().setError(describeError(e)));
    return () => {
      cancelled = true;
    };
  }, [perf, intent, h]);
}

/** Accompaniment scheduler (offline lite): while a proposal is armed and the take
 *  plays, the pad takes the chord of the current phrase and the drone the proposal's
 *  root; both change only at phrase boundaries (the engine crossfades). */
export function useHarmonyFollow(position: number | null) {
  const harmony = useStore((s) => s.harmony);
  const armed = useStore((s) => s.armed);
  const index = armed.side === "a" ? armed.a : armed.b;
  const proposal = index !== null && harmony ? harmony[index] : null;
  useEffect(() => {
    const st = useStore.getState();
    if (!proposal) {
      if (st.padChord || st.droneRootOverride !== null) st.setFollow(null, null);
      return;
    }
    const droneRoot = proposal.drone[0]?.midi ?? null;
    const pos = position ?? 0;
    const chord = proposal.chords.find((c) => pos >= c.start && pos < c.end) ?? proposal.chords.find((c) => c.start >= pos) ?? proposal.chords[proposal.chords.length - 1];
    const notes = chord ? chord.tones.map((t) => ({ midi: t.midi, cents: t.cents })) : null;
    const same =
      st.droneRootOverride === droneRoot &&
      ((st.padChord === null && notes === null) || (st.padChord !== null && notes !== null && st.padChord.length === notes.length && st.padChord.every((n, i) => n.midi === notes[i].midi && n.cents === notes[i].cents)));
    if (!same) st.setFollow(notes, droneRoot);
  }, [proposal, position]);
}

export function HarmonyPanel() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const h = s.prefs.harmony ?? DEFAULT_HARMONY;
  const set = (patch: Partial<HarmonyPrefs>) => s.setPref("harmony", { ...h, ...patch });
  const [open, setOpen] = useState<number | null>(null);
  const proposals = s.harmony;
  const busy = s.pending !== null;
  const fail = (e: unknown) => s.setError(describeError(e));

  const arm = (side: "a" | "b", i: number) => {
    const cur = s.armed;
    const same = cur[side] === i;
    s.setArmed({ ...cur, [side]: same ? null : i, side });
    if (!same && !s.layers.pad && proposals && proposals[i].chords.length > 0) s.setLayer("pad", true);
    if (!same && !s.layers.drone) s.setLayer("drone", true);
  };

  const exportMidi = async (p: HarmonyProposal) => {
    try {
      const path = await save({ title: t.exportHarmonyMidi, filters: [{ name: "MIDI", extensions: ["mid"] }] });
      if (!path) return;
      const r = await api.exportHarmonyMidi(p, path);
      s.setMessage(`${t.exported}: ${r.path} · ${r.chords} ${t.chordsWord} · ${r.bass_notes} ${t.bassWord}`);
    } catch (e) {
      fail(e);
    }
  };

  const bars: { key: keyof HarmonyProposal["score"]; label: string }[] = [
    { key: "melodic_compatibility", label: t.scMelodic },
    { key: "modal_fidelity", label: t.scModal },
    { key: "voice_leading", label: t.scVoiceLeading },
    { key: "tension_fit", label: t.scTension },
    { key: "register_separation", label: t.scRegister },
    { key: "variety", label: t.scVariety },
  ];

  return (
    <details className="panel harmony" open>
      <summary>
        {t.harmonyTitle}
        {s.armed.a !== null || s.armed.b !== null ? ` · ${t.playing}: ${s.armed.side.toUpperCase()}` : ""}
      </summary>
      <div className="muted small">{t.harmonyHint}</div>
      <div className="drone-sliders">
        <label className="drone-slider">
          <span>{t.hTension}</span>
          <input type="range" min={0} max={100} value={Math.round(h.tension * 100)} onChange={(e) => set({ tension: Number(e.target.value) / 100 })} />
          <span className="mono-num small">{Math.round(h.tension * 100)}</span>
        </label>
        <label className="drone-slider">
          <span>{t.hMovement}</span>
          <input type="range" min={0} max={100} value={Math.round(h.movement * 100)} onChange={(e) => set({ movement: Number(e.target.value) / 100 })} />
          <span className="mono-num small">{Math.round(h.movement * 100)}</span>
        </label>
        <label className="drone-slider">
          <span>{t.hRichness}</span>
          <input type="range" min={0} max={100} value={Math.round(h.richness * 100)} onChange={(e) => set({ richness: Number(e.target.value) / 100 })} />
          <span className="mono-num small">{Math.round(h.richness * 100)}</span>
        </label>
        <label className="drone-slider">
          <span>{t.hRegister}</span>
          <input type="range" min={36} max={72} value={h.registerLow} onChange={(e) => set({ registerLow: Number(e.target.value), registerHigh: Number(e.target.value) + 24 })} />
          <span className="mono-num small">
            {t.noteNames[noteParts(h.registerLow).pc]}
            {noteParts(h.registerLow).octave}
          </span>
        </label>
      </div>
      <div className="analysis-row small">
        <span className="muted">
          {t.padSeed}: <span className="mono-num">{h.seed}</span>
        </span>
        <button className="ghost small" onClick={() => set({ seed: Math.floor(Math.random() * 1_000_000) + 1 })}>
          {t.padNewSeed}
        </button>
        {(s.armed.a !== null || s.armed.b !== null) && (
          <>
            <span className="muted">{t.abCompare}:</span>
            <button className={`small ${s.armed.side === "a" ? "" : "ghost"}`} onClick={() => s.setArmed({ ...s.armed, side: "a" })} disabled={s.armed.a === null}>
              A
            </button>
            <button className={`small ${s.armed.side === "b" ? "" : "ghost"}`} onClick={() => s.setArmed({ ...s.armed, side: "b" })} disabled={s.armed.b === null}>
              B
            </button>
            <button className="ghost small" onClick={() => s.setArmed({ a: null, b: null, side: "a" })}>
              {t.disarm}
            </button>
          </>
        )}
      </div>
      {!proposals && <div className="muted small">{t.noProposals}</div>}
      {proposals && (
        <div className="proposal-row">
          {proposals.map((p, i) => (
            <div key={p.strategy} className={`proposal ${s.armed.a === i || s.armed.b === i ? "armed" : ""}`}>
              <div className="analysis-row">
                <strong>{strategyLabel(t, p.strategy)}</strong>
                <span className="mono-num small">{Math.round(p.score.total * 100)}</span>
                <span className="muted small">
                  {t.confidence}: {Math.round(p.confidence * 100)} %
                </span>
              </div>
              <div className="score-bars">
                {bars.map((b) => (
                  <div key={b.key} className="score-bar" title={b.label}>
                    <span className="small muted">{b.label}</span>
                    <div className="meter">
                      <div className="meter-fill" style={{ width: `${Math.round(Math.max(0, Math.min(1, p.score[b.key])) * 100)}%` }} />
                    </div>
                  </div>
                ))}
              </div>
              <div className="small">
                {t.droneRoot}: {p.drone.map((d) => `${t.noteNames[noteParts(d.midi).pc]}${noteParts(d.midi).octave}`).join(" + ")}
                {p.chords.length > 0 && ` · ${p.chords.length} ${t.chordsWord}`}
              </div>
              <div className="analysis-row">
                <button className={`small ${s.armed.a === i ? "" : "ghost"}`} onClick={() => arm("a", i)} disabled={busy}>
                  {s.armed.a === i ? t.armedA : t.armA}
                </button>
                <button className={`small ${s.armed.b === i ? "" : "ghost"}`} onClick={() => arm("b", i)} disabled={busy}>
                  {s.armed.b === i ? t.armedB : t.armB}
                </button>
                <button className="ghost small" onClick={() => setOpen(open === i ? null : i)}>
                  {open === i ? t.hideWhy : t.showWhy}
                </button>
                <button className="ghost small" onClick={() => void exportMidi(p)} disabled={busy}>
                  {t.exportHarmonyMidi}
                </button>
              </div>
              {open === i && (
                <ol className="phrase-list small">
                  {p.chords.length === 0 && <li>{t.droneOnlyWhy(t.noteNames[p.tonic_pc])}</li>}
                  {p.chords.map((c) => (
                    <li key={c.phrase}>
                      <span className="mono-num muted">
                        {c.start.toFixed(1)}–{c.end.toFixed(1)} s
                      </span>{" "}
                      <strong>{chordLabel(t, c)}</strong> — {c.why.map((r) => reasonText(t, r)).join("; ")}
                    </li>
                  ))}
                </ol>
              )}
            </div>
          ))}
        </div>
      )}
    </details>
  );
}
