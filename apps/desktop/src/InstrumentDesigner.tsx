import { useEffect, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, describeError, type InstrumentPreset, type InstrumentSummary, type InstrumentVoice, MAX_EXTRAS, type ExtraKind } from "./api";
import { ConfirmDialog } from "./ConfirmDialog";
import { Knob } from "./Knob";
import { formatDb } from "./format";
import { messages, type StringMessageKey } from "./i18n";
import { DEFAULT_RHYTHM, useStore } from "./store";
import { newExtraPart } from "./extraParts";

type Cat = "drums" | "bass" | "synth" | "tapan" | "chordpad" | "lead";
const PART_NAME: Record<"drums" | "bass" | "ostinato" | "arp", StringMessageKey> = { drums: "igPartDrums", bass: "igPartBass", ostinato: "igPartOstinato", arp: "igPartArp" };
const CATS: Cat[] = ["drums", "bass", "synth", "tapan", "chordpad", "lead"];
const CAT_LABEL: Record<Cat, StringMessageKey> = { drums: "igCatDrums", bass: "igCatBass", synth: "igCatSynth", tapan: "igCatTapan", chordpad: "igCatChordPad", lead: "igCatLead" };
/** Rack kind for the categories that live in the Instrument Rack (P1 R3). */
const RACK_KIND: Partial<Record<string, ExtraKind>> = { tapan: "tapan", chordpad: "chord_pad", lead: "lead" };
/** Factory ids → localized names. */
const FACTORY_NAME: Record<string, StringMessageKey> = {
  "factory:drums.liquid": "ifLiquidKit",
  "factory:drums.dark": "ifDarkKit",
  "factory:drums.ethno": "ifEthnoKit",
  "factory:drums.soft": "ifSoftKit",
  "factory:bass.sub": "ifSub",
  "factory:bass.pluck": "ifPluck",
  "factory:bass.reese": "ifReese",
  "factory:bass.deep": "ifDeep",
  "factory:bass.growl": "ifGrowl",
  "factory:bass.wood": "ifWood",
  "factory:synth.breath": "ifBreath",
  "factory:synth.glass": "ifGlass",
  "factory:synth.marimba": "ifMarimba",
  "factory:synth.pad": "ifSoftPad",
  "factory:tapan.folk": "ifTapanFolk",
  "factory:tapan.deep": "ifTapanDeep",
  "factory:tapan.tight": "ifTapanTight",
  "factory:chordpad.warm": "ifPadWarm",
  "factory:chordpad.glass": "ifPadGlass",
  "factory:chordpad.dark": "ifPadDark",
  "factory:lead.kaval": "ifLeadKaval",
  "factory:lead.saw": "ifLeadSaw",
  "factory:lead.hollow": "ifLeadHollow",
};
export function instrumentName(p: { id: string; name: string }, t: (typeof messages)["bg"]): string {
  const k = FACTORY_NAME[p.id];
  return k ? t[k] : p.name;
}

interface KnobSpec {
  key: string;
  label: StringMessageKey;
  tip: StringMessageKey;
  min: number;
  max: number;
  step: number;
  reset: number;
  log?: boolean;
  fmt: (v: number) => string;
}
const ms = (v: number) => (v >= 1 ? `${v.toFixed(2)} s` : `${Math.round(v * 1000)} ms`);
const pct = (v: number) => `${Math.round(v * 100)}%`;
const hz = (v: number) => (v >= 1000 ? `${(v / 1000).toFixed(2)} kHz` : `${Math.round(v)} Hz`);
const st = (v: number) => `${v > 0 ? "+" : ""}${Math.round(v)} st`;

