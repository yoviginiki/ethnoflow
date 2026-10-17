import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, describeError, type Ambient, type AmbientStatus, type AutomationLane, type AutomationMode, type LayerSource, type Scene, type Snapshot, type Timing } from "./api";
import { newId } from "./ambientPresets";
import { formatBytes, formatTime } from "./format";
import { messages, type StringMessageKey } from "./i18n";
import { useStore } from "./store";

const STRIP_PARAMS = ["gain_db", "pan", "lowpass_hz", "highpass_hz"];
const MACRO_PARAMS = ["intensity", "brightness", "motion", "space", "space_size", "tonal_gravity", "width"];
function sourceParams(src: LayerSource): string[] {
  switch (src.kind) {
    case "noise":
      return ["color"];
    case "wind":
      return ["gust", "speed", "center_hz", "resonance"];
    case "texture":
      return ["brightness", "motion", "resonance"];
    case "bells":
      return ["density", "decay_s", "brightness", "velocity_spread"];
    case "pluck":
      return ["density", "damping", "brightness", "velocity_spread"];
    case "loop":
      return ["rate", "crossfade"];
    case "cloud":
      return ["grain_ms", "density", "position", "spray", "scan", "pitch", "jitter", "reverse_p", "octave_p"];
  }
}
const PARAM_LABEL: Record<string, StringMessageKey> = {
  gain_db: "levelLabel",
  grain_ms: "grainLabel",
  position: "positionLabel",
  spray: "sprayLabel",
  scan: "scanLabel",
  pitch: "pitchLabel",
  jitter: "jitterLabel",
  reverse_p: "reverseProbLabel",
  octave_p: "octaveProbLabel",
  pan: "panLabel",
  lowpass_hz: "lowpassLabel",
  highpass_hz: "highpassLabel",
  color: "colorLabel",
  gust: "gustLabel",
  speed: "speedLabel",
  center_hz: "centerLabel",
  resonance: "resonanceLabel",
  brightness: "brightnessLabel",
  motion: "motionLabel",
  density: "densityLabel",
  decay_s: "decayLabel",
  velocity_spread: "spreadLabel",
  damping: "dampingLabel",
  rate: "rateLabel",
  crossfade: "crossfadeLabel",
  intensity: "macroIntensity",
  space: "macroSpace",
  space_size: "macroSpaceSize",
  tonal_gravity: "macroGravity",
  width: "macroWidth",
};
const MODES: AutomationMode["mode"][] = ["static", "lfo", "walk", "keyframes"];
const MODE_LABEL: Record<AutomationMode["mode"], StringMessageKey> = { static: "modeStatic", lfo: "modeLfo", walk: "modeWalk", keyframes: "modeKeyframes" };

function defaultMode(mode: AutomationMode["mode"]): AutomationMode {
  switch (mode) {
    case "static":
      return { mode };
    case "lfo":
      return { mode, period_s: 30, phase: 0, shape: "sine" };
    case "walk":
      return { mode, period_s: 12 };
    case "keyframes":
      return { mode, points: [] };
  }
}

interface LaneProps {
  rack: Ambient;
  /** Layer id, or null for the macros. */
  layerId: string | null;
  source: LayerSource | null;
  laneTime: number;
  onChange: (next: Ambient) => void;
}

