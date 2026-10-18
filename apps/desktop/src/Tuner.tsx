import { useEffect, useRef, useState } from "react";
import type { InstrumentProfile, Scale } from "./api";
import { centsFromScale, hzToMidi, noteParts } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";
import { FingeringDiagram } from "./FingeringDiagram";
import { chartFor } from "./fingerings";

interface Props {
  hz: number | null;
  confidence: number;
  a4: number;
  scale: Scale | null;
  /** Null = tuner off (shows a hint instead of a reading). */
  active: boolean;
  /** The project's instrument profile: adds the profile mark and the written name. */
  profile: InstrumentProfile | null;
}

const IN_SCALE_TOLERANCE_CENTS = 25;
const PRACTICE_TOLERANCE_CENTS = 15;
const PRACTICE_HOLD_MS = 1000;
const HOLD_WINDOW_S = 10;
const HOLD_TOLERANCE_CENTS = 15;

interface HoldSample {
  t: number;
  cents: number | null;
}

/** Drift strip for "Hold the tone": cents over the last seconds, tolerance band shaded. */
function HoldStrip({ samples, now }: { samples: HoldSample[]; now: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    const w = c.clientWidth;
    const h = c.clientHeight;
    if (c.width !== Math.floor(w * dpr) || c.height !== Math.floor(h * dpr)) {
      c.width = Math.floor(w * dpr);
      c.height = Math.floor(h * dpr);
    }
    const ctx = c.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const style = getComputedStyle(c);
    const col = (v: string, d: string) => style.getPropertyValue(v).trim() || d;
    const yOf = (cents: number) => h / 2 - (Math.max(-50, Math.min(50, cents)) / 50) * (h / 2 - 2);
    ctx.fillStyle = col("--selection", "rgba(255,180,84,0.22)");
    ctx.fillRect(0, yOf(HOLD_TOLERANCE_CENTS), w, yOf(-HOLD_TOLERANCE_CENTS) - yOf(HOLD_TOLERANCE_CENTS));
    ctx.strokeStyle = col("--axis", "#333");
    ctx.beginPath();
    ctx.moveTo(0, h / 2 + 0.5);
    ctx.lineTo(w, h / 2 + 0.5);
    ctx.stroke();
    ctx.strokeStyle = col("--wave", "#7fd6c2");
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    let pen = false;
    for (const s of samples) {
      const x = w - ((now - s.t) / HOLD_WINDOW_S) * w;
      if (s.cents === null || x < 0) {
        pen = false;
        continue;
      }
      const y = yOf(s.cents);
      if (pen) ctx.lineTo(x, y);
      else ctx.moveTo(x, y);
      pen = true;
    }
    ctx.stroke();
  }, [samples, now]);
  return <canvas ref={ref} className="hold-strip" aria-hidden="true" />;
}

/**
 * Live tuner: note name, cents needle, Hz, confidence, in-scale indicator when the
 * project has a chosen scale. Includes the first practice: hold a target note.
 */
