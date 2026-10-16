import { useEffect, useRef, useState } from "react";
import type { Scale } from "./api";
import { centsFromScale, hzToMidi, noteParts } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

interface Props {
  hz: number | null;
  confidence: number;
  a4: number;
  scale: Scale | null;
  /** Null = tuner off (shows a hint instead of a reading). */
  active: boolean;
}

const IN_SCALE_TOLERANCE_CENTS = 25;
const PRACTICE_TOLERANCE_CENTS = 15;
const PRACTICE_HOLD_MS = 1000;

/**
 * Live tuner: note name, cents needle, Hz, confidence, in-scale indicator when the
 * project has a chosen scale. Includes the first practice: hold a target note.
 */
export function Tuner({ hz, confidence, a4, scale, active }: Props) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const [targetMidi, setTargetMidi] = useState(69);
  const [hit, setHit] = useState(false);
  const heldSince = useRef<number | null>(null);

  const midiFrac = hz !== null && confidence >= 0.5 ? hzToMidi(hz, a4) : null;
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
  const scaleClass = scaleOff === null ? "" : Math.abs(scaleOff) <= IN_SCALE_TOLERANCE_CENTS ? "in" : "out";

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
          {parts && <div className="needle" style={{ left: `${50 + needle}%` }} />}
        </div>
        <div className="small muted">
          {scale === null ? t.noScaleChosen : scaleOff === null ? "" : scaleClass === "in" ? `✓ ${t.inScale}` : `△ ${t.outOfScale} (${scaleOff > 0 ? "+" : ""}${scaleOff.toFixed(0)} c)`}
        </div>
      </div>

      <div className="practice">
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