const BASS_GROUPS: { title: StringMessageKey; knobs: KnobSpec[] }[] = [
  {
    title: "igShape",
    knobs: [
      { key: "attack_s", label: "igAttack", tip: "igAttackTip", min: 0.0005, max: 2, step: 0.0005, reset: 0.004, log: true, fmt: ms },
      { key: "decay_s", label: "igDecay", tip: "igDecayTip", min: 0.01, max: 4, step: 0.005, reset: 0.2, log: true, fmt: ms },
      { key: "sustain", label: "igSustain", tip: "igSustainTip", min: 0, max: 1, step: 0.01, reset: 1, fmt: pct },
      { key: "release_s", label: "igRelease", tip: "igReleaseTip", min: 0.01, max: 2, step: 0.005, reset: 0.08, log: true, fmt: ms },
      { key: "pitch_env_st", label: "igPitchEnv", tip: "igPitchEnvTip", min: -24, max: 24, step: 1, reset: 0, fmt: st },
      { key: "pitch_env_s", label: "igPitchTime", tip: "igPitchTimeTip", min: 0.005, max: 1, step: 0.005, reset: 0.05, log: true, fmt: ms },
      { key: "microvar", label: "igMicrovar", tip: "igMicrovarTip", min: 0, max: 1, step: 0.01, reset: 0, fmt: pct },
    ],
  },
  {
    title: "igFilter",
    knobs: [
      { key: "cutoff_hz", label: "igCutoff", tip: "igCutoffTip", min: 60, max: 8000, step: 1, reset: 400, log: true, fmt: hz },
      { key: "resonance", label: "igResonance", tip: "igResonanceTip", min: 0, max: 0.95, step: 0.01, reset: 0.2, fmt: pct },
      { key: "filter_env", label: "igFilterEnv", tip: "igFilterEnvTip", min: 0, max: 1, step: 0.01, reset: 0, fmt: pct },
      { key: "wobble_depth", label: "igWobbleDepth", tip: "igWobbleDepthTip", min: 0, max: 1, step: 0.01, reset: 0.5, fmt: pct },
      { key: "drive", label: "igDrive", tip: "igDriveTip", min: 0, max: 1, step: 0.01, reset: 0.3, fmt: pct },
      { key: "warmth", label: "igWarmth", tip: "igWarmthTip", min: 0, max: 1, step: 0.01, reset: 0.5, fmt: pct },
      { key: "glide_s", label: "igGlide", tip: "igGlideTip", min: 0, max: 1, step: 0.005, reset: 0.08, fmt: ms },
      { key: "sidechain", label: "igSidechain", tip: "igSidechainTip", min: 0, max: 1, step: 0.01, reset: 0.6, fmt: pct },
    ],
  },
];
const DRUM_GROUPS: { title: StringMessageKey; knobs: KnobSpec[] }[] = [
  {
    title: "igSource",
    knobs: [
      { key: "kick_hz", label: "igKickHz", tip: "igKickHzTip", min: 35, max: 90, step: 1, reset: 50, fmt: hz },
      { key: "decay", label: "igDrumDecay", tip: "igDrumDecayTip", min: 0, max: 1, step: 0.01, reset: 0.5, fmt: pct },
      { key: "brightness", label: "igBrightness", tip: "igBrightnessTip", min: 0, max: 1, step: 0.01, reset: 0.5, fmt: pct },
    ],
  },
];
const DRUM_NAMES: StringMessageKey[] = ["igKick", "igSnare", "igGhost", "igHatClosed", "igHatOpen", "igShaker"];
const SYNTH_GROUPS: { title: StringMessageKey; knobs: KnobSpec[] }[] = [
  {
    title: "igShape",
    knobs: [
      { key: "attack_s", label: "igAttack", tip: "igAttackTip", min: 0.001, max: 2, step: 0.001, reset: 0.01, log: true, fmt: ms },
      { key: "release_s", label: "igRelease", tip: "igReleaseTip", min: 0.01, max: 4, step: 0.005, reset: 0.15, log: true, fmt: ms },
      { key: "tone_hz", label: "igTone", tip: "igToneTip", min: 200, max: 20000, step: 1, reset: 6000, log: true, fmt: hz },
      { key: "dynamics", label: "igDynamics", tip: "igDynamicsTip", min: 0, max: 1, step: 0.01, reset: 0.6, fmt: pct },
    ],
  },
];

