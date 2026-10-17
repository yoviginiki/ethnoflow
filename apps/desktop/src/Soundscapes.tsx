import { useEffect, useState } from "react";
import { api, describeError, type AmbientLayer, type LayerKind, type LayerSource } from "./api";
import { DEFAULT_MACROS, KIND_LABEL, MAX_AMBIENT_LAYERS, defaultSource, makeLayer } from "./ambientPresets";
import { mulberry, randomizeLayer } from "./ambientRandom";
import { MACRO_SLIDERS, REGISTER, SOURCE_SLIDERS, hasTonal, pct } from "./ambientSpecs";
import { BackgroundTransport } from "./BackgroundTransport";
import { ConfirmDialog } from "./ConfirmDialog";
import { Knob } from "./Knob";
import { PresetLibrary } from "./PresetLibrary";
import { formatDb, linearToDb, meterFraction, noteParts, takeName } from "./format";
import { messages, type StringMessageKey } from "./i18n";
import { commitRack, flushRack } from "./rack";
import { useStore } from "./store";

/** Module kinds in the order of the "new module" menu. */
const KINDS: LayerKind[] = ["cloud", "wind", "texture", "bells", "pluck", "noise", "loop"];
/** How to use each module (the teaching text under the title). */
const HOW_TO: Record<LayerKind, StringMessageKey> = { noise: "howNoise", wind: "howWind", texture: "howTexture", bells: "howBells", pluck: "howPluck", loop: "howLoop", cloud: "howCloud" };
const NEEDS_TAKE = (k: LayerKind) => k === "loop" || k === "cloud";
const POLL_MS = 150;

/** Log-scaled knob position (0..1) ↔ Hz for the layer filters. */
const HZ_MIN = 40;
const HZ_MAX = 20_000;
const hzToPos = (hz: number) => Math.log2(Math.max(HZ_MIN, hz) / HZ_MIN) / Math.log2(HZ_MAX / HZ_MIN);
const posToHz = (p: number) => Math.round(HZ_MIN * Math.pow(HZ_MAX / HZ_MIN, Math.min(1, Math.max(0, p))));
const fmtHz = (hz: number) => (hz >= 1000 ? `${(hz / 1000).toFixed(1)} kHz` : `${Math.round(hz)} Hz`);

interface ModulePreset {
  name?: string;
  source: LayerSource;
  gain_db?: number;
  pan?: number;
  lowpass_hz?: number;
  highpass_hz?: number;
}

/**
 * Soundscapes (F1B.1): the atmosphere rack seen one module at a time, with big knobs,
 * a how-to text per module, per-module presets, macros and a mini mixer. It edits the
 * same rack as the Atmosphere screen, so the two always agree.
 */
