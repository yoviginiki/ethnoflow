import { useState } from "react";
import { api, describeError, type ArpDirection, type GrooveStyle, type RhythmPrefs } from "./api";
import { formatDb, linearToDb, meterFraction, noteParts } from "./format";
import { Knob } from "./Knob";
import { messages, type StringMessageKey } from "./i18n";
import { PresetLibrary } from "./PresetLibrary";
import { DEFAULT_RHYTHM, useStore } from "./store";

const pct = (v: number) => `${Math.round(v * 100)}%`;
const STYLES: GrooveStyle[] = ["liquid", "halftime", "ethno"];
const STYLE_LABEL: Record<GrooveStyle, StringMessageKey> = { liquid: "grooveLiquid", halftime: "grooveHalftime", ethno: "grooveEthno" };
const DIRECTIONS: ArpDirection[] = ["up", "down", "up_down", "random"];
const DIR_LABEL: Record<ArpDirection, StringMessageKey> = { up: "arpUp", down: "arpDown", up_down: "arpUpDown", random: "arpRandom" };
const BASS_VOICES: StringMessageKey[] = ["bassSub", "bassPluck", "bassReese"];
const TIMBRES: StringMessageKey[] = ["voiceSine", "voiceTriangle", "voiceFlute"];
const REGISTER = Array.from({ length: 73 }, (_, i) => 24 + i);

interface SliderSpec {
  key: string;
  label: StringMessageKey;
  min: number;
  max: number;
  step: number;
  fmt: (v: number) => string;
}

function Sliders<T extends object>({ specs, value, onChange, disabled }: { specs: SliderSpec[]; value: T; onChange: (patch: Partial<T>) => void; disabled: boolean }) {
  return (
    <div className="drone-sliders">
      {specs.map((sp) => {
        const v = (value as unknown as Record<string, number>)[sp.key];
        return (
          <label key={sp.key} className="drone-slider">
            <span>{messages[useStore.getState().prefs.lang][sp.label]}</span>
            <input type="range" min={sp.min} max={sp.max} step={sp.step} value={v} disabled={disabled} onChange={(e) => onChange({ [sp.key]: Number(e.target.value) } as Partial<T>)} />
            <span className="mono-num small">{sp.fmt(v)}</span>
          </label>
        );
      })}
    </div>
  );
}

/**
 * Rhythm section (Phase 4): drums, bass, ostinato and arpeggio on one beat clock,
 * following the project's tempo/meter and the current chord. Runs through
 * `useBackgroundLayers` like the drone and the pad; this panel edits the prefs.
 */