export function Tuner({ hz, confidence, a4, scale, active, profile }: Props) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const [targetMidi, setTargetMidi] = useState(69);
  const [hit, setHit] = useState(false);
  const heldSince = useRef<number | null>(null);

  // "Hold the tone": the first stable note latches as the reference; samples of the
  // drift are kept for a strip, and seconds within tolerance add up.
  const [holdOn, setHoldOn] = useState(false);
  const [holdRef, setHoldRef] = useState<number | null>(null);
  const [holdSamples, setHoldSamples] = useState<HoldSample[]>([]);
  const [holdSeconds, setHoldSeconds] = useState(0);
  const [holdNow, setHoldNow] = useState(0);
  const lastHoldT = useRef<number | null>(null);

  const midiFrac = hz !== null && confidence >= 0.5 ? hzToMidi(hz, a4) : null;

  useEffect(() => {
    if (!holdOn) return;
    const now = performance.now() / 1000;
    setHoldNow(now);
    let ref = holdRef;
    if (ref === null && midiFrac !== null) {
      ref = Math.round(midiFrac);
      setHoldRef(ref);
    }
    const cents = midiFrac !== null && ref !== null ? (midiFrac - ref) * 100 : null;
    setHoldSamples((prev) => [...prev.filter((s) => now - s.t <= HOLD_WINDOW_S), { t: now, cents }]);
    if (lastHoldT.current !== null && cents !== null && Math.abs(cents) <= HOLD_TOLERANCE_CENTS) {
      setHoldSeconds((s) => s + Math.min(0.2, now - lastHoldT.current!));
    }
    lastHoldT.current = now;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [midiFrac, holdOn]);

  const resetHold = () => {
    setHoldRef(null);
    setHoldSamples([]);
    setHoldSeconds(0);
    lastHoldT.current = null;
  };
  const parts = midiFrac !== null ? noteParts(midiFrac) : null;
  const scaleOff = midiFrac !== null && scale ? centsFromScale(scale, midiFrac) : null;

  // Practice: the target counts as hit after a continuous hold within tolerance.
  useEffect(() => {
    const within = midiFrac !== null && Math.abs((midiFrac - targetMidi) * 100) <= PRACTICE_TOLERANCE_CENTS;
    if (!within) {
      heldSince.current = null;
      return;
    }
    const now = performance.now();
    if (heldSince.current === null) heldSince.current = now;
    else if (now - heldSince.current >= PRACTICE_HOLD_MS) setHit(true);
  }, [midiFrac, targetMidi]);

  const needle = parts ? Math.max(-50, Math.min(50, parts.cents)) : 0;
  // Profile mark: nearest calibrated note (by its measured pitch) and the distance to it.
  const profileMark = (() => {
    if (!profile || midiFrac === null || profile.notes.length === 0) return null;
    let best: { midi: number; off: number } | null = null;
    for (const n of profile.notes) {
      const off = (midiFrac - (n.midi + n.cents_offset / 100)) * 100;
      if (!best || Math.abs(off) < Math.abs(best.off)) best = { midi: n.midi, off };
    }
    return best;
  })();
  const written = parts && profile && profile.transpose_semitones !== 0 ? noteParts(parts.midi + profile.transpose_semitones) : null;
  const scaleClass = scaleOff === null ? "" : Math.abs(scaleOff) <= IN_SCALE_TOLERANCE_CENTS ? "in" : "out";
  // F2.6: the owner's kaval chart next to the reading when the profile is a kaval.
  const chart = chartFor(profile?.instrument, useStore((st) => st.fingeringOverrides));

  return (
    <section className="tuner" aria-label={t.tuner} aria-live="polite">
      <div className="tuner-main">
        <div className={`tuner-note ${scaleClass}`}>
          {!active ? (
            <span className="muted">{t.tunerOff}</span>
          ) : parts ? (
            <>
              <span className="tuner-name">
                {t.noteNames[parts.pc]}
                <sub>{parts.octave}</sub>
              </span>
              <span className="mono-num small">
                {hz!.toFixed(1)} Hz · {parts.cents >= 0 ? "+" : ""}
                {parts.cents.toFixed(0)} c · {t.confidence} {Math.round(confidence * 100)} %
              </span>
            </>
          ) : (
            <span className="muted">{t.tunerSilent}</span>
          )}
        </div>
        <div className="needle-track" role="meter" aria-valuemin={-50} aria-valuemax={50} aria-valuenow={Math.round(needle)} aria-label={t.tuner}>
          <div className="needle-centre" />
          {profileMark && parts && (
            <div
              className="needle profile-mark"
              style={{ left: `${50 + Math.max(-50, Math.min(50, needle - profileMark.off))}%` }}
              title={t.profileMark}
            />
          )}
          {parts && <div className="needle" style={{ left: `${50 + needle}%` }} />}
        </div>
        <div className="small muted">
          {scale === null ? t.noScaleChosen : scaleOff === null ? "" : scaleClass === "in" ? `✓ ${t.inScale}` : `△ ${t.outOfScale} (${scaleOff > 0 ? "+" : ""}${scaleOff.toFixed(0)} c)`}
          {profileMark && parts && (
            <>
              {" "}
              · {t.profileMark}: {t.noteNames[noteParts(profileMark.midi).pc]}
              {noteParts(profileMark.midi).octave} {profileMark.off >= 0 ? "+" : ""}
              {profileMark.off.toFixed(0)} c
            </>
          )}
          {written && (
            <>
              {" "}
              · {t.writtenNote}: {t.noteNames[written.pc]}
              {written.octave}
            </>
          )}
        </div>
      </div>

      {chart && active && parts && <FingeringDiagram chart={chart} midi={parts.midi} compact showAlternatives={false} />}
      <div className="practice">
        <label className="inline small">
          <input
            type="checkbox"
            checked={holdOn}
            onChange={(e) => {
              setHoldOn(e.target.checked);
              resetHold();
            }}
          />
          {t.holdTone}
        </label>
        {holdOn && (
          <div className="hold" title={t.holdHint}>
            <HoldStrip samples={holdSamples} now={holdNow} />
            <span className="small mono-num">
              {t.holdTarget}:{" "}
              {holdRef !== null ? `${t.noteNames[noteParts(holdRef).pc]}${noteParts(holdRef).octave}` : "—"} · {holdSeconds.toFixed(1)} {t.holdSeconds}
            </span>
            <button className="ghost small" onClick={resetHold}>
              {t.holdReset}
            </button>
          </div>
        )}
        <span className="small muted">{t.hitTheNote}</span>
        <label className="inline">
          {t.target}
          <select
            value={targetMidi}
            onChange={(e) => {
              setTargetMidi(Number(e.target.value));
              setHit(false);
              heldSince.current = null;
            }}
          >
            {Array.from({ length: 36 }, (_, i) => 48 + i).map((m) => {
              const p = noteParts(m);
              return (
                <option key={m} value={m}>
                  {t.noteNames[p.pc]}
                  {p.octave}
                </option>
              );
            })}
          </select>
        </label>
        <span className={`badge ${hit ? "ok" : ""}`}>{hit ? t.hit : t.hold}</span>
        {hit && (
          <button className="ghost small" onClick={() => { setHit(false); heldSince.current = null; }}>
            ↺
          </button>
        )}
      </div>
    </section>
  );
}
