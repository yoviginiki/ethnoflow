import { useEffect, useRef, useState } from "react";
import { api, describeError, EMOTIONS, type Emotion, type InstrumentProfile, type PadNote, type PadSettings, type PadStatus, type Scale } from "./api";
import { formatDb, linearToDb, meterFraction, noteParts } from "./format";
import { messages } from "./i18n";
import { DEFAULT_PAD, useStore, VOICINGS, type PadPrefs, type Voicing } from "./store";

interface Props {
  scale: Scale | null;
  profile: InstrumentProfile | null;
}

const ROOT_RANGE = Array.from({ length: 49 }, (_, i) => 36 + i); // C2 … C6

/** Chord notes for a voicing: mode-neutral intervals, or degrees of the chosen scale. */
export function voicingNotes(voicing: Voicing, root: number, scale: Scale | null): PadNote[] | null {
  const plain = (semis: number[]) => semis.map((s) => ({ midi: root + s, cents: 0 }));
  switch (voicing) {
    case "fifth":
      return plain([0, 7, 12]);
    case "sus2":
      return plain([0, 2, 7]);
    case "sus4":
      return plain([0, 5, 7]);
    case "fifth9":
      return plain([0, 7, 14]);
    case "scale_triad":
    case "scale_7th": {
      if (!scale || scale.degrees_cents.length < 7) return null;
      const idx = voicing === "scale_triad" ? [0, 2, 4] : [0, 2, 4, 6];
      return idx.map((i) => {
        const c = scale.degrees_cents[i];
        const semis = Math.round(c / 100);
        return { midi: root + semis, cents: c - semis * 100 };
      });
    }
  }
}

function toSettings(p: PadPrefs, notes: PadNote[]): PadSettings {
  return { notes, emotion: p.emotion, x: p.x, y: p.y, level_db: p.levelDb, lfo_hz: p.lfoHz, seed: p.seed, intonation: p.intonation };
}

