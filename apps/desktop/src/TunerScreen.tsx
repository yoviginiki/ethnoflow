import { useCallback, useEffect, useState } from "react";
import { api, describeError } from "./api";
import { FingeringDiagram } from "./FingeringDiagram";
import { chartFor } from "./fingerings";
import { hzToMidi, noteParts } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";
import {
  A4_CHOICES,
  A4_MAX,
  A4_MIN,
  clampA4,
  GAUGE_CENTS,
  type HeldTone,
  holdTone,
  IN_TUNE_CENTS,
  lockRange,
  monitorMinHz,
  needleAngle,
  needsLock,
  readTuner,
  smoothPitch,
  targetsFor,
  TUNER_INSTRUMENTS,
  type TunerInstrument,
} from "./tunerTargets";

const POLL_MS = 50;

/** Analog needle: ±GAUGE_CENTS over ±45°, green zone ±IN_TUNE_CENTS. */
function Gauge({ cents, inTune }: { cents: number | null; inTune: boolean }) {
  const R = 86;
  const polar = (deg: number, r: number): [number, number] => {
    const a = ((deg - 90) * Math.PI) / 180;
    return [100 + r * Math.cos(a), 100 + r * Math.sin(a)];
  };
  const arc = (from: number, to: number, r: number) => {
    const [x1, y1] = polar(from, r);
    const [x2, y2] = polar(to, r);
    return `M ${x1.toFixed(2)} ${y1.toFixed(2)} A ${r} ${r} 0 0 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`;
  };
  const ang = cents === null ? 0 : needleAngle(cents);
  const [nx, ny] = polar(ang, R - 6);
  const ticks: { deg: number; major: boolean; label: string | null }[] = [];
  for (let c = -GAUGE_CENTS; c <= GAUGE_CENTS; c += 10) {
    ticks.push({ deg: needleAngle(c), major: c % 50 === 0 || c === 0, label: c % 25 === 0 ? `${c > 0 ? "+" : ""}${c}` : null });
  }
  return (
    <svg viewBox="0 0 200 112" className={`gauge ${cents === null ? "idle" : inTune ? "in" : "out"}`} role="meter" aria-valuemin={-GAUGE_CENTS} aria-valuemax={GAUGE_CENTS} aria-valuenow={cents === null ? 0 : Math.round(cents)}>
      <path d={arc(-45, 45, R)} className="gauge-arc" />
      <path d={arc(needleAngle(-IN_TUNE_CENTS), needleAngle(IN_TUNE_CENTS), R)} className="gauge-zone" />
      {ticks.map((tk) => {
        const [x1, y1] = polar(tk.deg, R);
        const [x2, y2] = polar(tk.deg, R - (tk.major ? 10 : 6));
        const [lx, ly] = polar(tk.deg, R + 9);
        return (
          <g key={tk.deg}>
            <line x1={x1} y1={y1} x2={x2} y2={y2} className={`gauge-tick ${tk.major ? "major" : ""}`} />
            {tk.label && (
              <text x={lx} y={ly + 3} textAnchor="middle" className="gauge-label">
                {tk.label}
              </text>
            )}
          </g>
        );
      })}
      <line x1={100} y1={100} x2={nx} y2={ny} className="gauge-needle" />
      <circle cx={100} cy={100} r={4} className="gauge-pivot" />
      <text x={100} y={111} textAnchor="middle" className="gauge-cents">
        {cents === null ? "—" : `${cents >= 0 ? "+" : ""}${cents.toFixed(0)} c`}
      </text>
    </svg>
  );
}

/**
 * Tuner screen: the microphone's pitch against the targets of a chosen instrument
 * (kaval D, shakuhachi D, guitar strings, a monochord bed on one note, or free
 * chromatic) at a chosen reference (440, 432 or typed). Settings persist in prefs.
 */
