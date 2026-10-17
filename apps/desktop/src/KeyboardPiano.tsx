import { useEffect, useRef, useState } from "react";
import { api, describeError, type Scale } from "./api";
import { centsFromScale, isTypingTarget, noteParts } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

/** Two rows of the computer keyboard = two octaves (lower row from C). */
const LOWER: Record<string, number> = { z: 0, s: 1, x: 2, d: 3, c: 4, v: 5, g: 6, b: 7, h: 8, n: 9, j: 10, m: 11, ",": 12, l: 13, ".": 14 };
const UPPER: Record<string, number> = { q: 12, "2": 13, w: 14, "3": 15, e: 16, r: 17, "5": 18, t: 19, "6": 20, y: 21, "7": 22, u: 23, i: 24, "9": 25, o: 26, "0": 27, p: 28 };
const BLACK = new Set([1, 3, 6, 8, 10]);

interface Props {
  scale: Scale | null;
}

/** Play the live synth from the computer keyboard; optional Scale Lock. */
export function KeyboardPiano({ scale }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [on, setOn] = useState(false);
  const [base, setBase] = useState(60); // C4 at the lower row
  const [lock, setLock] = useState(true);
  const [pressed, setPressed] = useState<Set<number>>(new Set());
  const held = useRef<Map<string, number>>(new Map());

  const toMidi = (code: number) => {
    let midi = base + code;
    if (lock && scale) {
      // Snap to the nearest scale degree (never silently assume a scale: only when chosen).
      const off = centsFromScale(scale, midi);
      midi = Math.round(midi - off / 100);
    }
    return Math.max(0, Math.min(127, midi));
  };

  useEffect(() => {
    if (!on) return;
    let cancelled = false;
    api.startLiveSynth(s.prefs.selectedOutput, s.prefs.voice).catch((e) => {
      s.setError(describeError(e));
      setOn(false);
    });
    const down = (e: KeyboardEvent) => {
      if (cancelled || isTypingTarget(e.target) || e.ctrlKey || e.altKey || e.metaKey) return;
      const key = e.key.toLowerCase();
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        e.stopPropagation();
        setBase((b) => Math.max(24, Math.min(96, b + (e.key === "ArrowLeft" ? -12 : 12))));
        return;
      }
      const code = LOWER[key] ?? UPPER[key];
      if (code === undefined || e.repeat) return;
      e.preventDefault();
      e.stopPropagation();
      if (held.current.has(key)) return;
      const midi = toMidi(code);
      held.current.set(key, midi);
      setPressed(new Set(held.current.values()));
      void api.synthNote(midi, true, 0.7).catch(() => undefined);
    };
    const up = (e: KeyboardEvent) => {
      const key = e.key.toLowerCase();
      const midi = held.current.get(key);
      if (midi === undefined) return;
      e.stopPropagation();
      held.current.delete(key);
      setPressed(new Set(held.current.values()));
      void api.synthNote(midi, false, null).catch(() => undefined);
    };
    const blur = () => {
      held.current.clear();
      setPressed(new Set());
      void api.synthAllOff().catch(() => undefined);
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    window.addEventListener("blur", blur);
    return () => {
      cancelled = true;
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
      window.removeEventListener("blur", blur);
      held.current.clear();
      setPressed(new Set());
      void api.stopLiveSynth().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [on, base, lock, scale, s.prefs.selectedOutput, s.prefs.voice]);

  const keys = Array.from({ length: 29 }, (_, i) => base + i);
  const midiHeld = s.midiHeld;
  return (
    <div className="panel kb-piano">
      <div className="analysis-row">
        <label className="inline">
          <input type="checkbox" checked={on} onChange={(e) => setOn(e.target.checked)} />
          {t.keyboardPiano}
        </label>
        <label className="inline">
          <input type="checkbox" checked={lock} disabled={!scale} onChange={(e) => setLock(e.target.checked)} />
          {t.scaleLock}
        </label>
        <span className="small muted">
          {t.octave}: {t.noteNames[noteParts(base).pc]}
          {noteParts(base).octave} … {t.noteNames[noteParts(base + 28).pc]}
          {noteParts(base + 28).octave}
        </span>
        <button className="ghost small" onClick={() => setBase((b) => Math.max(24, b - 12))} disabled={!on}>
          ←
        </button>
        <button className="ghost small" onClick={() => setBase((b) => Math.min(96, b + 12))} disabled={!on}>
          →
        </button>
      </div>
      {(on || s.prefs.midiInput) && (
        <div className="kb-keys" aria-hidden="true">
          {keys.map((m) => {
            const pc = ((m % 12) + 12) % 12;
            const inScale = scale ? Math.abs(centsFromScale(scale, m)) < 1 : true;
            return (
              <span
                key={m}
                className={`kb-key ${BLACK.has(pc) ? "black" : ""} ${pressed.has(m) || midiHeld.includes(m) ? "down" : ""} ${inScale ? "" : "dim"}`}
                title={`${t.noteNames[pc]}${noteParts(m).octave}`}
              />
            );
          })}
        </div>
      )}
      <div className="muted small">{t.keyboardHint}</div>
    </div>
  );
}