/** Automation lanes (F1A.4) of one layer or of the macros. */
export function LaneEditor({ rack, layerId, source, laneTime, onChange }: LaneProps) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const params = layerId === null ? MACRO_PARAMS : [...STRIP_PARAMS, ...(source ? sourceParams(source) : [])];
  const lanes = rack.automation.filter((l) => (l.target.layer ?? null) === layerId);
  const [newParam, setNewParam] = useState(params[0] ?? "");
  const patchLane = (id: string, patch: Partial<AutomationLane>) => onChange({ ...rack, automation: rack.automation.map((l) => (l.id === id ? { ...l, ...patch } : l)) });
  const addLane = () => {
    if (!newParam) return;
    const lane: AutomationLane = { id: newId(), target: { layer: layerId, param: newParam }, mode: defaultMode("walk"), depth: 0.4, humanize: 0, seed: 1 + Math.floor(Math.random() * 1_000_000), enabled: true };
    onChange({ ...rack, automation: [...rack.automation, lane] });
  };
  const removeLane = (id: string) => onChange({ ...rack, automation: rack.automation.filter((l) => l.id !== id) });
  const label = (p: string) => (PARAM_LABEL[p] ? t[PARAM_LABEL[p]] : p);

  return (
    <div className="lanes">
      <div className="analysis-row">
        <strong className="small">{t.automationTitle}</strong>
        <select value={newParam} onChange={(e) => setNewParam(e.target.value)} aria-label={t.automationParam}>
          {params.map((p) => (
            <option key={p} value={p}>
              {label(p)}
            </option>
          ))}
        </select>
        <button className="ghost small" onClick={addLane} disabled={!newParam}>
          + {t.automationAdd}
        </button>
      </div>
      {lanes.length === 0 && <div className="muted small">{t.automationHint}</div>}
      {lanes.map((l) => (
        <div key={l.id} className={`lane ${l.enabled ? "" : "dim"}`}>
          <div className="analysis-row">
            <label className="inline">
              <input type="checkbox" checked={l.enabled} onChange={(e) => patchLane(l.id, { enabled: e.target.checked })} />
              <strong>{label(l.target.param)}</strong>
            </label>
            <select value={l.mode.mode} onChange={(e) => patchLane(l.id, { mode: defaultMode(e.target.value as AutomationMode["mode"]) })}>
              {MODES.map((m) => (
                <option key={m} value={m}>
                  {t[MODE_LABEL[m]]}
                </option>
              ))}
            </select>
            <button className="ghost small" title={t.deleteClip} onClick={() => removeLane(l.id)}>
              ✕
            </button>
          </div>
          {(l.mode.mode === "lfo" || l.mode.mode === "walk") && (
            <div className="drone-sliders one-col">
              <label className="drone-slider">
                <span>{t.periodLabel}</span>
                <input type="range" min={0} max={1} step={0.005} value={Math.log(l.mode.period_s / 0.5) / Math.log(1200)} onChange={(e) => patchLane(l.id, { mode: { ...l.mode, period_s: Math.round(0.5 * Math.pow(1200, Number(e.target.value)) * 10) / 10 } as AutomationMode })} />
                <span className="mono-num small">{l.mode.period_s < 60 ? `${l.mode.period_s.toFixed(1)} s` : `${(l.mode.period_s / 60).toFixed(1)} min`}</span>
              </label>
              <label className="drone-slider">
                <span>{t.depthLabel}</span>
                <input type="range" min={0} max={1} step={0.01} value={l.depth} onChange={(e) => patchLane(l.id, { depth: Number(e.target.value) })} />
                <span className="mono-num small">{Math.round(l.depth * 100)}%</span>
              </label>
              {l.mode.mode === "lfo" && (
                <>
                  <label className="drone-slider">
                    <span>{t.phaseLabel}</span>
                    <input type="range" min={0} max={1} step={0.01} value={l.mode.phase} onChange={(e) => patchLane(l.id, { mode: { ...l.mode, phase: Number(e.target.value) } as AutomationMode })} />
                    <span className="mono-num small">{Math.round(l.mode.phase * 360)}°</span>
                  </label>
                  <label className="inline small">
                    {t.shapeLabel}
                    <select value={l.mode.shape} onChange={(e) => patchLane(l.id, { mode: { ...l.mode, shape: e.target.value as "sine" | "triangle" } as AutomationMode })}>
                      <option value="sine">{t.shapeSine}</option>
                      <option value="triangle">{t.shapeTriangle}</option>
                    </select>
                  </label>
                </>
              )}
            </div>
          )}
          {l.mode.mode === "keyframes" && (
            <div className="keyframes">
              <ul>
                {l.mode.points.map(([pt, pv], i) => (
                  <li key={i} className="analysis-row small">
                    <input type="number" step={0.1} min={0} value={pt} aria-label={t.timeLabel} onChange={(e) => patchLane(l.id, { mode: { ...l.mode, points: (l.mode as { points: [number, number][] }).points.map((p, k) => (k === i ? [Math.max(0, Number(e.target.value)), p[1]] : p)).sort((a, b) => a[0] - b[0]) } as AutomationMode })} />
                    s →
                    <input type="number" step={0.01} value={pv} aria-label={t.valueLabel} onChange={(e) => patchLane(l.id, { mode: { ...l.mode, points: (l.mode as { points: [number, number][] }).points.map((p, k) => (k === i ? [p[0], Number(e.target.value)] : p)) } as AutomationMode })} />
                    <button className="ghost small" onClick={() => patchLane(l.id, { mode: { ...l.mode, points: (l.mode as { points: [number, number][] }).points.filter((_, k) => k !== i) } as AutomationMode })}>
                      ✕
                    </button>
                  </li>
                ))}
              </ul>
              <button
                className="ghost small"
                onClick={() => {
                  const pts = (l.mode as { points: [number, number][] }).points;
                  const last = pts[pts.length - 1];
                  const at = Math.max(Math.round(laneTime * 10) / 10, last ? last[0] + 1 : 0);
                  patchLane(l.id, { mode: { ...l.mode, points: [...pts, [at, last ? last[1] : 0.5]].sort((a, b) => a[0] - b[0]) } as AutomationMode });
                }}
              >
                + {t.keyframeAdd}
              </button>
              <span className="muted small">{t.keyframeHint}</span>
            </div>
          )}
          <div className="drone-sliders one-col">
            <label className="drone-slider">
              <span>{t.humanizeLabel}</span>
              <input type="range" min={0} max={1} step={0.01} value={l.humanize} onChange={(e) => patchLane(l.id, { humanize: Number(e.target.value) })} />
              <span className="mono-num small">{Math.round(l.humanize * 100)}%</span>
            </label>
            <label className="drone-slider">
              <span>{t.seedLabel}</span>
              <input type="number" min={1} step={1} value={l.seed} onChange={(e) => patchLane(l.id, { seed: Math.max(1, Math.floor(Number(e.target.value) || 1)) })} />
              <button className="ghost small" title={t.rerollSeed} onClick={() => patchLane(l.id, { seed: 1 + Math.floor(Math.random() * 1_000_000) })}>
                🎲
              </button>
            </label>
          </div>
        </div>
      ))}
    </div>
  );
}

