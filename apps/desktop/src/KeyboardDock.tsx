import { useEffect, useRef, useState } from "react";
import { isTypingTarget, noteParts } from "./format";
import { messages } from "./i18n";
import { BLACK, codeForKey, keyboardAllOff, keyboardNote, midiForCode, moduleSources, OCTAVE_MAX, OCTAVE_MIN, type KeyboardSource } from "./keyboard";
import { useStore } from "./store";

const KEYS = 29;

/**
 * The keyboard dock (K1): a strip above the "Now playing" bar on every screen. Keys with
 * the mouse, the computer keyboard (Z–M / Q–P, ← → octaves) and the MIDI port; the sound
 * source is the project's voice or the instrument being edited (K2). It never records.
 */
export function KeyboardDock() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const on = s.prefs.keyboardOn;
  const base = s.prefs.keyboardOctave;
  const lock = s.prefs.keyboardLock;
  const scale = s.project?.scale ?? null;
  const [pressed, setPressed] = useState<Set<number>>(new Set());
  const held = useRef<Map<string, number>>(new Map());
  const mouseNote = useRef<number | null>(null);

  const setBase = (b: number) => s.setPref("keyboardOctave", Math.max(OCTAVE_MIN, Math.min(OCTAVE_MAX, b)));

  // Computer keys while the dock is on.
  useEffect(() => {
    if (!on) return;
    const down = (e: KeyboardEvent) => {
      if (isTypingTarget(e.target) || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        e.stopPropagation();
        setBase(base + (e.key === "ArrowLeft" ? -12 : 12));
        return;
      }
      const code = codeForKey(e.key);
      if (code === null || e.repeat) return;
      e.preventDefault();
      e.stopPropagation();
      const key = e.key.toLowerCase();
      if (held.current.has(key)) return;
      const midi = midiForCode(base, code, scale, lock);
      held.current.set(key, midi);
      setPressed(new Set(held.current.values()));
      void keyboardNote(midi, true).catch(() => undefined);
    };
    const up = (e: KeyboardEvent) => {
      const key = e.key.toLowerCase();
      const midi = held.current.get(key);
      if (midi === undefined) return;
      e.stopPropagation();
      held.current.delete(key);
      setPressed(new Set(held.current.values()));
      void keyboardNote(midi, false).catch(() => undefined);
    };
    const blur = () => {
      held.current.clear();
      setPressed(new Set());
      void keyboardAllOff();
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
      window.removeEventListener("blur", blur);
      blur();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [on, base, lock, scale]);

  if (!on) return null;

  const keys = Array.from({ length: KEYS }, (_, i) => base + i);
  const mouseDown = (m: number) => {
    const midi = midiForCode(0, m, scale, lock);
    mouseNote.current = midi;
    setPressed((p) => new Set([...p, midi]));
    void keyboardNote(midi, true).catch(() => undefined);
  };
  const mouseUp = () => {
    const midi = mouseNote.current;
    if (midi === null) return;
    mouseNote.current = null;
    setPressed((p) => {
      const n = new Set(p);
      n.delete(midi);
      return n;
    });
    void keyboardNote(midi, false).catch(() => undefined);
  };
  const srcLabel = (k: KeyboardSource) => (k === "voice" ? t.kbSourceVoice : k === "instrument" ? t.kbSourceInstrument : k === "pad" ? t.kbSourcePad : t.kbSourceDrone);
  const modules = moduleSources();
  const kindName: Record<string, string> = { bells: t.kindBells, pluck: t.kindPluck, resonator: t.kindResonator, sampler: t.kindSampler };

  return (
    <div className="kb-dock" role="region" aria-label={t.kbDock} data-coach="keyboard-dock">
      <div className="analysis-row small">
        <strong>⌨ {t.kbDock}</strong>
        <label className="inline">
          {t.kbSource}
          <select value={s.prefs.keyboardSource} onChange={(e) => s.setPref("keyboardSource", e.target.value as KeyboardSource)}>
            {(["voice", "instrument", "pad", "drone"] as KeyboardSource[]).map((k) => (
              <option key={k} value={k}>
                {srcLabel(k)}
              </option>
            ))}
            {modules.map((m) => (
              <option key={m.id} value={m.id}>
                {t.kbSourceModule}: {m.name} ({kindName[m.kind] ?? m.kind}, {m.rack === "ambient" ? t.areaAmbient : t.scapesTitle})
              </option>
            ))}
          </select>
        </label>
        {s.prefs.keyboardSource === "instrument" && !s.designerVoice && <span className="muted">{t.kbNoInstrument}</span>}
        <label className="inline">
          {t.kbMidi}
          <select value={s.prefs.midiInput ?? ""} onChange={(e) => s.setPref("midiInput", e.target.value || null)}>
            <option value="">{t.midiNone}</option>
            {s.midiPorts.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        </label>
        <label className="inline">
          <input type="checkbox" checked={lock} disabled={!scale} onChange={(e) => s.setPref("keyboardLock", e.target.checked)} />
          {t.scaleLock}
        </label>
        <span className="muted">
          {t.octave}: {t.noteNames[noteParts(base).pc]}
          {noteParts(base).octave} … {t.noteNames[noteParts(base + KEYS - 1).pc]}
          {noteParts(base + KEYS - 1).octave}
        </span>
        <button className="ghost small" onClick={() => setBase(base - 12)} disabled={base <= OCTAVE_MIN}>
          ←
        </button>
        <button className="ghost small" onClick={() => setBase(base + 12)} disabled={base >= OCTAVE_MAX}>
          →
        </button>
        <label className="inline">
          {t.kbVelocity}
          <input type="range" min={0.05} max={1} step={0.05} value={s.prefs.keyboardVelocity} onChange={(e) => s.setPref("keyboardVelocity", Number(e.target.value))} />
          <span className="mono-num">{Math.round(s.prefs.keyboardVelocity * 100)}%</span>
        </label>
        <span className="spacer" />
        <button className="ghost small" onClick={() => s.setPref("keyboardOn", false)} aria-label={t.close}>
          ✕
        </button>
      </div>
      <div className="kb-keys" onPointerUp={mouseUp} onPointerLeave={mouseUp} onPointerCancel={mouseUp}>
        {keys.map((m) => {
          const pc = ((m % 12) + 12) % 12;
          const inScale = scale ? Math.abs(centsOff(scale, m)) < 1 : true;
          const down = pressed.has(m) || s.midiHeld.includes(m);
          return (
            <button
              key={m}
              type="button"
              className={`kb-key ${BLACK.has(pc) ? "black" : ""} ${down ? "down" : ""} ${inScale ? "" : "dim"}`}
              title={`${t.noteNames[pc]}${noteParts(m).octave}`}
              aria-label={`${t.noteNames[pc]}${noteParts(m).octave}`}
              onPointerDown={(e) => {
                e.preventDefault();
                mouseDown(m);
              }}
            />
          );
        })}
      </div>
      <div className="muted small">{t.kbDockHint}</div>
    </div>
  );
}

function centsOff(scale: { tonic_pc: number; degrees_cents: number[] }, midi: number): number {
  // Same rule as the scale lock: distance to the nearest degree.
  const rel = (((midi - scale.tonic_pc) % 12) + 12) % 12;
  const cents = rel * 100;
  let best = Infinity;
  for (const d of scale.degrees_cents) {
    for (const k of [-1200, 0, 1200]) best = Math.min(best, Math.abs(cents - (d + k)));
  }
  return best;
}