/** Liquid pad on the audio graph (F3.3): chord from a voicing, emotion preset, X/Y macro. */
export function PadPanel({ scale, profile }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const prefs = s.prefs.pad ?? DEFAULT_PAD;
  const [on, setOn] = useState(false);
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState<PadStatus | null>(null);
  const padRef = useRef<HTMLDivElement>(null);
  const suggested = s.prefs.drone?.rootMidi ?? (scale ? 48 + scale.tonic_pc : (profile?.notes[0]?.midi ?? null));
  const rootMidi = prefs.rootMidi ?? suggested;
  const notes = rootMidi === null ? null : voicingNotes(prefs.voicing, rootMidi, scale);
  const set = (patch: Partial<PadPrefs>) => s.setPref("pad", { ...prefs, ...patch });
  const fail = (e: unknown) => s.setError(describeError(e));

  useEffect(() => {
    if (!on || !notes) return;
    let cancelled = false;
    api
      .startPad(s.prefs.selectedOutput, toSettings(prefs, notes))
      .then((st) => {
        if (cancelled) return;
        setStatus(st);
        setRunning(true);
      })
      .catch((e) => {
        fail(e);
        setOn(false);
      });
    return () => {
      cancelled = true;
      setRunning(false);
      setStatus(null);
      void api.stopPad().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [on, s.prefs.selectedOutput]);

  useEffect(() => {
    if (!running || !notes) return;
    api.setPad(toSettings(prefs, notes)).then(setStatus).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running, prefs, rootMidi, scale, profile?.version, profile?.id]);

  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => {
      api
        .padStatus()
        .then((st) => {
          if (st) setStatus(st);
        })
        .catch(() => undefined);
    }, 200);
    return () => window.clearInterval(id);
  }, [running]);

  const xyFromEvent = (e: React.PointerEvent) => {
    const r = padRef.current?.getBoundingClientRect();
    if (!r) return;
    const x = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    const y = Math.max(0, Math.min(1, 1 - (e.clientY - r.top) / r.height));
    set({ x: Math.round(x * 100) / 100, y: Math.round(y * 100) / 100 });
  };

  const peakDb = status ? linearToDb(status.layer_peak) : -60;
  const noteLabel = (n: PadNote) => {
    const np = noteParts(n.midi);
    return `${t.noteNames[np.pc]}${np.octave}${n.cents ? ` ${n.cents > 0 ? "+" : ""}${Math.round(n.cents)}¢` : ""}`;
  };

  return (
    <div className="panel pad">
      <div className="analysis-row">
        <label className="inline">
          <input type="checkbox" checked={on} disabled={!notes} onChange={(e) => setOn(e.target.checked)} />
          <strong>{t.pad}</strong>
        </label>
        <label>
          {t.droneRoot}
          <select value={rootMidi ?? ""} onChange={(e) => set({ rootMidi: e.target.value ? Number(e.target.value) : null })}>
            {rootMidi === null && <option value="">{t.droneChooseRoot}</option>}
            {ROOT_RANGE.map((m) => {
              const np = noteParts(m);
              return (
                <option key={m} value={m}>
                  {t.noteNames[np.pc]}
                  {np.octave}
                </option>
              );
            })}
          </select>
        </label>
        <label>
          {t.padVoicing}
          <select value={prefs.voicing} onChange={(e) => set({ voicing: e.target.value as Voicing })}>
            {VOICINGS.map((v) => (
              <option key={v} value={v} disabled={v.startsWith("scale") && (!scale || scale.degrees_cents.length < 7)}>
                {t.voicings[v]}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t.padEmotion}
          <select value={prefs.emotion} onChange={(e) => set({ emotion: e.target.value as Emotion })}>
            {EMOTIONS.map((e) => (
              <option key={e} value={e}>
                {t.emotions[e]}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t.droneIntonation}
          <select value={prefs.intonation} onChange={(e) => set({ intonation: e.target.value === "profile" ? "profile" : "tempered" })}>
            <option value="profile" disabled={!profile}>
              {t.droneIntProfile}
            </option>
            <option value="tempered">{t.droneIntTempered}</option>
          </select>
        </label>
      </div>
      {rootMidi !== null && !notes && <div className="muted small">{t.padNeedsScale}</div>}
      <div className="pad-body">
        <div
          ref={padRef}
          className="xy-pad"
          role="slider"
          aria-label={`${t.padX} / ${t.padY}`}
          aria-valuenow={Math.round(prefs.x * 100)}
          onPointerDown={(e) => {
            e.currentTarget.setPointerCapture(e.pointerId);
            xyFromEvent(e);
          }}
          onPointerMove={(e) => {
            if (e.buttons & 1) xyFromEvent(e);
          }}
        >
          <span className="xy-dot" style={{ left: `${prefs.x * 100}%`, top: `${(1 - prefs.y) * 100}%` }} />
          <span className="xy-label x small muted">{t.padX}</span>
          <span className="xy-label y small muted">{t.padY}</span>
        </div>
        <div className="drone-sliders pad-sliders">
          <label className="drone-slider">
            <span>{t.droneLevel}</span>
            <input type="range" min={-40} max={0} step={1} value={prefs.levelDb} onChange={(e) => set({ levelDb: Number(e.target.value) })} />
            <span className="mono-num small">{formatDb(prefs.levelDb)}</span>
          </label>
          <label className="drone-slider">
            <span>{t.padLfo}</span>
            <input type="range" min={0.02} max={1} step={0.01} value={prefs.lfoHz} onChange={(e) => set({ lfoHz: Number(e.target.value) })} />
            <span className="mono-num small">{prefs.lfoHz.toFixed(2)}</span>
          </label>
          <div className="analysis-row small">
            <span className="muted">
              {t.padSeed}: <span className="mono-num">{prefs.seed}</span>
            </span>
            <button className="ghost small" onClick={() => set({ seed: Math.floor(Math.random() * 1_000_000) + 1 })}>
              {t.padNewSeed}
            </button>
          </div>
          {notes && (
            <div className="small muted">
              {t.padNotes}: {notes.map(noteLabel).join(" · ")}
              {status && ` · ${status.intonation_used === "profile" ? t.droneIntProfile : t.droneIntTempered}`}
            </div>
          )}
          {status && (
            <div className="analysis-row small">
              <div className="meter" role="meter" aria-valuemin={-60} aria-valuemax={0} aria-valuenow={Math.round(peakDb)} aria-label={t.droneLevel}>
                <div className={`meter-fill ${peakDb > -3 ? "hot" : peakDb > -12 ? "warm" : ""}`} style={{ width: `${meterFraction(peakDb) * 100}%` }} />
              </div>
              <span className="mono-num">{formatDb(peakDb)}</span>
              {status.stream_errors > 0 && (
                <span className="clip">
                  {status.stream_errors} {t.droneStreamErrors}
                </span>
              )}
            </div>
          )}
        </div>
      </div>
      <div className="muted small">{t.padHint}</div>
    </div>
  );
}