export function Soundscapes() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const rack = s.ambient;
  const status = s.ambientStatus;
  const layers = rack?.layers ?? [];
  const [selected, setSelected] = useState<string | null>(null);
  const [newKind, setNewKind] = useState<LayerKind>("wind");
  const [newTake, setNewTake] = useState("");
  const [confirmRemove, setConfirmRemove] = useState<string | null>(null);
  const [editingName, setEditingName] = useState(false);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));
  const sel = layers.find((l) => l.id === selected) ?? layers[0] ?? null;
  const selIndex = sel ? layers.indexOf(sel) : -1;
  const noScale = !project.scale;
  const noteName = (midi: number) => {
    const np = noteParts(midi);
    return `${t.noteNames[np.pc]}${np.octave}`;
  };

  useEffect(() => {
    if (!useStore.getState().ambient) {
      api
        .ambientGet()
        .then((v) => useStore.getState().setAmbient(v.ambient))
        .catch(fail);
    }
    const id = window.setInterval(() => {
      api
        .ambientStatus()
        .then((st) => useStore.getState().setAmbientStatus(st))
        .catch(() => undefined);
    }, POLL_MS);
    return () => {
      window.clearInterval(id);
      flushRack();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const commit = (next: NonNullable<typeof rack>) => commitRack(next, fail);
  const patchLayer = (id: string, patch: Partial<AmbientLayer>) => {
    if (!rack) return;
    commit({ ...rack, layers: rack.layers.map((l) => (l.id === id ? { ...l, ...patch } : l)) });
  };
  const patchSource = (id: string, patch: Record<string, unknown>) => {
    const l = layers.find((x) => x.id === id);
    if (l) patchLayer(id, { source: { ...l.source, ...patch } as LayerSource });
  };
  const addModule = () => {
    if (!rack || layers.length >= MAX_AMBIENT_LAYERS) return;
    if (NEEDS_TAKE(newKind) && !newTake) return;
    const take = project.takes.find((x) => x.id === newTake);
    const name = NEEDS_TAKE(newKind) && take ? `${t[KIND_LABEL[newKind]]}: ${takeName(take, t.take)}` : t[KIND_LABEL[newKind]];
    const layer = makeLayer(name, defaultSource(newKind, newTake), { gain_db: newKind === "noise" ? -24 : -18, pan: ((layers.length % 3) - 1) * 0.3 });
    commit({ ...rack, layers: [...rack.layers, layer] });
    setSelected(layer.id);
  };
  const removeModule = (id: string) => {
    if (!rack) return;
    commit({ ...rack, layers: rack.layers.filter((l) => l.id !== id), automation: rack.automation.filter((a) => a.target.layer !== id) });
    setSelected(null);
  };
  /** New knob values for this module only; the level and pan (the user's mix) stay. */
  const randomizeModule = () => {
    if (!sel) return;
    const rnd = mulberry(1 + (Date.now() % 1_000_000));
    const nl = randomizeLayer(rnd, { ...sel, locked: false });
    patchLayer(sel.id, { source: nl.source, seed: nl.seed });
  };
  const applyModulePreset = (p: unknown) => {
    if (!sel || !p || typeof p !== "object" || !("source" in p)) return;
    const m = p as ModulePreset;
    const src = m.source;
    // A loop preset from another project points at a take we do not have: keep ours.
    if ("take_id" in src && !project.takes.some((x) => x.id === src.take_id)) {
      if ("take_id" in sel.source) src.take_id = sel.source.take_id;
      else return;
    }
    patchLayer(sel.id, { source: src, gain_db: m.gain_db ?? sel.gain_db, pan: m.pan ?? sel.pan, lowpass_hz: m.lowpass_hz ?? sel.lowpass_hz, highpass_hz: m.highpass_hz ?? sel.highpass_hz });
  };
  const setMacro = (key: keyof typeof DEFAULT_MACROS, v: number) => rack && commit({ ...rack, macros: { ...rack.macros, [key]: v } });
  const macros = rack?.macros ?? DEFAULT_MACROS;
  const peakOf = (i: number) => (status?.layer_peaks[i] !== undefined ? linearToDb(status.layer_peaks[i]) : -60);
  const selPeak = selIndex >= 0 ? peakOf(selIndex) : -60;

  return (
    <section className="soundscapes">
      <div className="muted small">{t.ssIntro}</div>
      <div className="analysis-row">
        <BackgroundTransport compact />
        <label className="inline">
          {t.moduleLabel}
          <select value={sel?.id ?? ""} onChange={(e) => setSelected(e.target.value)} disabled={layers.length === 0}>
            {layers.length === 0 && <option value="">{t.noModules}</option>}
            {layers.map((l) => (
              <option key={l.id} value={l.id}>
                {l.name}
                {t[KIND_LABEL[l.source.kind]] !== l.name ? ` · ${t[KIND_LABEL[l.source.kind]]}` : ""}
              </option>
            ))}
          </select>
        </label>
        <label className="inline">
          {t.newModule}
          <select value={newKind} onChange={(e) => setNewKind(e.target.value as LayerKind)}>
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {t[KIND_LABEL[k]]}
              </option>
            ))}
          </select>
        </label>
        {NEEDS_TAKE(newKind) && (
          <select value={newTake} onChange={(e) => setNewTake(e.target.value)} aria-label={t.take}>
            <option value="">…</option>
            {project.takes.map((tk) => (
              <option key={tk.id} value={tk.id}>
                {takeName(tk, t.take)}
              </option>
            ))}
          </select>
        )}
        <button onClick={addModule} disabled={!rack || layers.length >= MAX_AMBIENT_LAYERS || (NEEDS_TAKE(newKind) && !newTake)}>
          + {t.addModule}
        </button>
        {layers.length >= MAX_AMBIENT_LAYERS && <span className="muted small">{t.maxLayers}</span>}
      </div>

      <div className="ss-layout">
        <div className="panel ss-module">
          {!sel && <div className="muted">{t.noModulesHint}</div>}
          {sel && (
            <>
              <div className="analysis-row">
                {editingName ? (
                  <input
                    autoFocus
                    defaultValue={sel.name}
                    maxLength={40}
                    aria-label={t.renameHint}
                    onBlur={(e) => {
                      patchLayer(sel.id, { name: e.target.value.trim() || sel.name });
                      setEditingName(false);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                      if (e.key === "Escape") setEditingName(false);
                    }}
                  />
                ) : (
                  <h2 className="ss-title" title={t.renameHint} onDoubleClick={() => setEditingName(true)}>
                    {sel.name}
                  </h2>
                )}
                <span className="badge">{t[KIND_LABEL[sel.source.kind]]}</span>
                <button className={`small ${sel.mute ? "" : "primary"}`} onClick={() => patchLayer(sel.id, { mute: !sel.mute })} aria-pressed={!sel.mute}>
                  {sel.mute ? t.moduleOff : t.moduleOn}
                </button>
                <button className="ghost small" onClick={randomizeModule} title={t.randomizeModuleHint}>
                  🎲 {t.randomizeModule}
                </button>
                <button className={`ghost small ${sel.locked ? "on" : ""}`} onClick={() => patchLayer(sel.id, { locked: !sel.locked })} title={t.lockLayer} aria-pressed={sel.locked}>
                  {sel.locked ? "🔒" : "🔓"}
                </button>
                <PresetLibrary
                  kind="module"
                  compact
                  getPayload={() => ({ name: sel.name, source: sel.source, gain_db: sel.gain_db, pan: sel.pan, lowpass_hz: sel.lowpass_hz, highpass_hz: sel.highpass_hz })}
                  apply={applyModulePreset}
                />
                <button className="ghost small" onClick={() => setConfirmRemove(sel.id)} title={t.removeLayer}>
                  ✕
                </button>
              </div>
              <div className="ss-howto small">
                <strong>{t.howToTitle}:</strong> {t[HOW_TO[sel.source.kind]]}
              </div>
              <div className="ss-knobs">
                <div className="ss-knob">
                  <Knob value={sel.gain_db} min={-60} max={12} step={0.5} reset={-18} size={72} label={t.levelLabel} text={sel.gain_db <= -59.5 ? "off" : formatDb(sel.gain_db)} onChange={(v) => patchLayer(sel.id, { gain_db: v })} />
                  <span className="small">{t.levelLabel}</span>
                  <div className="hmeter" aria-hidden="true">
                    <div className={`hmeter-fill ${selPeak > -3 ? "hot" : selPeak > -12 ? "warm" : ""}`} style={{ width: `${meterFraction(selPeak) * 100}%` }} />
                  </div>
                </div>
                <div className="ss-knob">
                  <Knob value={sel.pan} min={-1} max={1} step={0.05} reset={0} size={72} label={t.panLabel} text={sel.pan === 0 ? "C" : sel.pan < 0 ? `L${Math.round(-sel.pan * 100)}` : `R${Math.round(sel.pan * 100)}`} onChange={(v) => patchLayer(sel.id, { pan: v })} />
                  <span className="small">{t.panShort}</span>
                </div>
                {SOURCE_SLIDERS[sel.source.kind].map((sp) => {
                  const v = (sel.source as unknown as Record<string, number>)[sp.key];
                  return (
                    <div className="ss-knob" key={sp.key}>
                      <Knob value={v} min={sp.min} max={sp.max} step={sp.step} size={72} label={t[sp.label]} text={sp.fmt(v)} onChange={(x) => patchSource(sel.id, { [sp.key]: x })} />
                      <span className="small">{t[sp.label]}</span>
                    </div>
                  );
                })}
                <div className="ss-knob">
                  <Knob value={hzToPos(sel.highpass_hz)} min={0} max={1} step={0.005} reset={0} size={56} label={t.highpassLabel} text={fmtHz(sel.highpass_hz)} onChange={(p) => patchLayer(sel.id, { highpass_hz: Math.min(8000, Math.max(10, posToHz(p))) })} />
                  <span className="small">HP</span>
                </div>
                <div className="ss-knob">
                  <Knob value={hzToPos(sel.lowpass_hz)} min={0} max={1} step={0.005} reset={1} size={56} label={t.lowpassLabel} text={fmtHz(sel.lowpass_hz)} onChange={(p) => patchLayer(sel.id, { lowpass_hz: posToHz(p) })} />
                  <span className="small">LP</span>
                </div>
              </div>
              {"take_id" in sel.source && (
                <div className="analysis-row">
                  <label className="inline">
                    {t.take}
                    <select value={sel.source.take_id} onChange={(e) => patchSource(sel.id, { take_id: e.target.value })}>
                      {project.takes.map((tk) => (
                        <option key={tk.id} value={tk.id}>
                          {takeName(tk, t.take)}
                        </option>
                      ))}
                    </select>
                  </label>
                  {sel.source.kind === "loop" && (
                    <label className="inline">
                      <input type="checkbox" checked={sel.source.reverse} onChange={(e) => patchSource(sel.id, { reverse: e.target.checked })} />
                      {t.reverseLabel}
                    </label>
                  )}
                </div>
              )}
              {hasTonal(sel.source) && (
                <div className="analysis-row">
                  <label className="inline">
                    <input type="checkbox" checked={sel.source.tonal} onChange={(e) => patchSource(sel.id, { tonal: e.target.checked })} />
                    {sel.source.tonal ? t.tonalLabel : t.noKeyLabel}
                  </label>
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
                  {sel.source.tonal && noScale && <span className="muted small">{t.noScaleForTonal}</span>}
                </div>
              )}
            </>
          )}
        </div>

        <aside className="panel ss-mixer">
          <h3>{t.mixerTitle}</h3>
          {layers.length === 0 && <div className="muted small">{t.noModules}</div>}
          {layers.map((l, i) => {
            const pk = peakOf(i);
            return (
              <div key={l.id} className={`ss-mix-row ${sel?.id === l.id ? "selected" : ""} ${l.mute ? "dim" : ""}`} onClick={() => setSelected(l.id)} role="button" tabIndex={0} onKeyDown={(e) => e.key === "Enter" && setSelected(l.id)}>
                <span className="grow small">{l.name}</span>
                <div className="hmeter" aria-hidden="true">
                  <div className={`hmeter-fill ${pk > -3 ? "hot" : pk > -12 ? "warm" : ""}`} style={{ width: `${meterFraction(pk) * 100}%` }} />
                </div>
                <span onClick={(e) => e.stopPropagation()}>
                  <Knob value={l.gain_db} min={-60} max={12} step={0.5} reset={-18} size={34} label={`${t.levelLabel}: ${l.name}`} onChange={(v) => patchLayer(l.id, { gain_db: v })} />
                </span>
                <button className={`small ${l.mute ? "on" : "ghost"}`} title={t.mute} onClick={(e) => (e.stopPropagation(), patchLayer(l.id, { mute: !l.mute }))}>
                  {t.muteAbbr}
                </button>
              </div>
            );
          })}
          <h3>{t.macrosTitle}</h3>
          <div className="ss-knobs">
            {MACRO_SLIDERS.map(({ key, label }) => (
              <div className="ss-knob narrow" key={key}>
                <Knob value={macros[key]} min={0} max={1} step={0.01} reset={DEFAULT_MACROS[key]} size={46} label={t[label]} text={pct(macros[key])} disabled={!rack} onChange={(v) => setMacro(key, v)} />
                <span className="small">{t[label]}</span>
              </div>
            ))}
          </div>
          <div className="muted small">{t.macroHint}</div>
        </aside>
      </div>
      {confirmRemove && (
        <ConfirmDialog
          title={t.removeLayer}
          body={t.removeLayerExplain}
          confirmLabel={t.removeLayer}
          cancelLabel={t.cancel}
          onConfirm={() => {
            removeModule(confirmRemove);
            setConfirmRemove(null);
          }}
          onCancel={() => setConfirmRemove(null)}
        />
      )}
    </section>
  );
}