export function RhythmPanel() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const r = s.prefs.rhythm ?? DEFAULT_RHYTHM;
  const on = s.layers.rhythm;
  const status = s.rhythmStatus;
  const stripControl = s.prefs.stripControl ?? "slider";
  const hasTiming = project.timing.bpm !== null && project.timing.numerator !== null && project.timing.unit !== null;
  const hasRoot = project.scale !== null || (s.prefs.drone?.rootMidi ?? null) !== null;
  const canPlay = hasTiming && hasRoot;
  const [openPart, setOpenPart] = useState<"drums" | "bass" | "ostinato" | "arp" | null>("drums");
  const set = (patch: Partial<RhythmPrefs>) => s.setPref("rhythm", { ...r, ...patch });
  const setPart = <K extends "drums" | "bass" | "ostinato" | "arp">(part: K, patch: Partial<RhythmPrefs[K]>) => set({ [part]: { ...r[part], ...patch } } as Partial<RhythmPrefs>);
  const reroll = (part: "drums" | "bass" | "ostinato" | "arp") => setPart(part, { seed: 1 + Math.floor(Math.random() * 1_000_000), ...(part === "ostinato" ? { motif: null } : {}) } as never);
  const regenerate = async () => {
    try {
      const next = await api.rhythmRegenerate({ ...r, chord: null, root_midi: null, intonation: s.prefs.drone?.intonation ?? "profile" });
      set({ drums: next.drums, bass: next.bass, ostinato: next.ostinato, arp: next.arp });
    } catch (e) {
      s.setError(describeError(e));
    }
  };
  const noteName = (midi: number) => {
    const np = noteParts(midi);
    return `${t.noteNames[np.pc]}${np.octave}`;
  };
  const meter = (i: number) => {
    const db = status ? linearToDb(status.peaks[i]) : -60;
    return (
      <div className="meter" style={{ width: 70 }} aria-hidden="true">
        <div className={`meter-fill ${db > -3 ? "hot" : db > -12 ? "warm" : ""}`} style={{ width: `${meterFraction(db) * 100}%` }} />
      </div>
    );
  };
  const partHead = (part: "drums" | "bass" | "ostinato" | "arp", title: string, index: number) => {
    const p = r[part];
    return (
      <div className="analysis-row part-head">
        <label className="inline">
          <input type="checkbox" checked={p.on} onChange={(e) => setPart(part, { on: e.target.checked } as never)} />
          <strong>{title}</strong>
        </label>
        <button className="ghost small" onClick={() => setOpenPart(openPart === part ? null : part)} aria-expanded={openPart === part}>
          {openPart === part ? "▾" : "▸"}
        </button>
        {stripControl === "knob" ? (
          <Knob value={p.level_db} min={-40} max={6} step={1} reset={-12} label={`${t.levelLabel}: ${title}`} text={p.level_db <= -39.5 ? "off" : formatDb(p.level_db)} size={40} onChange={(v) => setPart(part, { level_db: v } as never)} />
        ) : (
          <label className="inline small" title={t.levelLabel}>
            🔊
            <input type="range" min={-40} max={6} step={1} value={p.level_db} onChange={(e) => setPart(part, { level_db: Number(e.target.value) } as never)} onDoubleClick={() => setPart(part, { level_db: -12 } as never)} style={{ width: 90 }} />
            <span className="mono-num">{p.level_db <= -39.5 ? "off" : formatDb(p.level_db)}</span>
          </label>
        )}
        {meter(index)}
        <button className={`ghost small ${p.locked ? "on" : ""}`} title={t.lockPart} aria-pressed={p.locked} onClick={() => setPart(part, { locked: !p.locked } as never)}>
          {p.locked ? "🔒" : "🔓"}
        </button>
        <button className="ghost small" title={t.rerollSeed} onClick={() => reroll(part)} disabled={p.locked}>
          🎲 {p.seed}
        </button>
      </div>
    );
  };

  return (
    <section className="panel rhythm">
      <div className="analysis-row">
        <h3>{t.rhythmTitle}</h3>
        <PresetLibrary kind="rhythm" compact getPayload={() => useStore.getState().prefs.rhythm} apply={(p) => s.setPref("rhythm", { ...useStore.getState().prefs.rhythm, ...(p as object) })} />
        {on ? (
          <button className="primary" onClick={() => s.setLayer("rhythm", false)}>
            ■ {t.rackStop}
          </button>
        ) : (
          <button className="primary" onClick={() => s.setLayer("rhythm", true)} disabled={!canPlay} title={canPlay ? "" : !hasTiming ? t.rhythmNeedsTiming : t.bgNeedsRoot}>
            ▶ {t.rhythmPlay}
          </button>
        )}
        <button className="ghost" disabled={!on} onClick={() => void api.rhythmRestart().then((st) => useStore.getState().setRhythmStatus(st)).catch((e) => s.setError(describeError(e)))}>
          ⏮ {t.toStart}
        </button>
        <button className="ghost" onClick={() => void regenerate()} title={t.regenerateHint}>
          ♻ {t.regenerate}
        </button>
        <button
          className="ghost"
          disabled={!canPlay || s.pending !== null || s.mode !== "idle"}
          title={t.rhythmToTakesHint}
          onClick={() =>
            void (async () => {
              try {
                s.setPending(t.saving);
                const res = await api.rhythmToTakes({ ...r, chord: s.padChord, root_midi: s.prefs.drone?.rootMidi ?? null, intonation: s.prefs.drone?.intonation ?? "profile" });
                s.setProject(res.project);
                s.setMessage(`${t.rhythmTakesCreated}: ${res.created.length}`);
              } catch (e) {
                s.setError(describeError(e));
              } finally {
                useStore.getState().setPending(null);
              }
            })()
          }
        >
          ♩ {t.rhythmToTakes}
        </button>
        <label className="inline small">
          {t.viewLabel}
          <select value={stripControl} onChange={(e) => s.setPref("stripControl", e.target.value as "slider" | "knob")}>
            <option value="slider">{t.viewSliders}</option>
            <option value="knob">{t.viewKnobs}</option>
          </select>
        </label>
        <label className="inline small">
          {t.barsLabel}
          <select value={r.bars} onChange={(e) => set({ bars: Number(e.target.value) })}>
            {[1, 2, 4, 8, 16].map((b) => (
              <option key={b} value={b}>
                {b}
              </option>
            ))}
          </select>
        </label>
        {status?.playing && (
          <span className="mono-num small">
            {t.barShort} {status.bar} · {status.bpm.toFixed(0)} BPM{status.root_pc !== null ? ` · ${t.noteNames[status.root_pc]}` : ""}
          </span>
        )}
        <span className={`badge warn ${status && status.limiter_gain < 0.98 ? "" : "invisible"}`} aria-hidden={!(status && status.limiter_gain < 0.98)}>
          {t.limiterActive}
        </span>
      </div>
      {!hasTiming && <div className="muted small">{t.rhythmNeedsTiming}</div>}
      <div className="muted small">{t.rhythmSectionHint}</div>

      {partHead("drums", t.drumsTitle, 0)}
      {openPart === "drums" && (
        <>
          <div className="analysis-row">
            <label className="inline small">
              {t.styleLabel}
              <select value={r.drums.style} onChange={(e) => setPart("drums", { style: e.target.value as GrooveStyle })}>
                {STYLES.map((st) => (
                  <option key={st} value={st}>
                    {t[STYLE_LABEL[st]]}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.fillEvery}
              <select value={r.drums.fill_every_bars} onChange={(e) => setPart("drums", { fill_every_bars: Number(e.target.value) })}>
                {[0, 2, 4, 8].map((n) => (
                  <option key={n} value={n}>
                    {n === 0 ? t.timerOff : n}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <Sliders
            specs={[
              { key: "complexity", label: "complexityLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "humanize", label: "humanizeLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "swing", label: "swingLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "kick_hz", label: "kickTune", min: 35, max: 90, step: 1, fmt: (v) => `${v} Hz` },
              { key: "decay", label: "decayLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "brightness", label: "brightnessLabel", min: 0, max: 1, step: 0.01, fmt: pct },
            ]}
            value={r.drums}
            onChange={(p) => setPart("drums", p)}
            disabled={false}
          />
        </>
      )}

      {partHead("bass", t.bassTitle, 1)}
      {openPart === "bass" && (
        <>
          <div className="analysis-row">
            <label className="inline small">
              {t.voiceLabel}
              <select value={r.bass.voice} onChange={(e) => setPart("bass", { voice: Number(e.target.value) })}>
                {BASS_VOICES.map((k, i) => (
                  <option key={k} value={i}>
                    {t[k]}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.wobbleLabel}
              <select value={r.bass.wobble_div} onChange={(e) => setPart("bass", { wobble_div: Number(e.target.value) })}>
                <option value={0}>{t.timerOff}</option>
                <option value={4}>1/4</option>
                <option value={8}>1/8</option>
                <option value={16}>1/16</option>
              </select>
            </label>
            <label className="inline small">
              {t.registerLow}
              <select value={r.bass.register_low} onChange={(e) => setPart("bass", { register_low: Number(e.target.value), register_high: Math.max(Number(e.target.value) + 5, r.bass.register_high) })}>
                {REGISTER.filter((m) => m <= 60).map((m) => (
                  <option key={m} value={m}>
                    {noteName(m)}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.registerHigh}
              <select value={r.bass.register_high} onChange={(e) => setPart("bass", { register_high: Number(e.target.value), register_low: Math.min(Number(e.target.value) - 5, r.bass.register_low) })}>
                {REGISTER.filter((m) => m <= 72).map((m) => (
                  <option key={m} value={m}>
                    {noteName(m)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <Sliders
            specs={[
              { key: "density", label: "densityShort", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "syncopation", label: "syncopationLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "passing", label: "passingLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "cutoff_hz", label: "cutoffLabel", min: 60, max: 4000, step: 10, fmt: (v) => `${v} Hz` },
              { key: "resonance", label: "resonanceLabel", min: 0, max: 0.95, step: 0.01, fmt: pct },
              { key: "drive", label: "driveLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "glide_s", label: "glideLabel", min: 0, max: 0.5, step: 0.01, fmt: (v) => `${Math.round(v * 1000)} ms` },
              { key: "sidechain", label: "sidechainLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "wobble_depth", label: "wobbleDepth", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "warmth", label: "droneWarmth", min: 0, max: 1, step: 0.01, fmt: pct },
            ]}
            value={r.bass}
            onChange={(p) => setPart("bass", p)}
            disabled={false}
          />
        </>
      )}

      {partHead("ostinato", t.layerOstinato, 2)}
      {openPart === "ostinato" && (
        <>
          <div className="analysis-row">
            <label className="inline small">
              {t.stepsLabel}
              <select value={r.ostinato.steps} onChange={(e) => setPart("ostinato", { steps: Number(e.target.value) })}>
                {[4, 6, 7, 8, 12, 16].map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.stepLength}
              <select value={r.ostinato.step_beats} onChange={(e) => setPart("ostinato", { step_beats: Number(e.target.value) })}>
                <option value={1}>1/4</option>
                <option value={0.5}>1/8</option>
                <option value={0.25}>1/16</option>
              </select>
            </label>
            <label className="inline small">
              {t.variationEvery}
              <select value={r.ostinato.variation_every_bars} onChange={(e) => setPart("ostinato", { variation_every_bars: Number(e.target.value) })}>
                {[0, 1, 2, 4, 8].map((n) => (
                  <option key={n} value={n}>
                    {n === 0 ? t.timerOff : n}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.voiceLabel}
              <select value={r.ostinato.timbre} onChange={(e) => setPart("ostinato", { timbre: Number(e.target.value) })}>
                {TIMBRES.map((k, i) => (
                  <option key={k} value={i}>
                    {t[k]}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.registerLow}
              <select value={r.ostinato.register_low} onChange={(e) => setPart("ostinato", { register_low: Number(e.target.value), register_high: Math.max(Number(e.target.value) + 5, r.ostinato.register_high) })}>
                {REGISTER.map((m) => (
                  <option key={m} value={m}>
                    {noteName(m)}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.registerHigh}
              <select value={r.ostinato.register_high} onChange={(e) => setPart("ostinato", { register_high: Number(e.target.value), register_low: Math.min(Number(e.target.value) - 5, r.ostinato.register_low) })}>
                {REGISTER.map((m) => (
                  <option key={m} value={m}>
                    {noteName(m)}
                  </option>
                ))}
              </select>
            </label>
            <button className={`ghost small ${r.ostinato.motif ? "on" : ""}`} title={t.lockMotifHint} disabled={!status?.motif && !r.ostinato.motif} onClick={() => setPart("ostinato", { motif: r.ostinato.motif ? null : (status?.motif ?? null) })}>
              {r.ostinato.motif ? `🔒 ${t.motifLocked}` : `🔓 ${t.lockMotif}`}
            </button>
          </div>
          <Sliders
            specs={[
              { key: "density", label: "densityShort", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "probability", label: "probabilityLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "swing", label: "swingLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "release_s", label: "releaseLabel", min: 0.02, max: 2, step: 0.01, fmt: (v) => `${v.toFixed(2)} s` },
            ]}
            value={r.ostinato}
            onChange={(p) => setPart("ostinato", p)}
            disabled={false}
          />
        </>
      )}

      {partHead("arp", t.arpTitle, 3)}
      {openPart === "arp" && (
        <>
          <div className="analysis-row">
            <label className="inline small">
              {t.directionLabel}
              <select value={r.arp.direction} onChange={(e) => setPart("arp", { direction: e.target.value as ArpDirection })}>
                {DIRECTIONS.map((d) => (
                  <option key={d} value={d}>
                    {t[DIR_LABEL[d]]}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.octavesLabel}
              <select value={r.arp.octaves} onChange={(e) => setPart("arp", { octaves: Number(e.target.value) })}>
                {[1, 2, 3].map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.stepLength}
              <select value={r.arp.step_beats} onChange={(e) => setPart("arp", { step_beats: Number(e.target.value) })}>
                <option value={1}>1/4</option>
                <option value={0.5}>1/8</option>
                <option value={0.25}>1/16</option>
              </select>
            </label>
            <label className="inline small">
              {t.voiceLabel}
              <select value={r.arp.timbre} onChange={(e) => setPart("arp", { timbre: Number(e.target.value) })}>
                {TIMBRES.map((k, i) => (
                  <option key={k} value={i}>
                    {t[k]}
                  </option>
                ))}
              </select>
            </label>
            <label className="inline small">
              {t.registerLow}
              <select value={r.arp.register_low} onChange={(e) => setPart("arp", { register_low: Number(e.target.value) })}>
                {REGISTER.map((m) => (
                  <option key={m} value={m}>
                    {noteName(m)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <Sliders
            specs={[
              { key: "gate", label: "gateLabel", min: 0.05, max: 1, step: 0.01, fmt: pct },
              { key: "density", label: "densityShort", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "swing", label: "swingLabel", min: 0, max: 1, step: 0.01, fmt: pct },
              { key: "release_s", label: "releaseLabel", min: 0.02, max: 2, step: 0.01, fmt: (v) => `${v.toFixed(2)} s` },
            ]}
            value={r.arp}
            onChange={(p) => setPart("arp", p)}
            disabled={false}
          />
        </>
      )}
    </section>
  );
}