const TAPAN_GROUPS: { title: StringMessageKey; knobs: KnobSpec[] }[] = [
  {
    title: "igSource",
    knobs: [
      { key: "tune_hz", label: "igTune", tip: "igTuneTip", min: 45, max: 110, step: 1, reset: 70, fmt: hz },
      { key: "decay", label: "igDrumDecay", tip: "igDrumDecayTip", min: 0, max: 1, step: 0.01, reset: 0.5, fmt: pct },
      { key: "skin", label: "igSkin", tip: "igSkinTip", min: 0, max: 1, step: 0.01, reset: 0.45, fmt: pct },
      { key: "rim_tone", label: "igRimTone", tip: "igRimToneTip", min: 0, max: 1, step: 0.01, reset: 0.5, fmt: pct },
    ],
  },
];
const TAPAN_NAMES: StringMessageKey[] = ["igBoom", "igRim", "igGhostStroke"];
const CHORDPAD_GROUPS: { title: StringMessageKey; knobs: KnobSpec[] }[] = [
  {
    title: "igShape",
    knobs: [
      { key: "attack_s", label: "igAttack", tip: "igAttackTip", min: 0.005, max: 4, step: 0.005, reset: 0.8, log: true, fmt: ms },
      { key: "release_s", label: "igRelease", tip: "igReleaseTip", min: 0.05, max: 8, step: 0.01, reset: 1.6, log: true, fmt: ms },
      { key: "dynamics", label: "igDynamics", tip: "igDynamicsTip", min: 0, max: 1, step: 0.01, reset: 0.4, fmt: pct },
    ],
  },
  {
    title: "igFilter",
    knobs: [
      { key: "detune_cents", label: "igDetune", tip: "igDetuneTip", min: 0, max: 30, step: 0.5, reset: 7, fmt: (v) => `${v.toFixed(1)} ¢` },
      { key: "cutoff_hz", label: "igCutoff", tip: "igCutoffTip", min: 200, max: 12000, step: 1, reset: 1400, log: true, fmt: hz },
      { key: "movement", label: "igMovement", tip: "igMovementTip", min: 0, max: 1, step: 0.01, reset: 0.3, fmt: pct },
    ],
  },
];
const LEAD_GROUPS: { title: StringMessageKey; knobs: KnobSpec[] }[] = [
  {
    title: "igShape",
    knobs: [
      { key: "attack_s", label: "igAttack", tip: "igAttackTip", min: 0.001, max: 2, step: 0.001, reset: 0.01, log: true, fmt: ms },
      { key: "release_s", label: "igRelease", tip: "igReleaseTip", min: 0.01, max: 4, step: 0.005, reset: 0.2, log: true, fmt: ms },
      { key: "glide_s", label: "igGlide", tip: "igGlideTip", min: 0, max: 1, step: 0.005, reset: 0.04, fmt: ms },
      { key: "vibrato_hz", label: "igVibratoHz", tip: "igVibratoHzTip", min: 0.5, max: 9, step: 0.1, reset: 5.5, fmt: (v) => `${v.toFixed(1)} Hz` },
      { key: "vibrato_cents", label: "igVibratoDepth", tip: "igVibratoDepthTip", min: 0, max: 100, step: 1, reset: 12, fmt: (v) => `${Math.round(v)} ¢` },
    ],
  },
  {
    title: "igFilter",
    knobs: [
      { key: "cutoff_hz", label: "igCutoff", tip: "igCutoffTip", min: 200, max: 12000, step: 1, reset: 1200, log: true, fmt: hz },
      { key: "resonance", label: "igResonance", tip: "igResonanceTip", min: 0, max: 0.95, step: 0.01, reset: 0.3, fmt: pct },
      { key: "env_amount", label: "igFilterEnv", tip: "igFilterEnvTip", min: 0, max: 1, step: 0.01, reset: 0.5, fmt: pct },
      { key: "drive", label: "igDrive", tip: "igDriveTip", min: 0, max: 1, step: 0.01, reset: 0.2, fmt: pct },
    ],
  },
];

const toPos = (k: KnobSpec, v: number) => (k.log ? Math.log(Math.max(k.min, v) / k.min) / Math.log(k.max / k.min) : v);
const fromPos = (k: KnobSpec, p: number) => {
  if (!k.log) return p;
  const v = k.min * Math.pow(k.max / k.min, Math.min(1, Math.max(0, p)));
  return Math.round(v / k.step) * k.step;
};

