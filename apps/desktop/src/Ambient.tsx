import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, describeError, type Ambient as AmbientRack, type AmbientLayer, type LayerKind, type LayerSource, type Macros } from "./api";
import { DEFAULT_MACROS, KIND_LABEL, MAX_AMBIENT_LAYERS, defaultSource, makeLayer, starter, type StarterKey } from "./ambientPresets";
import { BackgroundTransport } from "./BackgroundTransport";
import { ConfirmDialog } from "./ConfirmDialog";
import { formatDb, linearToDb, meterFraction, noteParts, takeName } from "./format";
import { fadeOutBackground } from "./layers";
import { messages, type StringMessageKey } from "./i18n";
import { useStore } from "./store";

const KINDS: LayerKind[] = ["noise", "wind", "texture", "bells", "pluck"];
const STARTERS: StarterKey[] = ["starterMountain", "starterNoKey", "starterNight"];
const POLL_MS = 150;
const COMMIT_MS = 80;
const REGISTER = Array.from({ length: 73 }, (_, i) => 36 + i); // C2 … C8

interface SliderSpec {
  key: string;
  label: StringMessageKey;
  min: number;
  max: number;
  step: number;
  /** How the value reads next to the slider. */
  fmt: (v: number) => string;
}
const pct = (v: number) => `${Math.round(v * 100)}%`;
const SOURCE_SLIDERS: Record<LayerKind, SliderSpec[]> = {
  noise: [{ key: "color", label: "colorLabel", min: 0, max: 1, step: 0.01, fmt: pct }],
  wind: [
    { key: "gust", label: "gustLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "speed", label: "speedLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "center_hz", label: "centerLabel", min: 80, max: 6000, step: 10, fmt: (v) => `${Math.round(v)} Hz` },
    { key: "resonance", label: "resonanceLabel", min: 0, max: 1, step: 0.01, fmt: pct },
  ],
  texture: [
    { key: "brightness", label: "brightnessLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "motion", label: "motionLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "resonance", label: "resonanceLabel", min: 0, max: 1, step: 0.01, fmt: pct },
  ],
  bells: [
    { key: "density", label: "densityLabel", min: 0, max: 60, step: 0.5, fmt: (v) => v.toFixed(1) },
    { key: "decay_s", label: "decayLabel", min: 0.2, max: 30, step: 0.1, fmt: (v) => `${v.toFixed(1)} s` },
    { key: "brightness", label: "brightnessLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "velocity_spread", label: "spreadLabel", min: 0, max: 1, step: 0.01, fmt: pct },
  ],
  pluck: [
    { key: "density", label: "densityLabel", min: 0, max: 120, step: 0.5, fmt: (v) => v.toFixed(1) },
    { key: "damping", label: "dampingLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "brightness", label: "brightnessLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "velocity_spread", label: "spreadLabel", min: 0, max: 1, step: 0.01, fmt: pct },
  ],
  loop: [
    { key: "start", label: "loopStart", min: 0, max: 600, step: 0.01, fmt: (v) => `${v.toFixed(2)} s` },
    { key: "end", label: "loopEnd", min: 0, max: 600, step: 0.01, fmt: (v) => (v === 0 ? "—" : `${v.toFixed(2)} s`) },
    { key: "crossfade", label: "crossfadeLabel", min: 0, max: 5, step: 0.01, fmt: (v) => `${v.toFixed(2)} s` },
    { key: "rate", label: "rateLabel", min: 0.25, max: 4, step: 0.01, fmt: (v) => `×${v.toFixed(2)}` },
  ],
};
const MACRO_SLIDERS: { key: keyof Macros; label: StringMessageKey }[] = [
  { key: "intensity", label: "macroIntensity" },
  { key: "brightness", label: "macroBrightness" },
  { key: "motion", label: "macroMotion" },
  { key: "space", label: "macroSpace" },
  { key: "space_size", label: "macroSpaceSize" },
  { key: "tonal_gravity", label: "macroGravity" },
  { key: "width", label: "macroWidth" },
];

function hasTonal(src: LayerSource): src is Extract<LayerSource, { tonal: boolean }> {
  return "tonal" in src;
}

/**
 * Atmosphere / Ambient Playground (F1A.1, F1A.2): up to eight layers of synthesized
 * sources and audio loops on the same graph as the drone, the pad and the tracks.
 * Sound starts only on Play; every slider is smoothed in the engine.
 */
export function Ambient() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const rack = s.ambient;
  const playing = s.layers.ambient;
  const status = s.ambientStatus;
  const [selected, setSelected] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<{ kind: "remove"; id: string } | { kind: "starter"; key: StarterKey } | null>(null);
  const [fadeSeconds, setFadeSeconds] = useState(15);
  const [editingName, setEditingName] = useState<string | null>(null);
  const fail = useCallback((e: unknown) => useStore.getState().setError(describeError(e)), []);
  const pendingRef = useRef<AmbientRack | null>(null);
  const timerRef = useRef<number | null>(null);

  // Load the rack if the project opened before the hook fetched it.
  useEffect(() => {
    if (rack) return;
    api
      .ambientGet()
      .then((v) => useStore.getState().setAmbient(v.ambient))
      .catch(fail);
  }, [rack, fail]);

  // Meters and pitches, also while stopped (pitches show what a layer would play).
  useEffect(() => {
    const id = window.setInterval(() => {
      api
        .ambientStatus()
        .then((st) => useStore.getState().setAmbientStatus(st))
        .catch(() => undefined);
    }, POLL_MS);
    return () => window.clearInterval(id);
  }, []);

  /** Local state now, save + apply shortly after (sliders fire continuously). */
  const commit = useCallback(
    (next: AmbientRack) => {
      useStore.getState().setAmbient(next);
      pendingRef.current = next;
      if (timerRef.current !== null) return;
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        const p = pendingRef.current;
        pendingRef.current = null;
        if (!p) return;
        api
          .setAmbient(p)
          .then((v) => {
            // Keep newer local edits if any arrived while saving.
            if (pendingRef.current === null) useStore.getState().setAmbient(v.ambient);
          })
          .catch(fail);
      }, COMMIT_MS);
    },
    [fail],
  );
  useEffect(
    () => () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        const p = pendingRef.current;
        if (p) void api.setAmbient(p).catch(() => undefined);
      }
    },
    [],
  );

  const layers = rack?.layers ?? [];
  const macros = rack?.macros ?? DEFAULT_MACROS;
  const sel = layers.find((l) => l.id === selected) ?? null;
  const selIndex = sel ? layers.indexOf(sel) : -1;
  const kindNames = useMemo(
    () => ({ noise: t.kindNoise, wind: t.kindWind, texture: t.kindTexture, bells: t.kindBells, pluck: t.kindPluck, loop: t.kindLoop }),
    [t],
  );

  const patchLayer = (id: string, patch: Partial<AmbientLayer>) => {
    if (!rack) return;
    commit({ ...rack, layers: rack.layers.map((l) => (l.id === id ? { ...l, ...patch } : l)) });
  };
  const patchSource = (id: string, patch: Record<string, unknown>) => {
    const l = layers.find((x) => x.id === id);
    if (!l) return;
    patchLayer(id, { source: { ...l.source, ...patch } as LayerSource });
  };
  const addLayer = (kind: LayerKind, takeId = "") => {
    if (!rack || layers.length >= MAX_AMBIENT_LAYERS) return;
    const layer = makeLayer(kindNames[kind], defaultSource(kind, takeId), { gain_db: kind === "noise" ? -24 : -18, pan: ((layers.length % 3) - 1) * 0.3 });
    commit({ ...rack, layers: [...rack.layers, layer] });
    setSelected(layer.id);
  };
  const removeLayer = (id: string) => {
    if (!rack) return;
    commit({ ...rack, layers: rack.layers.filter((l) => l.id !== id) });
    if (selected === id) setSelected(null);
  };
  const moveLayer = (id: string, dir: -1 | 1) => {
    if (!rack) return;
    const i = rack.layers.findIndex((l) => l.id === id);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= rack.layers.length) return;
    const next = [...rack.layers];
    [next[i], next[j]] = [next[j], next[i]];
    commit({ ...rack, layers: next });
  };
  const applyStarter = (key: StarterKey) => {
    commit(starter(key, kindNames));
    setSelected(null);
  };
  const setMacro = (key: keyof Macros, v: number) => rack && commit({ ...rack, macros: { ...rack.macros, [key]: v } });

  const masterDb = s.prefs.masterDb ?? 0;
  const masterPeakDb = status ? linearToDb(status.master_peak) : -60;
  const sleepLeft = s.sleepAt === null ? null : Math.max(0, Math.round((s.sleepAt - Date.now()) / 60000));
  const canPlay = layers.length > 0;
  const noScale = !project.scale;

  const noteName = (midi: number) => {
    const np = noteParts(midi);
    return `${t.noteNames[np.pc]}${np.octave}`;
  };
  const hzName = (hz: number) => noteName(Math.round(69 + 12 * Math.log2(hz / (project.a4_hz || 440))));

  return (
    <main className="ambient">
      <section className="panel ambient-top">
        <div className="analysis-row">
          <h2 className="arrange-title">{t.ambientTitle}</h2>
          {playing ? (
            <button className="primary" onClick={() => s.setLayer("ambient", false)}>
              ■ {t.rackStop}
            </button>
          ) : (
            <button className="primary" onClick={() => s.setLayer("ambient", true)} disabled={!canPlay} title={canPlay ? t.rackPlay : t.rackEmpty}>
              ▶ {t.rackPlay}
            </button>
          )}
          <label className="inline">
            {t.fadeOutBg}
            <select value={fadeSeconds} onChange={(e) => setFadeSeconds(Number(e.target.value))}>
              {[5, 15, 30, 60].map((n) => (
                <option key={n} value={n}>
                  {n} s
                </option>
              ))}
            </select>
            <button className="ghost" disabled={!s.layers.ambient && !s.layers.drone && !s.layers.pad} onClick={() => void fadeOutBackground(fadeSeconds)}>
              {t.fadeOutBg}
            </button>
          </label>
          <label className="inline">
            {t.sleepTimer}
            <select value={s.sleepAt === null ? 0 : "set"} onChange={(e) => s.setSleepAt(e.target.value === "0" ? null : Date.now() + Number(e.target.value) * 60000)}>
              <option value={0}>{t.timerOff}</option>
              {s.sleepAt !== null && (
                <option value="set">
                  {sleepLeft} {t.minutesShort}
                </option>
              )}
              {[5, 15, 30, 60, 90].map((n) => (
                <option key={n} value={n}>
                  {n} {t.minutesShort}
                </option>
              ))}
            </select>
          </label>
          <label className="inline" title={t.backgroundLevel}>
            🔊
            <input type="range" min={-40} max={6} step={1} value={masterDb} onChange={(e) => s.setPref("masterDb", Number(e.target.value))} style={{ width: 120 }} />
            <span className="mono-num">{formatDb(masterDb)}</span>
          </label>
          {masterDb > 0 && <span className="badge warn">{t.headphoneWarn}</span>}
          <div className="meter" style={{ width: 120 }} role="meter" aria-valuemin={-60} aria-valuemax={0} aria-valuenow={Math.round(masterPeakDb)} aria-label={t.mixTitle}>
            <div className={`meter-fill ${masterPeakDb > -3 ? "hot" : masterPeakDb > -12 ? "warm" : ""}`} style={{ width: `${meterFraction(masterPeakDb) * 100}%` }} />
          </div>
          {status && status.limiter_gain < 0.98 && <span className="badge warn">{t.limiterActive}</span>}
          {status && status.stream_errors > 0 && (
            <span className="clip">
              {status.stream_errors} {t.droneStreamErrors}
            </span>
          )}
          <span className="bg-inline">
            <BackgroundTransport compact />
          </span>
        </div>
        <div className="muted small">{t.ambientIntro}</div>
      </section>

      <div className="ambient-body">
        <aside className="panel ambient-left">
          <h3>{t.starters}</h3>
          <div className="style-row">
            {STARTERS.map((k) => (
              <button key={k} className="ghost" onClick={() => (layers.length ? setConfirm({ kind: "starter", key: k }) : applyStarter(k))}>
                {t[k]}
              </button>
            ))}
          </div>
          <div className="muted small">{t.starterHint}</div>
          <h3>{t.addLayer}</h3>
          <div className="style-row">
            {KINDS.map((k) => (
              <button key={k} className="ghost" disabled={layers.length >= MAX_AMBIENT_LAYERS} onClick={() => addLayer(k)}>
                + {t[KIND_LABEL[k]]}
              </button>
            ))}
          </div>
          <label>
            {t.loopFromTake}
            <select value="" disabled={layers.length >= MAX_AMBIENT_LAYERS || project.takes.length === 0} onChange={(e) => e.target.value && addLayer("loop", e.target.value)}>
              <option value="">…</option>
              {project.takes.map((tk) => (
                <option key={tk.id} value={tk.id}>
                  {takeName(tk, t.take)}
                </option>
              ))}
            </select>
          </label>
          {layers.length >= MAX_AMBIENT_LAYERS && <div className="muted small">{t.maxLayers}</div>}
          <div className="muted small">{t.safeStartHint}</div>
        </aside>

        <section className="panel ambient-rack" aria-label={t.ambientTitle}>
          {layers.length === 0 && <div className="muted">{t.rackEmpty}</div>}
          <div className="strips">
            {layers.map((l, i) => {
              const peakDb = status?.layer_peaks[i] !== undefined ? linearToDb(status.layer_peaks[i]) : -60;
              const anySolo = layers.some((x) => x.solo);
              const dim = l.mute || (anySolo && !l.solo);
              return (
                <div key={l.id} className={`strip ${l.id === selected ? "selected" : ""} ${dim ? "dim" : ""}`} onClick={() => setSelected(l.id)} role="button" tabIndex={0} onKeyDown={(e) => e.key === "Enter" && setSelected(l.id)}>
                  {editingName === l.id ? (
                    <input
                      autoFocus
                      className="strip-name-input"
                      defaultValue={l.name}
                      onBlur={(e) => {
                        patchLayer(l.id, { name: e.target.value.trim() || l.name });
                        setEditingName(null);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                        if (e.key === "Escape") setEditingName(null);
                      }}
                    />
                  ) : (
                    <button className="strip-name ghost" title={t.renameHint} onDoubleClick={() => setEditingName(l.id)}>
                      {l.name}
                    </button>
                  )}
                  <span className="muted small">{t[KIND_LABEL[l.source.kind]]}</span>
                  <div className="strip-fader">
                    <div className="vmeter" aria-hidden="true">
                      <div className={`vmeter-fill ${peakDb > -3 ? "hot" : peakDb > -12 ? "warm" : ""}`} style={{ height: `${meterFraction(peakDb) * 100}%` }} />
                    </div>
                    <input type="range" className="vslider" min={-60} max={12} step={0.5} value={l.gain_db} aria-label={t.levelLabel} onChange={(e) => patchLayer(l.id, { gain_db: Number(e.target.value) })} onDoubleClick={() => patchLayer(l.id, { gain_db: -18 })} />
                  </div>
                  <span className="mono-num small">{formatDb(l.gain_db)}</span>
                  <label className="strip-pan" title={t.panLabel}>
                    <input type="range" min={-1} max={1} step={0.05} value={l.pan} aria-label={t.panLabel} onChange={(e) => patchLayer(l.id, { pan: Number(e.target.value) })} onDoubleClick={() => patchLayer(l.id, { pan: 0 })} />
                  </label>
                  <div className="track-ctl">
                    <button className={l.mute ? "on" : ""} title={t.mute} onClick={(e) => (e.stopPropagation(), patchLayer(l.id, { mute: !l.mute }))}>
                      {t.muteAbbr}
                    </button>
                    <button className={l.solo ? "on" : ""} title={t.solo} onClick={(e) => (e.stopPropagation(), patchLayer(l.id, { solo: !l.solo }))}>
                      {t.soloAbbr}
                    </button>
                  </div>
                  <div className="track-ctl">
                    <button className="ghost small" title={t.moveLeft} disabled={i === 0} onClick={(e) => (e.stopPropagation(), moveLayer(l.id, -1))}>
                      ◀
                    </button>
                    <button className="ghost small" title={t.moveRight} disabled={i === layers.length - 1} onClick={(e) => (e.stopPropagation(), moveLayer(l.id, 1))}>
                      ▶
                    </button>
                    <button className="ghost small" title={t.removeLayer} onClick={(e) => (e.stopPropagation(), setConfirm({ kind: "remove", id: l.id }))}>
                      ✕
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </section>

        <aside className="panel ambient-right">
          <h3>{t.macrosTitle}</h3>
          <div className="drone-sliders one-col">
            {MACRO_SLIDERS.map(({ key, label }) => (
              <label key={key} className="drone-slider">
                <span>{t[label]}</span>
                <input type="range" min={0} max={1} step={0.01} value={macros[key]} disabled={!rack} onChange={(e) => setMacro(key, Number(e.target.value))} onDoubleClick={() => setMacro(key, DEFAULT_MACROS[key])} />
                <span className="mono-num small">{pct(macros[key])}</span>
              </label>
            ))}
          </div>
          <div className="muted small">{t.macroHint}</div>

          <h3>{sel ? `${t.layerDetail}: ${sel.name}` : t.layerDetail}</h3>
          {!sel && <div className="muted small">{t.selectLayerHint}</div>}
          {sel && (
            <div className="drone-sliders one-col">
              {SOURCE_SLIDERS[sel.source.kind].map((sp) => {
                const v = (sel.source as unknown as Record<string, number>)[sp.key];
                return (
                  <label key={sp.key} className="drone-slider">
                    <span>{t[sp.label]}</span>
                    <input type="range" min={sp.min} max={sp.max} step={sp.step} value={v} onChange={(e) => patchSource(sel.id, { [sp.key]: Number(e.target.value) })} />
                    <span className="mono-num small">{sp.fmt(v)}</span>
                  </label>
                );
              })}
              {sel.source.kind === "loop" && (
                <>
                  <label className="inline">
                    {t.kindLoop}
                    <select value={sel.source.take_id} onChange={(e) => patchSource(sel.id, { take_id: e.target.value })}>
                      {project.takes.map((tk) => (
                        <option key={tk.id} value={tk.id}>
                          {takeName(tk, t.take)}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="inline">
                    <input type="checkbox" checked={sel.source.reverse} onChange={(e) => patchSource(sel.id, { reverse: e.target.checked })} />
                    {t.reverseLabel}
                  </label>
                </>
              )}
              {hasTonal(sel.source) && (
                <>
                  <label className="inline">
                    <input type="checkbox" checked={sel.source.tonal} onChange={(e) => patchSource(sel.id, { tonal: e.target.checked })} />
                    {sel.source.tonal ? t.tonalLabel : t.noKeyLabel}
                  </label>
                  <div className="analysis-row">
                    <label className="inline">
                      {t.registerLow}
                      <select value={sel.source.register[0]} onChange={(e) => patchSource(sel.id, { register: [Number(e.target.value), Math.max(Number(e.target.value), (sel.source as { register: [number, number] }).register[1])] })}>
                        {REGISTER.map((m) => (
                          <option key={m} value={m}>
                            {noteName(m)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="inline">
                      {t.registerHigh}
                      <select value={sel.source.register[1]} onChange={(e) => patchSource(sel.id, { register: [Math.min(Number(e.target.value), (sel.source as { register: [number, number] }).register[0]), Number(e.target.value)] })}>
                        {REGISTER.map((m) => (
                          <option key={m} value={m}>
                            {noteName(m)}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                  {sel.source.tonal && noScale && <div className="muted small">{t.noScaleForTonal}</div>}
                  {status && selIndex >= 0 && status.pitches_hz[selIndex]?.length > 0 && (
                    <div className="muted small">
                      {t.pitchesLabel}: {sel.source.tonal && !noScale ? status.pitches_hz[selIndex].map(hzName).join(" ") : status.pitches_hz[selIndex].map((hz) => `${hz.toFixed(0)} Hz`).join(" ")}
                    </div>
                  )}
                </>
              )}
              {sel.source.kind !== "loop" && (
                <label className="drone-slider">
                  <span>{t.seedLabel}</span>
                  <input type="number" min={1} max={4_000_000_000} step={1} value={sel.seed} onChange={(e) => patchLayer(sel.id, { seed: Math.max(1, Math.floor(Number(e.target.value) || 1)) })} />
                  <button className="ghost small" title={t.rerollSeed} onClick={() => patchLayer(sel.id, { seed: 1 + Math.floor(Math.random() * 1_000_000) })}>
                    🎲
                  </button>
                </label>
              )}
              <label className="drone-slider">
                <span>{t.lowpassLabel}</span>
                <input type="range" min={0} max={1} step={0.005} value={Math.log(sel.lowpass_hz / 40) / Math.log(500)} onChange={(e) => patchLayer(sel.id, { lowpass_hz: Math.round(40 * Math.pow(500, Number(e.target.value))) })} onDoubleClick={() => patchLayer(sel.id, { lowpass_hz: 20000 })} />
                <span className="mono-num small">{sel.lowpass_hz >= 20000 ? "—" : `${sel.lowpass_hz} Hz`}</span>
              </label>
              <label className="drone-slider">
                <span>{t.highpassLabel}</span>
                <input type="range" min={0} max={1} step={0.005} value={Math.log(sel.highpass_hz / 10) / Math.log(800)} onChange={(e) => patchLayer(sel.id, { highpass_hz: Math.round(10 * Math.pow(800, Number(e.target.value))) })} onDoubleClick={() => patchLayer(sel.id, { highpass_hz: 10 })} />
                <span className="mono-num small">{sel.highpass_hz <= 10 ? "—" : `${sel.highpass_hz} Hz`}</span>
              </label>
            </div>
          )}
        </aside>
      </div>

      {confirm?.kind === "remove" && (
        <ConfirmDialog
          title={t.removeLayer}
          body={t.removeLayerExplain}
          confirmLabel={t.removeLayer}
          cancelLabel={t.cancel}
          onConfirm={() => {
            removeLayer(confirm.id);
            setConfirm(null);
          }}
          onCancel={() => setConfirm(null)}
        />
      )}
      {confirm?.kind === "starter" && (
        <ConfirmDialog
          title={t[confirm.key]}
          body={t.starterReplaceExplain}
          confirmLabel={t.starterReplace}
          cancelLabel={t.cancel}
          onConfirm={() => {
            applyStarter(confirm.key);
            setConfirm(null);
          }}
          onCancel={() => setConfirm(null)}
        />
      )}
    </main>
  );
}