export function TunerScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project;
  const a4 = s.prefs.tunerA4 ?? project?.a4_hz ?? 440;
  const inst = s.prefs.tunerInstrument;
  const lock = s.prefs.tunerLock;
  // The tone as shown (held across dropped frames, with hysteresis: no blinking) and its
  // smoothed fractional MIDI, which the note name and the needle both use. One state so
  // the two never disagree for a render (a tone of null with a note still set crashed
  // the screen on 2026-10-31).
  const [view, setView] = useState<{ tone: HeldTone | null; midi: number | null }>({ tone: null, midi: null });
  const [a4Draft, setA4Draft] = useState(String(a4));
  useEffect(() => setA4Draft(String(a4)), [a4]);
  const fail = useCallback((e: unknown) => useStore.getState().setError(describeError(e)), []);

  // Devices, so the microphone can be chosen here without visiting "Play & record".
  useEffect(() => {
    if (s.inputDevices.length > 0) return;
    void Promise.all([api.listInputDevices(), api.listOutputDevices(), api.audioHost()])
      .then(([ins, outs, host]) => useStore.getState().setDevices(ins, outs, `${host.host} / ${host.os}`))
      .catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The input monitor runs while this screen is open and nothing records or plays. The
  // bass range (30 Hz) is requested for the sound bed and for a locked low note.
  const minHz = monitorMinHz(s.prefs.tunerInstrument, s.prefs.tunerLock, a4);
  useEffect(() => {
    if (s.mode !== "idle") return;
    let cancelled = false;
    const p = useStore.getState().prefs;
    void api.startMonitor(p.selectedInput, p.sampleRate, minHz).catch(fail);
    const tick = async () => {
      if (cancelled) return;
      try {
        useStore.getState().setMonitor(await api.monitorStatus());
      } catch (e) {
        fail(e);
        return;
      }
      setTimeout(tick, POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
      void api.stopMonitor().catch(() => undefined);
      useStore.getState().setMonitor(null);
    };
  }, [s.mode, s.prefs.selectedInput, s.prefs.sampleRate, minHz, fail]);

  // Every monitor poll (50 ms) updates the held tone and the smoothing of its pitch.
  useEffect(() => {
    const m = s.monitor;
    setView((prev) => {
      const tone = m === null ? null : holdTone(prev.tone, m.pitch_hz, m.pitch_confidence, performance.now());
      const midi = tone === null ? null : smoothPitch(prev.midi, hzToMidi(tone.hz, a4));
      return { tone, midi };
    });
  }, [s.monitor, a4]);

  const tone = view.tone;
  const hz = tone?.hz ?? null;
  const confidence = tone?.confidence ?? 0;
  const midiFrac = tone === null ? null : view.midi;
  const targets = targetsFor(inst, lock);
  const reading = midiFrac !== null ? readTuner(midiFrac, targets) : null;
  const needle = reading?.cents ?? null;

  const parts = midiFrac !== null ? noteParts(midiFrac) : null;
  const targetParts = reading ? noteParts(reading.target.midi) : null;
  const inTune = reading?.direction === 0;
  const chart = inst === "kaval-d" ? chartFor("kaval", s.fingeringOverrides) : null;
  const [lockLo, lockHi] = lockRange(inst);
  const instLabel = (i: TunerInstrument) => t.tunerInst[i];

  const applyA4 = () => {
    const v = clampA4(Number(a4Draft.replace(",", ".")), a4);
    s.setPref("tunerA4", v);
  };
  const applyToProject = async () => {
    try {
      useStore.getState().setProject(await api.setReference(a4));
      s.setMessage(`${t.tunerA4Applied}: ${a4} Hz`);
    } catch (e) {
      fail(e);
    }
  };

  const direction =
    reading === null
      ? null
      : reading.direction === 0
        ? `✓ ${t.tunerInTune}`
        : reading.direction < 0
          ? `▲ ${t.tunerTooLow} (${reading.cents.toFixed(0)} c)`
          : `▼ ${t.tunerTooHigh} (+${reading.cents.toFixed(0)} c)`;

  return (
    <div className="tuner-screen">
      <section className="panel tuner-controls">
        <label className="inline">
          {t.tunerInstrument}
          <select value={inst} onChange={(e) => s.setPref("tunerInstrument", e.target.value as TunerInstrument)}>
            {TUNER_INSTRUMENTS.map((i) => (
              <option key={i} value={i}>
                {instLabel(i)}
              </option>
            ))}
          </select>
        </label>
        <label className="inline">
          {t.tunerTargetNote}
          <select value={lock ?? ""} onChange={(e) => s.setPref("tunerLock", e.target.value === "" ? null : Number(e.target.value))}>
            <option value="">{needsLock(inst) ? t.tunerPickNote : t.tunerAuto}</option>
            {Array.from({ length: lockHi - lockLo + 1 }, (_, i) => lockLo + i).map((m) => {
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
        <span className="inline tuner-a4" title={t.tunerA4Hint}>
          <span>{t.tunerA4}</span>
          {A4_CHOICES.map((v) => (
            <button key={v} className={`small ${a4 === v ? "on" : "ghost"}`} onClick={() => s.setPref("tunerA4", v)}>
              {v}
            </button>
          ))}
          <input
            inputMode="decimal"
            size={6}
            min={A4_MIN}
            max={A4_MAX}
            value={a4Draft}
            onChange={(e) => setA4Draft(e.target.value)}
            onBlur={applyA4}
            onKeyDown={(e) => e.key === "Enter" && applyA4()}
            aria-label={t.tunerA4}
          />
          <span className="muted small">Hz</span>
          {project && s.prefs.tunerA4 !== null && (
            <button className="ghost small" onClick={() => s.setPref("tunerA4", null)} title={t.tunerA4FromProjectHint}>
              {t.tunerA4FromProject} ({project.a4_hz})
            </button>
          )}
          {project && project.a4_hz !== a4 && (
            <button className="ghost small" onClick={() => void applyToProject()} title={t.tunerA4ApplyHint}>
              {t.tunerA4Apply}
            </button>
          )}
        </span>
        <label className="inline">
          {t.inputDevice}
          <select
            value={s.prefs.selectedInput ?? ""}
            onChange={(e) => {
              s.setPref("selectedInput", e.target.value || null);
              s.setPref("sampleRate", null);
            }}
          >
            <option value="">{t.defaultDevice}</option>
            {s.inputDevices.map((d) => (
              <option key={d.name} value={d.name}>
                {d.name}
              </option>
            ))}
          </select>
        </label>
      </section>

      <section className={`panel tuner-reading ${inTune ? "in" : reading ? "out" : ""} ${tone && !tone.live ? "held" : ""}`} aria-live="polite">
        <div className="tuner-big">
          {s.mode !== "idle" ? (
            <span className="muted">{t.tunerBusy}</span>
          ) : parts && hz !== null ? (
            <>
              <span className="tuner-name xl">
                {t.noteNames[parts.pc]}
                <sub>{parts.octave}</sub>
              </span>
              <span className="mono-num small">
                {hz.toFixed(1)} Hz · {t.confidence} {Math.round(confidence * 100)} %
              </span>
            </>
          ) : (
            <span className="muted">{needsLock(inst) && lock === null ? t.tunerPickNoteHint : t.tunerSilent}</span>
          )}
        </div>
        <Gauge cents={needle} inTune={inTune} />
        <div className="tuner-direction">
          {reading && targetParts && (
            <span className="small">
              {t.tunerTargetNote}: <b>{reading.target.label ?? `${t.noteNames[targetParts.pc]}${targetParts.octave}`}</b>
              {reading.target.label && (
                <span className="muted">
                  {" "}
                  ({t.noteNames[targetParts.pc]}
                  {targetParts.octave})
                </span>
              )}
            </span>
          )}
          <span className={`tuner-verdict ${reading ? (inTune ? "ok" : "bad") : ""}`}>{direction ?? ""}</span>
        </div>
        {chart && (
          <div className="tuner-fingering">
            {parts && <FingeringDiagram chart={chart} midi={parts.midi} compact showAlternatives={false} />}
          </div>
        )}
      </section>
      <div className="muted small">{t.tunerHint}</div>
      {minHz !== null && (
        <div className="muted small">{t.tunerBassRange(minHz, s.monitor?.min_hz ?? null)}</div>
      )}
    </div>
  );
}