export function snapshotOf(rack: Ambient): Snapshot {
  return {
    layers: rack.layers.map((l) => ({ id: l.id, gain_db: l.gain_db, pan: l.pan, lowpass_hz: l.lowpass_hz, highpass_hz: l.highpass_hz, mute: l.mute, source: l.source })),
    macros: rack.macros,
  };
}

interface ProgramProps {
  rack: Ambient;
  status: AmbientStatus | null;
  timing: Timing;
  projectName: string;
  onChange: (next: Ambient) => void;
}

/** Scenes on a timeline, program transport, loop and offline bounce (F1A.5). */
export function ProgramPanel({ rack, status, timing, projectName, onChange }: ProgramProps) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const scenes = rack.program.scenes;
  const end = scenes.reduce((m, sc) => Math.max(m, sc.start_s + sc.duration_s), 0);
  const beatSeconds = timing.bpm && timing.unit ? (60 / timing.bpm) * (4 / timing.unit) : null;
  const barSeconds = beatSeconds && timing.numerator ? beatSeconds * timing.numerator : null;
  const [inBars, setInBars] = useState(!!barSeconds);
  const [stems, setStems] = useState(false);
  const [exportSeconds, setExportSeconds] = useState(300);
  const [collapsed, setCollapsed] = useState(false);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));
  const pos = status?.program_position ?? 0;
  const running = status?.program_playing ?? false;
  const useBars = inBars && barSeconds !== null;
  const fmtDur = (sec: number) => (useBars && barSeconds ? `${(sec / barSeconds).toFixed(1)} ${t.barsShort}` : formatTime(sec));

  const setProgram = (patch: Partial<Ambient["program"]>) => onChange({ ...rack, program: { ...rack.program, ...patch } });
  /** Scenes stay contiguous: each starts where the previous ends. */
  const relayout = (list: Scene[]): Scene[] => {
    let at = 0;
    return list.map((sc) => {
      const next = { ...sc, start_s: at };
      at += sc.duration_s;
      return next;
    });
  };
  const addScene = () => {
    const duration = barSeconds ? barSeconds * 8 : 60;
    const sc: Scene = { id: newId(), name: `${t.sceneDefault} ${scenes.length + 1}`, start_s: end, duration_s: duration, transition_s: scenes.length ? Math.min(10, duration / 3) : 0, snapshot: snapshotOf(rack) };
    setProgram({ scenes: relayout([...scenes, sc]) });
  };
  const patchScene = (id: string, patch: Partial<Scene>) => setProgram({ scenes: relayout(scenes.map((sc) => (sc.id === id ? { ...sc, ...patch } : sc))) });
  const removeScene = (id: string) => {
    const next = relayout(scenes.filter((sc) => sc.id !== id));
    setProgram({ scenes: next, loop_region: null });
  };
  const moveScene = (id: string, dir: -1 | 1) => {
    const i = scenes.findIndex((sc) => sc.id === id);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= scenes.length) return;
    const next = [...scenes];
    [next[i], next[j]] = [next[j], next[i]];
    setProgram({ scenes: relayout(next) });
  };
  const transport = (playing: boolean | null, position: number | null) => api.ambientProgramSet(playing, position).then((st) => useStore.getState().setAmbientStatus(st)).catch(fail);
  const exportProgram = async () => {
    try {
      const path = await save({ title: t.exportProgram, defaultPath: `${projectName}-atmosphere.wav`, filters: [{ name: "WAV", extensions: ["wav"] }] });
      if (!path) return;
      s.setPending(t.exporting);
      const r = await api.exportAmbient(path, s.prefs.exportFormat, end > 0 ? null : exportSeconds, stems, null);
      s.setMessage(`${t.programExported}: ${r.path} (${formatBytes(r.bytes)}, ${formatTime(r.seconds)} ${t.renderedIn} ${(r.render_ms / 1000).toFixed(1)} s${r.stems.length ? `, ${r.stems.length} stems` : ""})`);
    } catch (e) {
      fail(e);
    } finally {
      useStore.getState().setPending(null);
    }
  };
  const loopChoice = rack.program.loop_region === null ? "" : rack.program.loop_region[0] === 0 && Math.abs(rack.program.loop_region[1] - end) < 1e-6 ? "all" : (scenes.find((sc) => Math.abs(sc.start_s - rack.program.loop_region![0]) < 1e-6 && Math.abs(sc.start_s + sc.duration_s - rack.program.loop_region![1]) < 1e-6)?.id ?? "custom");

  return (
    <section className="panel program">
      <div className="analysis-row">
        <button className="ghost small" title={collapsed ? t.restorePanel : t.minimizePanel} aria-expanded={!collapsed} onClick={() => setCollapsed((v) => !v)}>
          {collapsed ? "▸" : "▾"}
        </button>
        <h3>{t.programTitle}</h3>
        <button className="ghost" disabled={!s.layers.ambient || scenes.length === 0} onClick={() => void transport(!running, null)} title={s.layers.ambient ? "" : t.programNeedsRack}>
          {running ? "❚❚" : "▶"} {t.programPlay}
        </button>
        <button className="ghost" disabled={!s.layers.ambient || scenes.length === 0} onClick={() => void transport(false, 0)}>
          ⏮ {t.toStart}
        </button>
        <input type="range" min={0} max={Math.max(end, 1)} step={0.1} value={Math.min(pos, end)} disabled={!s.layers.ambient || scenes.length === 0} aria-label={t.programPosition} onChange={(e) => void transport(null, Number(e.target.value))} style={{ width: 220 }} />
        <span className="mono-num small">
          {formatTime(pos)} / {formatTime(end)}
        </span>
        <label className="inline small">
          {t.loopRegion}
          <select
            value={loopChoice}
            disabled={scenes.length === 0}
            onChange={(e) => {
              const v = e.target.value;
              if (v === "") setProgram({ loop_region: null });
              else if (v === "all") setProgram({ loop_region: [0, end] });
              else {
                const sc = scenes.find((x) => x.id === v);
                if (sc) setProgram({ loop_region: [sc.start_s, sc.start_s + sc.duration_s] });
              }
            }}
          >
            <option value="">{t.timerOff}</option>
            <option value="all">{t.loopAll}</option>
            {scenes.map((sc) => (
              <option key={sc.id} value={sc.id}>
                {sc.name}
              </option>
            ))}
            {loopChoice === "custom" && <option value="custom">…</option>}
          </select>
        </label>
        {barSeconds && (
          <label className="inline small">
            <input type="checkbox" checked={inBars} onChange={(e) => setInBars(e.target.checked)} />
            {t.beatSync}
          </label>
        )}
      </div>
      {!collapsed && (
        <>
      <div className="scenes">
        {scenes.map((sc, i) => {
          const active = pos >= sc.start_s && pos < sc.start_s + sc.duration_s && scenes.length > 0;
          return (
            <div key={sc.id} className={`scene ${active ? "active" : ""}`} style={{ flexGrow: Math.max(1, sc.duration_s) }}>
              <input className="scene-name" value={sc.name} onChange={(e) => patchScene(sc.id, { name: e.target.value })} aria-label={t.sceneName} />
              <div className="muted small mono-num">
                {formatTime(sc.start_s)} · {fmtDur(sc.duration_s)}
              </div>
              <label className="inline small">
                {t.durationLabel}
                <input type="number" min={useBars ? 0.5 : 1} step={useBars ? 0.5 : 1} value={useBars && barSeconds ? Math.round((sc.duration_s / barSeconds) * 2) / 2 : Math.round(sc.duration_s)} onChange={(e) => patchScene(sc.id, { duration_s: Math.max(1, Number(e.target.value) * (useBars && barSeconds ? barSeconds : 1)) })} />
                {useBars ? t.barsShort : "s"}
              </label>
              <label className="inline small">
                {t.transitionLabel}
                <input type="number" min={0} step={1} value={Math.round(sc.transition_s)} onChange={(e) => patchScene(sc.id, { transition_s: Math.max(0, Number(e.target.value)) })} />
                s
              </label>
              <div className="track-ctl">
                <button className="ghost small" title={t.sceneGo} disabled={!s.layers.ambient} onClick={() => void transport(null, sc.start_s)}>
                  ⤓
                </button>
                <button className="ghost small" title={t.sceneCapture} onClick={() => patchScene(sc.id, { snapshot: snapshotOf(rack) })}>
                  📷
                </button>
                <button className="ghost small" title={t.moveLeft} disabled={i === 0} onClick={() => moveScene(sc.id, -1)}>
                  ◀
                </button>
                <button className="ghost small" title={t.moveRight} disabled={i === scenes.length - 1} onClick={() => moveScene(sc.id, 1)}>
                  ▶
                </button>
                <button className="ghost small" title={t.sceneDelete} onClick={() => removeScene(sc.id)}>
                  ✕
                </button>
              </div>
            </div>
          );
        })}
        <button className="scene add" onClick={addScene} disabled={rack.layers.length === 0} title={t.sceneAddHint}>
          + {t.sceneAdd}
        </button>
      </div>
      <div className="muted small">{t.programHint}</div>
      <div className="analysis-row">
        <button className="ghost" disabled={rack.layers.length === 0 || s.pending !== null} onClick={() => void exportProgram()}>
          {t.exportProgram}
        </button>
        {end === 0 && (
          <label className="inline small">
            {t.durationLabel}
            <input type="number" min={5} max={14400} step={5} value={exportSeconds} onChange={(e) => setExportSeconds(Math.max(5, Number(e.target.value)))} />
            s
          </label>
        )}
        <label className="inline small">
          <input type="checkbox" checked={stems} onChange={(e) => setStems(e.target.checked)} />
          {t.exportStems}
        </label>
        <span className="muted small">{t.exportProgramHint}</span>
      </div>
        </>
      )}
    </section>
  );
}