/**
 * Instrument Designer (Program P1, R2): one instrument preset at a time with real
 * knobs on the engine's voice, live audition through the graph, Save As / Update /
 * delete, lineage, and "use in the rhythm section". The browser on the left lists
 * factory and user presets by category.
 */
export function InstrumentDesigner() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [list, setList] = useState<InstrumentSummary[]>([]);
  const [cat, setCat] = useState<Cat | null>(null);
  const [query, setQuery] = useState("");
  const [preset, setPreset] = useState<InstrumentPreset | null>(null);
  const [base, setBase] = useState<InstrumentPreset | null>(null);
  const [history, setHistory] = useState<InstrumentVoice[]>([]);
  const [audition, setAudition] = useState(false);
  const [saving, setSaving] = useState<null | "as">(null);
  const [newName, setNewName] = useState("");
  const [toDelete, setToDelete] = useState<InstrumentSummary | null>(null);
  const pushTimer = useRef<number | null>(null);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));
  const dirty = !!preset && !!base && JSON.stringify(preset) !== JSON.stringify(base);
  const output = s.prefs.selectedOutput;

  const refresh = async (selectId?: string | null) => {
    try {
      const l = await api.instrumentList(cat, query.trim() || null);
      setList(l);
      const id = selectId ?? s.designerId ?? l[0]?.id ?? null;
      if (id && (!preset || preset.id !== id || selectId)) {
        const p = await api.instrumentGet(id);
        setPreset(p);
        setBase(p);
        setHistory([]);
        s.setDesignerId(id);
      }
    } catch (e) {
      fail(e);
    }
  };
  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cat, query]);
  useEffect(
    () => () => {
      void api.instrumentAuditionStop().catch(() => undefined);
    },
    [],
  );

  const select = async (id: string) => {
    if (audition) await stopAudition();
    try {
      const p = await api.instrumentGet(id);
      setPreset(p);
      setBase(p);
      setHistory([]);
      s.setDesignerId(id);
    } catch (e) {
      fail(e);
    }
  };
  const setVoice = (patch: Record<string, unknown>) => {
    if (!preset) return;
    setHistory((h) => [preset.voice, ...h].slice(0, 50));
    const voice = { ...preset.voice, ...patch } as InstrumentVoice;
    setPreset({ ...preset, voice });
    if (audition) {
      if (pushTimer.current !== null) window.clearTimeout(pushTimer.current);
      pushTimer.current = window.setTimeout(() => {
        pushTimer.current = null;
        void api.instrumentAuditionSet(voice).catch(fail);
      }, 25);
    }
  };
  const undo = () => {
    if (!preset || history.length === 0) return;
    const [v, ...rest] = history;
    setHistory(rest);
    setPreset({ ...preset, voice: v });
    if (audition) void api.instrumentAuditionSet(v).catch(fail);
  };
  const startAudition = async () => {
    if (!preset) return;
    try {
      await api.instrumentAuditionStart(preset.voice, output, null, -6);
      setAudition(true);
    } catch (e) {
      fail(e);
    }
  };
  const stopAudition = async () => {
    setAudition(false);
    await api.instrumentAuditionStop().catch(() => undefined);
  };
  const saveAs = async () => {
    if (!preset || !newName.trim()) return;
    try {
      const saved = await api.instrumentSave({ ...preset, id: "", name: newName.trim(), parent_id: preset.id, factory: false, author: "user" });
      setSaving(null);
      setNewName("");
      s.setMessage(`${t.presetSaved}: ${saved.name}`);
      await refresh(saved.id);
    } catch (e) {
      fail(e);
    }
  };
  const update = async () => {
    if (!preset || preset.factory) return;
    try {
      const saved = await api.instrumentUpdate(preset);
      setPreset(saved);
      setBase(saved);
      s.setMessage(`${t.presetUpdated}: ${saved.name}`);
      await refresh(saved.id);
    } catch (e) {
      fail(e);
    }
  };
  const confirmDelete = async () => {
    if (!toDelete) return;
    const id = toDelete.id;
    setToDelete(null);
    try {
      await api.instrumentDelete(id);
      const r = s.prefs.rhythm ?? DEFAULT_RHYTHM;
      const inst = { ...(r.instruments ?? {}) };
      for (const k of Object.keys(inst) as (keyof typeof inst)[]) if (inst[k] === id) delete inst[k];
      s.setPref("rhythm", { ...r, instruments: inst });
      await refresh(null);
    } catch (e) {
      fail(e);
    }
  };
  const useInRhythm = (part: "drums" | "bass" | "ostinato" | "arp") => {
    if (!preset) return;
    const r = s.prefs.rhythm ?? DEFAULT_RHYTHM;
    s.setPref("rhythm", { ...r, instruments: { ...(r.instruments ?? {}), [part]: preset.id } });
    s.setMessage(`${t.igUsed}: ${instrumentName(preset, t)} → ${t[PART_NAME[part]]}`);
  };
  const renderWav = async () => {
    if (!preset) return;
    const path = await save({ title: t.igRender, defaultPath: `${instrumentName(preset, t)}.wav`, filters: [{ name: "WAV", extensions: ["wav"] }] });
    if (!path) return;
    try {
      const r = await api.instrumentRender(path, preset.voice, null, null);
      s.setMessage(`${t.exported}: ${r.path}`);
    } catch (e) {
      fail(e);
    }
  };

  const voice = preset?.voice;
  const groups =
    voice?.engine === "bass" ? BASS_GROUPS : voice?.engine === "drums" ? DRUM_GROUPS : voice?.engine === "tapan" ? TAPAN_GROUPS : voice?.engine === "chord_pad" ? CHORDPAD_GROUPS : voice?.engine === "lead" ? LEAD_GROUPS : SYNTH_GROUPS;
  const howTo =
    voice?.engine === "bass" ? t.howBassDesigner : voice?.engine === "drums" ? t.howDrumDesigner : voice?.engine === "tapan" ? t.howTapanDesigner : voice?.engine === "chord_pad" ? t.howChordPadDesigner : voice?.engine === "lead" ? t.howLeadDesigner : t.howSynthDesigner;
  const addAsRackPart = () => {
    if (!preset) return;
    const kind = RACK_KIND[preset.category];
    if (!kind) return;
    const r = s.prefs.rhythm ?? DEFAULT_RHYTHM;
    const extras = r.extras ?? [];
    if (extras.length >= MAX_EXTRAS) return;
    s.setPref("rhythm", { ...r, extras: [...extras, { ...newExtraPart(kind, extras), instrument: preset.id }] });
    s.setMessage(`${t.igAddedExtra}: ${instrumentName(preset, t)}`);
  };
  const v = (voice ?? {}) as unknown as Record<string, number>;
  const partsFor = (c: string): ("drums" | "bass" | "ostinato" | "arp")[] => (c === "drums" ? ["drums"] : c === "bass" ? ["bass"] : c === "synth" ? ["ostinato", "arp"] : []);

  return (
    <section className="forge">
      <div className="muted small">{t.igIntro}</div>
      <div className="forge-layout">
        <aside className="panel forge-browser">
          <div className="style-row">
            <button className={`ghost small ${cat === null ? "on" : ""}`} onClick={() => setCat(null)}>
              {t.libKindAll}
            </button>
            {CATS.map((c) => (
              <button key={c} className={`ghost small ${cat === c ? "on" : ""}`} onClick={() => setCat(cat === c ? null : c)}>
                {t[CAT_LABEL[c]]}
              </button>
            ))}
          </div>
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t.libSearchHint} aria-label={t.search} />
          <ul className="forge-list">
            {list.map((p) => (
              <li key={p.id}>
                <button className={`ghost grow ${preset?.id === p.id ? "on" : ""}`} onClick={() => void select(p.id)}>
                  <strong>{instrumentName(p, t)}</strong>
                  <span className="muted small">
                    {" "}
                    · {t[CAT_LABEL[p.category as Cat]]}
                    {p.factory ? ` · ${t.igFactory}` : ""}
                  </span>
                </button>
                {!p.factory && (
                  <button className="ghost small" title={t.delete} onClick={() => setToDelete(p)}>
                    🗑
                  </button>
                )}
              </li>
            ))}
          </ul>
        </aside>

        <div className="panel forge-main">
          {!preset || !voice ? (
            <div className="muted">{t.loading}</div>
          ) : (
            <>
              <div className="analysis-row">
                {preset.factory ? (
                  <h2 className="ss-title">{instrumentName(preset, t)}</h2>
                ) : (
                  <input className="forge-name" value={preset.name} maxLength={80} onChange={(e) => setPreset({ ...preset, name: e.target.value })} aria-label={t.presetName} />
                )}
                <span className="badge">{t[CAT_LABEL[preset.category as Cat]]}</span>
                {preset.factory && <span className="badge">{t.igFactory}</span>}
                {dirty && <span className="badge warn">{t.igDirty}</span>}
                {preset.parent_id && <span className="muted small">{t.igForkedFrom}: {instrumentName({ id: preset.parent_id, name: preset.parent_id }, t)}</span>}
              </div>
              <div className="analysis-row">
                {audition ? (
                  <button className="primary" onClick={() => void stopAudition()}>
                    ■ {t.igStopAudition}
                  </button>
                ) : (
                  <button className="primary" onClick={() => void startAudition()} title={t.igAuditionTip}>
                    ▶ {t.igAudition}
                  </button>
                )}
                <button className="ghost" onClick={undo} disabled={history.length === 0}>
                  ↶ {t.undo}
                </button>
                <button className="ghost" onClick={() => setSaving(saving === "as" ? null : "as")}>
                  💾 {t.igSaveAs}
                </button>
                {!preset.factory && (
                  <button className="ghost" onClick={() => void update()} disabled={!dirty}>
                    ✓ {t.igUpdate}
                  </button>
                )}
                <button className="ghost" onClick={() => void renderWav()} title={t.igRenderTip}>
                  ⬇ {t.igRender}
                </button>
                {RACK_KIND[preset.category] && (
                  <button className="small" onClick={addAsRackPart} title={t.igAddAsExtraTip}>
                    {t.igAddAsExtra}
                  </button>
                )}
                <label className="inline small" hidden={partsFor(preset.category).length === 0}>
                  {t.igUseIn}
                  <select value="" onChange={(e) => e.target.value && useInRhythm(e.target.value as never)}>
                    <option value="">…</option>
                    {partsFor(preset.category).map((p) => (
                      <option key={p} value={p}>
                        {t[PART_NAME[p]]}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              {saving === "as" && (
                <form
                  className="analysis-row"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void saveAs();
                  }}
                >
                  <input autoFocus value={newName} onChange={(e) => setNewName(e.target.value)} placeholder={t.presetName} maxLength={80} />
                  <button type="submit" className="small" disabled={!newName.trim()}>
                    {t.save}
                  </button>
                  <button type="button" className="ghost small" onClick={() => setSaving(null)}>
                    {t.cancel}
                  </button>
                </form>
              )}
              <div className="ss-howto small">
                <strong>{t.howToTitle}:</strong> {howTo}
              </div>

              {voice.engine === "bass" && (
                <div className="analysis-row">
                  <label className="inline">
                    {t.igVoice}
                    <select value={voice.voice} onChange={(e) => setVoice({ voice: Number(e.target.value) })}>
                      <option value={0}>{t.bassSub}</option>
                      <option value={1}>{t.bassPluck}</option>
                      <option value={2}>{t.bassReese}</option>
                    </select>
                  </label>
                  <label className="inline" title={t.igWobbleDivTip}>
                    {t.igWobbleDiv}
                    <select value={voice.wobble_div} onChange={(e) => setVoice({ wobble_div: Number(e.target.value) })}>
                      {[0, 1, 2, 4, 8, 16].map((d) => (
                        <option key={d} value={d}>
                          {d === 0 ? t.igWobbleOff : `1/${d}`}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
              )}
              {(voice.engine === "chord_pad" || voice.engine === "lead") && (
                <div className="analysis-row">
                  <label className="inline" title={t.igWaveTip}>
                    {t.igWave}
                    <select value={voice.wave} onChange={(e) => setVoice({ wave: Number(e.target.value) })}>
                      <option value={0}>{t.igWaveSaw}</option>
                      {voice.engine === "lead" && <option value={1}>{t.igWavePulse}</option>}
                      <option value={voice.engine === "lead" ? 2 : 1}>{t.igWaveTri}</option>
                    </select>
                  </label>
                </div>
              )}
              {voice.engine === "synth" && (
                <div className="analysis-row">
                  <label className="inline" title={t.igTimbreTip}>
                    {t.igTimbre}
                    <select value={voice.timbre} onChange={(e) => setVoice({ timbre: Number(e.target.value) })}>
                      <option value={0}>{t.igTimbreSine}</option>
                      <option value={1}>{t.igTimbreTriangle}</option>
                      <option value={2}>{t.igTimbreFlute}</option>
                    </select>
                  </label>
                </div>
              )}
              {groups.map((g) => (
                <div key={g.title} className="forge-group">
                  <h4>{t[g.title]}</h4>
                  <div className="ss-knobs">
                    {g.knobs.map((k) => (
                      <div className="ss-knob" key={k.key} title={t[k.tip]}>
                        <Knob
                          value={toPos(k, v[k.key] ?? k.reset)}
                          min={k.log ? 0 : k.min}
                          max={k.log ? 1 : k.max}
                          step={k.log ? 0.002 : k.step}
                          reset={toPos(k, k.reset)}
                          size={64}
                          label={t[k.label]}
                          text={k.fmt(v[k.key] ?? k.reset)}
                          onChange={(x) => setVoice({ [k.key]: fromPos(k, x) })}
                        />
                        <span className="small">{t[k.label]}</span>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
              {voice.engine === "tapan" && (
                <div className="forge-group">
                  <h4>{t.igLevels}</h4>
                  <div className="ss-knobs">
                    {voice.levels_db.map((l, i) => (
                      <div className="ss-knob narrow" key={i} title={t.igLevelTip}>
                        <Knob value={l} min={-60} max={6} step={0.5} reset={i === 2 ? -10 : 0} size={52} label={t[TAPAN_NAMES[i]]} text={l <= -59.5 ? "off" : formatDb(l)} onChange={(x) => setVoice({ levels_db: voice.levels_db.map((y, j) => (j === i ? x : y)) })} />
                        <span className="small">{t[TAPAN_NAMES[i]]}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              {voice.engine === "drums" && (
                <div className="forge-group">
                  <h4>{t.igLevels}</h4>
                  <div className="ss-knobs">
                    {voice.levels_db.map((l, i) => (
                      <div className="ss-knob narrow" key={i} title={t.igLevelTip}>
                        <Knob value={l} min={-60} max={6} step={0.5} reset={i === 2 ? -8 : 0} size={52} label={t[DRUM_NAMES[i]]} text={l <= -59.5 ? "off" : formatDb(l)} onChange={(x) => setVoice({ levels_db: voice.levels_db.map((y, j) => (j === i ? x : y)) })} />
                        <span className="small">{t[DRUM_NAMES[i]]}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              <label className="forge-notes">
                {t.libNotes}
                <textarea value={preset.notes} rows={2} disabled={preset.factory} onChange={(e) => setPreset({ ...preset, notes: e.target.value })} />
              </label>
              <label className="inline small">
                {t.libTags}
                <input value={preset.tags.join(", ")} disabled={preset.factory} onChange={(e) => setPreset({ ...preset, tags: e.target.value.split(/[,;]/).map((x) => x.trim().toLowerCase()).filter(Boolean) })} />
              </label>
            </>
          )}
        </div>
      </div>
      {toDelete && <ConfirmDialog title={t.delete} body={`${t.libDeleteExplain} ${instrumentName(toDelete, t)}`} confirmLabel={t.delete} cancelLabel={t.cancel} onConfirm={() => void confirmDelete()} onCancel={() => setToDelete(null)} />}
    </section>
  );
}
