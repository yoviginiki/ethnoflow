import { useEffect, useState } from "react";
import { api, describeError, type DroneSettings, type DroneStatus, type InstrumentProfile, type Scale } from "./api";
import { formatDb, linearToDb, meterFraction, noteParts } from "./format";
import { messages, type StringMessageKey } from "./i18n";
import { DEFAULT_DRONE, useStore, type DronePrefs } from "./store";

interface Props {
  scale: Scale | null;
  profile: InstrumentProfile | null;
}

const ROOT_RANGE = Array.from({ length: 49 }, (_, i) => 36 + i); // C2 … C6

type PresetKey = "dronePresetTraditional" | "dronePresetDeep" | "dronePresetCinematic";
const PRESETS: Record<PresetKey, Partial<DronePrefs>> = {
  dronePresetTraditional: { fifth: 0.6, octave: 0.3, sub: 0, warmth: 0.4, movement: 0.2, breath: 0.3, detuneCents: 6, cutoffHz: 2500 },
  dronePresetDeep: { fifth: 0.5, octave: 0.15, sub: 0.6, warmth: 0.7, movement: 0.4, breath: 0.4, detuneCents: 9, cutoffHz: 1200 },
  dronePresetCinematic: { fifth: 0.3, octave: 0.5, sub: 0.3, warmth: 0.3, movement: 0.8, breath: 0.6, detuneCents: 12, cutoffHz: 4000 },
};

type SliderKey = "levelDb" | "fifth" | "octave" | "sub" | "warmth" | "movement" | "breath";
const SLIDERS: { key: SliderKey; label: StringMessageKey; min: number; max: number; step: number }[] = [
  { key: "levelDb", label: "droneLevel", min: -40, max: 0, step: 1 },
  { key: "fifth", label: "droneFifth", min: 0, max: 1, step: 0.01 },
  { key: "octave", label: "droneOctave", min: 0, max: 1, step: 0.01 },
  { key: "sub", label: "droneSub", min: 0, max: 1, step: 0.01 },
  { key: "warmth", label: "droneWarmth", min: 0, max: 1, step: 0.01 },
  { key: "movement", label: "droneMovement", min: 0, max: 1, step: 0.01 },
  { key: "breath", label: "droneBreath", min: 0, max: 1, step: 0.01 },
];

function toSettings(p: DronePrefs, rootMidi: number): DroneSettings {
  return {
    root_midi: rootMidi,
    root_cents: p.rootCents,
    fifth: p.fifth,
    octave: p.octave,
    sub: p.sub,
    warmth: p.warmth,
    movement: p.movement,
    breath: p.breath,
    detune_cents: p.detuneCents,
    cutoff_hz: p.cutoffHz,
    level_db: p.levelDb,
    intonation: p.intonation,
  };
}

/** Drone on the audio graph (F3.2): root from the user, intonation from the profile. */
export function DronePanel({ scale, profile }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const prefs = s.prefs.drone ?? DEFAULT_DRONE;
  const [on, setOn] = useState(false);
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState<DroneStatus | null>(null);
  // Suggested root: the project's scale tonic, else the lowest calibrated note. Never
  // a built-in key — if nothing is known the user picks one.
  const suggested = scale ? 48 + scale.tonic_pc : (profile?.notes[0]?.midi ?? null);
  const rootMidi = prefs.rootMidi ?? suggested;
  const set = (patch: Partial<DronePrefs>) => s.setPref("drone", { ...prefs, ...patch });
  const fail = (e: unknown) => s.setError(describeError(e));

  useEffect(() => {
    if (!on || rootMidi === null) return;
    let cancelled = false;
    api
      .startDrone(s.prefs.selectedOutput, toSettings(prefs, rootMidi))
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
      void api.stopDrone().catch(() => undefined);
    };
    // Restart only on device change; parameter edits go through set_drone below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [on, s.prefs.selectedOutput]);

  useEffect(() => {
    if (!running || rootMidi === null) return;
    api.setDrone(toSettings(prefs, rootMidi)).then(setStatus).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running, prefs, rootMidi, profile?.version, profile?.id]);

  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => {
      api
        .droneStatus()
        .then((st) => {
          if (st) setStatus(st);
        })
        .catch(() => undefined);
    }, 200);
    return () => window.clearInterval(id);
  }, [running]);

  const profileHasRoot = rootMidi !== null && !!profile?.notes.some((n) => n.midi === rootMidi);
  const peakDb = status ? linearToDb(status.layer_peak) : -60;

  return (
    <div className="panel drone">
      <div className="analysis-row">
        <label className="inline">
          <input type="checkbox" checked={on} disabled={rootMidi === null} onChange={(e) => setOn(e.target.checked)} />
          <strong>{t.drone}</strong>
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
          {t.droneFine}
          <input type="number" min={-100} max={100} step={1} value={prefs.rootCents} onChange={(e) => set({ rootCents: Math.max(-100, Math.min(100, Number(e.target.value) || 0)) })} />
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
        <label>
          {t.dronePreset}
          <select value="" onChange={(e) => e.target.value && set(PRESETS[e.target.value as PresetKey])}>
            <option value="">…</option>
            {(Object.keys(PRESETS) as PresetKey[]).map((k) => (
              <option key={k} value={k}>
                {t[k]}
              </option>
            ))}
          </select>
        </label>
      </div>
      {prefs.intonation === "profile" && profile && rootMidi !== null && !profileHasRoot && <div className="muted small">{t.droneNoProfileNote}</div>}
      <div className="drone-sliders">
        {SLIDERS.map(({ key, label, min, max, step }) => (
          <label key={key} className="drone-slider">
            <span>{t[label]}</span>
            <input type="range" min={min} max={max} step={step} value={prefs[key]} onChange={(e) => set({ [key]: Number(e.target.value) })} />
            <span className="mono-num small">{key === "levelDb" ? formatDb(prefs.levelDb) : Math.round(prefs[key] * 100) + "%"}</span>
          </label>
        ))}
      </div>
      {status && (
        <div className="analysis-row small">
          <span className="mono-num">
            {t.droneRoot}: {status.root_hz.toFixed(2)} Hz ({status.root_cents_applied >= 0 ? "+" : ""}
            {status.root_cents_applied.toFixed(1)} ¢) · {t.droneFifth} {status.fifth_cents_applied >= 0 ? "+" : ""}
            {status.fifth_cents_applied.toFixed(1)} ¢ · {status.intonation_used === "profile" ? t.droneIntProfile : t.droneIntTempered}
          </span>
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
      <div className="muted small">{t.droneHint}</div>
    </div>
  );
}
