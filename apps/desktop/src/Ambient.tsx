import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { AMBIENT_TAGS, api, describeError, IMPORT_EXTENSIONS, type Ambient as AmbientRack, type AmbientLayer, type AmbientTag, type LayerKind, type LayerSource, type Macros, type PresetSummary } from "./api";
import { DEFAULT_MACROS, KIND_LABEL, MAX_AMBIENT_LAYERS, defaultSource, makeLayer } from "./ambientPresets";
import { randomizeRack } from "./ambientRandom";
import { MACRO_SLIDERS, REGISTER, SHIMMER_ST, SOURCE_SLIDERS, hasTonal, pct } from "./ambientSpecs";
import { LaneEditor, ProgramPanel } from "./AmbientProgram";
import { BackgroundTransport } from "./BackgroundTransport";
import { ConfirmDialog } from "./ConfirmDialog";
import { PresetLibrary } from "./PresetLibrary";
import { rackForProject } from "./session";
import { Knob } from "./Knob";
import { formatDb, linearToDb, meterFraction, noteParts, takeName } from "./format";
import { fadeOutBackground } from "./layers";
import { messages, type StringMessageKey } from "./i18n";
import { useStore } from "./store";

const KINDS: LayerKind[] = ["noise", "wind", "texture", "bells", "pluck"];
/** Audio files that can become loop layers (MIDI is not a sample). */
const SAMPLE_EXTENSIONS = IMPORT_EXTENSIONS.filter((e) => e !== "mid");
const fileStem = (path: string) => (path.split(/[\\/]/).pop() ?? "").replace(/\.[^.]+$/, "");
const HISTORY_MAX = 20;
/** Factory preset ids → their names in the UI language (user presets show their file name). */
const PRESET_NAME: Record<string, StringMessageKey> = {
  mountain_mist: "presetMountainMist",
  forest_morning: "presetForestMorning",
  river: "presetRiver",
  night: "presetNight",
  fog_no_key: "presetFogNoKey",
  meditation: "presetMeditation",
  warm_hearth: "presetWarmHearth",
  liquid_glass: "presetLiquidGlass",
  dark_cave: "presetDarkCave",
};
const TAG_NAME: Record<AmbientTag, StringMessageKey> = {
  forest: "tagForest",
  river: "tagRiver",
  night: "tagNight",
  fog: "tagFog",
  mountain: "tagMountain",
  ethno: "tagEthno",
  meditation: "tagMeditation",
  dark: "tagDark",
  warm: "tagWarm",
  liquid: "tagLiquid",
  no_key: "tagNoKey",
};
interface Snapshot {
  at: number;
  label: string;
  rack: AmbientRack;
}
const POLL_MS = 150;
const COMMIT_MS = 80;


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
  const [confirm, setConfirm] = useState<{ kind: "remove"; id: string } | { kind: "deletePreset"; id: string } | null>(null);
  const [presets, setPresets] = useState<PresetSummary[]>([]);
  const [tagFilter, setTagFilter] = useState<AmbientTag | "fav" | "recent" | null>(null);
  const [search, setSearch] = useState("");
  const [saveName, setSaveName] = useState("");
  const [saveTags, setSaveTags] = useState<AmbientTag[]>([]);
  const [showSave, setShowSave] = useState(false);
  const [ab, setAb] = useState<{ a: AmbientRack | null; b: AmbientRack | null; side: "a" | "b" }>({ a: null, b: null, side: "a" });
  const [history, setHistory] = useState<Snapshot[]>([]);
  const [showLeft, setShowLeft] = useState(true);
  const [showRight, setShowRight] = useState(true);
  const [fadeSeconds, setFadeSeconds] = useState(15);
  const [editingName, setEditingName] = useState<string | null>(null);
  const [dropping, setDropping] = useState(false);
  const [importing, setImporting] = useState<string | null>(null);
  const fail = useCallback((e: unknown) => useStore.getState().setError(describeError(e)), []);
  const pendingRef = useRef<AmbientRack | null>(null);
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    api.ambientPresets().then(setPresets).catch(fail);
  }, [fail]);

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
    () => ({ noise: t.kindNoise, wind: t.kindWind, texture: t.kindTexture, bells: t.kindBells, pluck: t.kindPluck, loop: t.kindLoop, cloud: t.kindCloud }),
    [t],
  );

  /** Remember the current rack before a big change (preset, surprise, A/B, starter). */
  const snapshot = (label: string) => {
    if (!rack) return;
    setHistory((h) => [{ at: Date.now(), label, rack }, ...h].slice(0, HISTORY_MAX));
  };
  const restore = (snap: Snapshot) => {
    snapshot(t.historyBeforeRestore);
    commit(snap.rack);
    setSelected(null);
  };
  const presetName = (p: PresetSummary) => (p.factory && PRESET_NAME[p.id] ? t[PRESET_NAME[p.id]] : p.name);
  const loadPreset = async (p: PresetSummary) => {
    try {
      const r = await api.ambientPresetLoad(p.id);
      // Factory layers are named in English in the catalogue: show them in the UI language.
      if (p.factory) r.layers = r.layers.map((l) => ({ ...l, name: t.layerNames[l.name] ?? l.name }));
      snapshot(`${t.historyBeforePreset} ${presetName(p)}`);
      commit(r);
      setSelected(null);
      const recent = [p.id, ...(s.prefs.recentPresets ?? []).filter((x) => x !== p.id)].slice(0, 8);
      s.setPref("recentPresets", recent);
    } catch (e) {
      fail(e);
    }
  };
  const toggleFav = (id: string) => {
    const fav = s.prefs.favPresets ?? [];
    s.setPref("favPresets", fav.includes(id) ? fav.filter((x) => x !== id) : [...fav, id]);
  };
  const savePreset = async () => {
    if (!rack || !saveName.trim()) return;
    try {
      setPresets(await api.ambientPresetSave(saveName.trim(), saveTags, rack));
      s.setMessage(`${t.presetSaved}: ${saveName.trim()}`);
      setShowSave(false);
      setSaveName("");
    } catch (e) {
      fail(e);
    }
  };
  const deletePreset = async (id: string) => {
    try {
      setPresets(await api.ambientPresetDelete(id));
    } catch (e) {
      fail(e);
    }
  };
  const surprise = () => {
    if (!rack || rack.layers.length === 0) return;
    snapshot(t.historyBeforeSurprise);
    commit(randomizeRack(rack, Math.floor(Math.random() * 2_000_000_000)));
  };
  const undoLast = () => {
    const [last, ...rest] = history;
    if (!last) return;
    setHistory(rest);
    commit(last.rack);
    setSelected(null);
  };
  const storeAb = (side: "a" | "b") => rack && setAb({ ...ab, [side]: rack, side });
  const switchAb = (side: "a" | "b") => {
    const target = ab[side];
    if (!target || !rack) return;
    // Keep the other side current so A/B flips back to what was just heard.
    setAb({ ...ab, [ab.side]: rack, side });
    commit(target);
    setSelected(null);
  };
  const filteredPresets = presets.filter((p) => {
    if (tagFilter === "fav" && !(s.prefs.favPresets ?? []).includes(p.id)) return false;
    if (tagFilter === "recent" && !(s.prefs.recentPresets ?? []).includes(p.id)) return false;
    if (tagFilter && tagFilter !== "fav" && tagFilter !== "recent" && !p.tags.includes(tagFilter)) return false;
    const q = search.trim().toLowerCase();
    return !q || presetName(p).toLowerCase().includes(q) || p.tags.some((tg) => t[TAG_NAME[tg as AmbientTag]]?.toLowerCase().includes(q));
  });

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
  /**
   * Import sample files (from a dialog or a drop from Explorer / a sample manager such as
   * Soundly) as takes of the project and add one loop layer per file.
   */
  const importSamples = async (paths: string[]) => {
    const files = paths.filter((p) => SAMPLE_EXTENSIONS.includes(p.split(".").pop()?.toLowerCase() ?? ""));
    if (files.length === 0 || importing) return;
    const added: AmbientLayer[] = [];
    let skipped = 0;
    try {
      for (const path of files) {
        const cur = useStore.getState().ambient;
        if (!cur) break;
        const n = cur.layers.length + added.length;
        if (n >= MAX_AMBIENT_LAYERS) {
          skipped++;
          continue;
        }
        const stem = fileStem(path);
        setImporting(`${t.importing} ${stem}`);
        let pv = await api.importAudio(path);
        const id = pv.loaded_take;
        if (!id) continue;
        const tk = pv.takes.find((x) => x.id === id);
        if (tk && !tk.label && stem) pv = await api.renameTake(id, stem);
        useStore.getState().setProject(pv);
        added.push(makeLayer(stem || kindNames.loop, defaultSource("loop", id), { gain_db: -18, pan: ((n % 3) - 1) * 0.3 }));
      }
      const cur = useStore.getState().ambient;
      if (cur && added.length > 0) {
        commit({ ...cur, layers: [...cur.layers, ...added] });
        setSelected(added[added.length - 1].id);
      }
      useStore.getState().setMessage(`${t.samplesImported}: ${added.length}${skipped > 0 ? ` · ${t.maxLayers}` : ""}`);
    } catch (e) {
      fail(e);
    } finally {
      setImporting(null);
    }
  };
  const importRef = useRef(importSamples);
  importRef.current = importSamples;
  const pickSamples = async () => {
    const r = await open({ title: t.importSamples, multiple: true, directory: false, filters: [{ name: t.audioFiles, extensions: SAMPLE_EXTENSIONS }] });
    if (!r) return;
    await importSamples(Array.isArray(r) ? r : [r]);
  };
  // Files dragged from Explorer or a sample manager onto the window become loop layers.
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | null = null;
    try {
      getCurrentWebview()
        .onDragDropEvent((ev) => {
          if (ev.payload.type === "enter") setDropping(true);
          else if (ev.payload.type === "leave") setDropping(false);
          else if (ev.payload.type === "drop") {
            setDropping(false);
            void importRef.current(ev.payload.paths);
          }
        })
        .then((u) => {
          if (alive) unlisten = u;
          else u();
        })
        .catch(() => {});
    } catch {
      // Not running inside Tauri (tests): no native drop.
    }
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);
  const removeLayer = (id: string) => {
    if (!rack) return;
    commit({ ...rack, layers: rack.layers.filter((l) => l.id !== id), automation: rack.automation.filter((l) => l.target.layer !== id) });
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
  const setMacro = (key: keyof Macros, v: number) => rack && commit({ ...rack, macros: { ...rack.macros, [key]: v } });

  const masterDb = s.prefs.masterDb ?? 0;
  const stripControl = s.prefs.stripControl ?? "slider";
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
          <label className="inline small">
            {t.viewLabel}
            <select value={stripControl} onChange={(e) => s.setPref("stripControl", e.target.value as "slider" | "knob")}>
              <option value="slider">{t.viewSliders}</option>
              <option value="knob">{t.viewKnobs}</option>
            </select>
          </label>
          <div className="meter" style={{ width: 120 }} role="meter" aria-valuemin={-60} aria-valuemax={0} aria-valuenow={Math.round(masterPeakDb)} aria-label={t.mixTitle}>
            <div className={`meter-fill ${masterPeakDb > -3 ? "hot" : masterPeakDb > -12 ? "warm" : ""}`} style={{ width: `${meterFraction(masterPeakDb) * 100}%` }} />
          </div>
          <span className={`badge warn ${status && status.limiter_gain < 0.98 ? "" : "invisible"}`} aria-hidden={!(status && status.limiter_gain < 0.98)}>
            {t.limiterActive}
          </span>
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

      <div className={`ambient-body ${showLeft ? "" : "left-collapsed"} ${showRight ? "" : "right-collapsed"}`}>
        <aside className="panel ambient-left">
          <div className="panel-head">
            {showLeft && <h3>{t.presetsTitle}</h3>}
            <button className="ghost small" title={showLeft ? t.minimizePanel : t.restorePanel} aria-expanded={showLeft} onClick={() => setShowLeft((v) => !v)}>
              {showLeft ? "◀" : "▶"}
            </button>
          </div>
          {showLeft && (
            <>
          <input type="search" value={search} placeholder={t.presetSearch} onChange={(e) => setSearch(e.target.value)} aria-label={t.presetSearch} />
          <div className="tag-row">
            <button className={`chip ${tagFilter === null ? "on" : ""}`} onClick={() => setTagFilter(null)}>
              {t.tagAll}
            </button>
            <button className={`chip ${tagFilter === "fav" ? "on" : ""}`} onClick={() => setTagFilter("fav")}>
              ★ {t.tagFavorites}
            </button>
            <button className={`chip ${tagFilter === "recent" ? "on" : ""}`} onClick={() => setTagFilter("recent")}>
              {t.tagRecent}
            </button>
            {AMBIENT_TAGS.map((tg) => (
              <button key={tg} className={`chip ${tagFilter === tg ? "on" : ""}`} onClick={() => setTagFilter(tagFilter === tg ? null : tg)}>
                {t[TAG_NAME[tg]]}
              </button>
            ))}
          </div>
          <ul className="preset-list">
            {filteredPresets.map((p) => (
              <li key={p.id} className={rack?.preset === p.id ? "current" : ""}>
                <button className="ghost preset-name" onClick={() => void loadPreset(p)} title={p.tags.map((tg) => t[TAG_NAME[tg as AmbientTag]] ?? tg).join(", ")}>
                  {presetName(p)} <span className="muted small">· {p.layer_count}</span>
                </button>
                <button className={`ghost small ${(s.prefs.favPresets ?? []).includes(p.id) ? "on" : ""}`} title={t.tagFavorites} onClick={() => toggleFav(p.id)}>
                  {(s.prefs.favPresets ?? []).includes(p.id) ? "★" : "☆"}
                </button>
                {!p.factory && (
                  <button className="ghost small" title={t.presetDelete} onClick={() => setConfirm({ kind: "deletePreset", id: p.id })}>
                    ✕
                  </button>
                )}
              </li>
            ))}
            {filteredPresets.length === 0 && <li className="muted small">{t.presetNone}</li>}
          </ul>
          <div className="style-row">
            <button className="ghost" disabled={!rack || layers.length === 0} onClick={() => setShowSave((v) => !v)}>
              {t.presetSaveAs}
            </button>
            <button className="ghost" disabled={!rack || layers.length === 0} onClick={surprise} title={t.surpriseHint}>
              🎲 {t.surprise}
            </button>
            <button className="ghost" disabled={history.length === 0} onClick={undoLast} title={history[0]?.label ?? ""}>
              {t.undo}
            </button>
          </div>
          <PresetLibrary
            kind="ambient"
            getPayload={() => useStore.getState().ambient}
            apply={(p) => {
              const cur = useStore.getState();
              if (!cur.ambient || !p) return;
              snapshot(t.historyBeforePreset);
              commit(rackForProject(p as AmbientRack, new Set(cur.project?.takes.map((x) => x.id) ?? [])));
            }}
          />
          {showSave && (
            <div className="panel save-preset">
              <input type="text" value={saveName} placeholder={t.presetNamePlaceholder} onChange={(e) => setSaveName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void savePreset()} />
              <div className="tag-row">
                {AMBIENT_TAGS.map((tg) => (
                  <button key={tg} className={`chip ${saveTags.includes(tg) ? "on" : ""}`} onClick={() => setSaveTags(saveTags.includes(tg) ? saveTags.filter((x) => x !== tg) : [...saveTags, tg])}>
                    {t[TAG_NAME[tg]]}
                  </button>
                ))}
              </div>
              <div className="style-row">
                <button className="primary" disabled={!saveName.trim()} onClick={() => void savePreset()}>
                  {t.presetSave}
                </button>
                <button className="ghost" onClick={() => setShowSave(false)}>
                  {t.cancel}
                </button>
              </div>
            </div>
          )}
          <div className="style-row" title={t.abHint}>
            <span className="muted small">A/B</span>
            <button className={`ghost small ${ab.side === "a" ? "on" : ""}`} onClick={() => (ab.a ? switchAb("a") : storeAb("a"))} disabled={!rack}>
              A{ab.a ? "" : " +"}
            </button>
            <button className={`ghost small ${ab.side === "b" ? "on" : ""}`} onClick={() => (ab.b ? switchAb("b") : storeAb("b"))} disabled={!rack}>
              B{ab.b ? "" : " +"}
            </button>
            <button className="ghost small" onClick={() => storeAb(ab.side)} disabled={!rack} title={t.abStore}>
              ↓ {ab.side.toUpperCase()}
            </button>
          </div>
          {history.length > 0 && (
            <details>
              <summary className="muted small">
                {t.historyTitle} ({history.length})
              </summary>
              <ul className="preset-list">
                {history.map((h) => (
                  <li key={h.at}>
                    <button className="ghost preset-name small" onClick={() => restore(h)}>
                      {new Date(h.at).toLocaleTimeString()} · {h.label}
                    </button>
                  </li>
                ))}
              </ul>
            </details>
          )}
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
          <label>
            {t.cloudFromTake}
            <select value="" disabled={layers.length >= MAX_AMBIENT_LAYERS || project.takes.length === 0} onChange={(e) => e.target.value && addLayer("cloud", e.target.value)}>
              <option value="">…</option>
              {project.takes.map((tk) => (
                <option key={tk.id} value={tk.id}>
                  {takeName(tk, t.take)}
                </option>
              ))}
            </select>
          </label>
          <div className={`dropzone ${dropping ? "active" : ""}`}>
            <button className="ghost" disabled={importing !== null || layers.length >= MAX_AMBIENT_LAYERS} onClick={() => pickSamples().catch(fail)}>
              {importing ?? t.importSamples}
            </button>
            <div className="muted small">{t.dropSamplesHint}</div>
          </div>
          {layers.length >= MAX_AMBIENT_LAYERS && <div className="muted small">{t.maxLayers}</div>}
          <div className="muted small">{t.safeStartHint}</div>
            </>
          )}
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
                  {t[KIND_LABEL[l.source.kind]] !== l.name && <span className="muted small">{t[KIND_LABEL[l.source.kind]]}</span>}
                  <div className="strip-fader" onClick={(e) => e.stopPropagation()}>
                    <div className="vmeter" aria-hidden="true">
                      <div className={`vmeter-fill ${peakDb > -3 ? "hot" : peakDb > -12 ? "warm" : ""}`} style={{ height: `${meterFraction(peakDb) * 100}%` }} />
                    </div>
                    {stripControl === "knob" ? (
                      <Knob value={l.gain_db} min={-60} max={12} step={0.5} reset={-18} label={`${t.levelLabel}: ${l.name}`} text={l.gain_db <= -59.5 ? "off" : formatDb(l.gain_db)} size={56} onChange={(v) => patchLayer(l.id, { gain_db: v })} />
                    ) : (
                      <input type="range" className="vslider" min={-60} max={12} step={0.5} value={l.gain_db} aria-label={`${t.levelLabel}: ${l.name}`} onChange={(e) => patchLayer(l.id, { gain_db: Number(e.target.value) })} onDoubleClick={() => patchLayer(l.id, { gain_db: -18 })} />
                    )}
                  </div>
                  {stripControl === "slider" && <span className="mono-num small">{l.gain_db <= -59.5 ? "off" : formatDb(l.gain_db)}</span>}
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
                    <button className={l.locked ? "on" : ""} title={t.lockLayer} aria-pressed={l.locked} onClick={(e) => (e.stopPropagation(), patchLayer(l.id, { locked: !l.locked }))}>
                      {l.locked ? "🔒" : "🔓"}
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
          <div className="panel-head">
            <button className="ghost small" title={showRight ? t.minimizePanel : t.restorePanel} aria-expanded={showRight} onClick={() => setShowRight((v) => !v)}>
              {showRight ? "▶" : "◀"}
            </button>
            {showRight && <h3>{t.macrosTitle}</h3>}
          </div>
          {showRight && (
            <>
          <div className="drone-sliders one-col">
            {MACRO_SLIDERS.map(({ key, label }) => (
              <label key={key} className="drone-slider">
                <span>{t[label]}</span>
                <input type="range" min={0} max={1} step={0.01} value={macros[key]} disabled={!rack} onChange={(e) => setMacro(key, Number(e.target.value))} onDoubleClick={() => setMacro(key, DEFAULT_MACROS[key])} />
                <span className="mono-num small">{pct(macros[key])}</span>
              </label>
            ))}
          </div>
          <label className="inline small" title={t.shimmerStHint}>
            {t.shimmerStLabel}
            <select value={macros.shimmer_st} disabled={!rack} onChange={(e) => setMacro("shimmer_st", Number(e.target.value))}>
              {SHIMMER_ST.map((o) => (
                <option key={o.st} value={o.st}>
                  {t[o.label]}
                </option>
              ))}
            </select>
          </label>
          <div className="muted small">{t.macroHint}</div>
          {rack && <LaneEditor rack={rack} layerId={null} source={null} laneTime={status?.lane_time ?? 0} onChange={commit} />}

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
              {"take_id" in sel.source && (
                <>
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
              {rack && <LaneEditor rack={rack} layerId={sel.id} source={sel.source} laneTime={status?.lane_time ?? 0} onChange={commit} />}
            </div>
          )}
            </>
          )}
        </aside>
      </div>
      {rack && <ProgramPanel rack={rack} status={status} timing={project.timing} projectName={project.name} onChange={commit} />}

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
      {confirm?.kind === "deletePreset" && (
        <ConfirmDialog
          title={t.presetDelete}
          body={t.presetDeleteExplain}
          confirmLabel={t.presetDelete}
          cancelLabel={t.cancel}
          onConfirm={() => {
            void deletePreset(confirm.id);
            setConfirm(null);
          }}
          onCancel={() => setConfirm(null)}
        />
      )}
    </main>
  );
}
