import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, describeError, IMPORT_EXTENSIONS, type AnalysisView, type Scale, type TempoCandidate, type Timing } from "./api";
import { scaleFromKind } from "./AnalysisPanel";
import { KIND_LABEL } from "./ambientPresets";
import { noteParts, takeName } from "./format";
import { messages, type StringMessageKey } from "./i18n";
import { applySession, collectSession } from "./session";
import { melodyRegister, proposeStarters, type Starter, type StarterFact, type StarterId } from "./starters";
import { useStore } from "./store";

const TITLE: Record<StarterId, StringMessageKey> = { quiet_ambient: "varQuietAmbient", dark_drone: "varDarkDrone", liquid_dnb: "varLiquidDnb", ambient_dnb: "varAmbientDnb", ethno_pulse: "varEthnoPulse" };
const DESC: Record<StarterId, StringMessageKey> = { quiet_ambient: "varQuietAmbientDesc", dark_drone: "varDarkDroneDesc", liquid_dnb: "varLiquidDnbDesc", ambient_dnb: "varAmbientDnbDesc", ethno_pulse: "varEthnoPulseDesc" };

interface Before {
  scale: Scale | null;
  timing: Timing;
  session: ReturnType<typeof collectSession>;
}

/**
 * Start from a melody (F1B.0): import or record a take → Analyse (on request, never
 * automatic) → several complete starting points to audition, save or undo.
 */
export function Start() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const [takeId, setTakeId] = useState<string>(project.loaded_take ?? project.takes[project.takes.length - 1]?.id ?? "");
  const [analysis, setAnalysis] = useState<AnalysisView | null>(null);
  const [tempos, setTempos] = useState<TempoCandidate[]>([]);
  const [chosenKey, setChosenKey] = useState<number>(0); // index into hypotheses; -1 = no key
  const [chosenTempo, setChosenTempo] = useState<number | null>(null);
  const [before, setBefore] = useState<Before | null>(null);
  const [tried, setTried] = useState<StarterId | null>(null);
  const [savedIds, setSavedIds] = useState<StarterId[]>([]);
  const busy = s.pending !== null;
  const fail = (e: unknown) => s.setError(describeError(e));

  useEffect(() => {
    if (!s.ambient) api.ambientGet().then((v) => useStore.getState().setAmbient(v.ambient)).catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    if (!takeId && project.takes.length > 0) setTakeId(project.takes[project.takes.length - 1].id);
  }, [project.takes, takeId]);

  const noteName = (midi: number) => {
    const np = noteParts(midi);
    return `${t.noteNames[np.pc]}${np.octave}`;
  };
  const scaleName = (sc: Scale) => `${t.noteNames[sc.tonic_pc]} ${sc.kind ? t.scaleKinds[sc.kind] : ""}`.trim();

  const onImport = async () => {
    const path = await open({ title: t.importAudio, multiple: false, directory: false, filters: [{ name: t.audioFiles, extensions: IMPORT_EXTENSIONS }] });
    if (!path || Array.isArray(path)) return;
    s.setPending(t.importing);
    try {
      const pv = await api.importAudio(path);
      s.setProject(pv);
      if (pv.loaded_take) setTakeId(pv.loaded_take);
      setAnalysis(null);
      s.setMessage(`${t.importedOk}: ${path}`);
    } catch (e) {
      fail(e);
    } finally {
      s.setPending(null);
    }
  };
  const onRecord = () => {
    s.setPendingRecord(true);
    s.setScreen("play");
  };
  const analyze = async () => {
    if (!takeId) return;
    s.setPending(t.analyzing);
    try {
      const info = await api.loadTake(takeId);
      useStore.getState().setLoaded(info, null);
      const a = await api.analyzeTake();
      useStore.getState().setAnalysis(a);
      setAnalysis(a);
      setChosenKey(a.hypotheses.length > 0 ? 0 : -1);
      const tc = await api.estimateTakeTempo(null, null).catch(() => [] as TempoCandidate[]);
      setTempos(tc);
      setChosenTempo(tc.length > 0 && tc[0].fit >= 0.5 ? Math.round(tc[0].bpm) : null);
      setTried(null);
      setSavedIds([]);
    } catch (e) {
      fail(e);
    } finally {
      s.setPending(null);
    }
  };

  const chosenScale: Scale | null = useMemo(() => {
    if (!analysis) return project.scale;
    if (chosenKey < 0) return null;
    const h = analysis.hypotheses[chosenKey];
    return h ? scaleFromKind({ tonic_pc: h.tonic_pc, degrees_cents: [], kind: h.kind }) : null;
  }, [analysis, chosenKey, project.scale]);
  const register = useMemo(() => (analysis ? melodyRegister(analysis.notes) : null), [analysis]);
  const names = useMemo(() => ({ noise: t.kindNoise, wind: t.kindWind, texture: t.kindTexture, bells: t.kindBells, pluck: t.kindPluck, loop: t.kindLoop }), [t]);
  const starters: Starter[] = useMemo(() => {
    if (!analysis || !s.ambient) return [];
    return proposeStarters({ scale: chosenScale, timing: project.timing, detectedBpm: chosenTempo, register, baseRack: s.ambient, names });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [analysis, chosenScale, chosenTempo, register, s.ambient?.schema_version, project.timing, names]);

  const factText = (f: StarterFact): string => {
    switch (f.kind) {
      case "key":
        return `${t.startKey}: ${scaleName(f.scale)}`;
      case "no_key":
        return `${t.startKey}: ${t.startNoKey}`;
      case "tempo_detected":
        return `${t.startTempo}: ${f.bpm} BPM (${t.startTempoDetected})`;
      case "tempo_project":
        return `${t.startTempo}: ${f.bpm} BPM (${t.startTempoProject})`;
      case "tempo_suggested":
        return `${t.startTempo}: ${f.bpm} BPM (${t.startTempoSuggested})`;
      case "tempo_doubled":
        return `${t.startTempo}: ${f.bpm} BPM (${t.startTempoDoubled})`;
      case "meter_project":
        return `${t.startMeter}: ${f.numerator}/${f.unit}${f.grouping.length ? ` (${f.grouping.join("+")})` : ""}`;
      case "meter_suggested":
        return `${t.startMeter}: ${f.numerator}/${f.unit} (${f.grouping.join("+")}) · ${t.startTempoSuggested}`;
      case "pad_below":
        return `${t.startPadBelow}: ${noteName(f.low)}–${noteName(f.high)}`;
      case "register":
        return `${t.startRegister}: ${noteName(f.low)}–${noteName(f.high)}`;
      case "register_unknown":
        return t.startRegisterUnknown;
    }
  };

  const tryStarter = async (st: Starter) => {
    s.setPending(t.saving);
    try {
      if (!before) setBefore({ scale: project.scale, timing: project.timing, session: collectSession() });
      let pv = project;
      if (JSON.stringify(chosenScale) !== JSON.stringify(project.scale)) pv = await api.setScale(chosenScale);
      if (st.timing && JSON.stringify(st.timing) !== JSON.stringify(pv.timing)) pv = await api.setTiming(st.timing);
      s.setProject(pv);
      await applySession(st.session, true);
      setTried(st.id);
      s.setMessage(`${t.startTrying}: ${t[TITLE[st.id]]}`);
    } catch (e) {
      fail(e);
    } finally {
      s.setPending(null);
    }
  };
  const stopAll = () => {
    for (const n of ["drone", "pad", "ambient", "rhythm"] as const) if (s.layers[n]) s.setLayer(n, false);
  };
  const undo = async () => {
    if (!before) return;
    s.setPending(t.saving);
    try {
      let pv = await api.setScale(before.scale);
      pv = await api.setTiming(before.timing);
      s.setProject(pv);
      await applySession(before.session, true);
      setBefore(null);
      setTried(null);
      s.setMessage(t.startRestored);
    } catch (e) {
      fail(e);
    } finally {
      s.setPending(null);
    }
  };
  const save = async (st: Starter) => {
    try {
      const notes = [t[DESC[st.id]], ...st.facts.map(factText)].join("\n");
      await api.librarySave({ kind: "session", name: `${project.name} · ${t[TITLE[st.id]]}`, tags: ["start", st.id.replace("_", "-")], notes, payload: st.session });
      setSavedIds((x) => [...x, st.id]);
      s.setMessage(`${t.presetSaved}: ${t[TITLE[st.id]]}`);
    } catch (e) {
      fail(e);
    }
  };

  const anyPlaying = s.layers.drone || s.layers.pad || s.layers.ambient || s.layers.rhythm;
  const take = project.takes.find((x) => x.id === takeId) ?? null;

  return (
    <section className="start">
      <div className="muted small">{t.startIntro}</div>

      <div className="panel">
        <h3>{t.startSource}</h3>
        <div className="analysis-row">
          <button onClick={onRecord} disabled={busy}>
            {t.startRecord}
          </button>
          <button onClick={() => void onImport()} disabled={busy}>
            {t.startImport}
          </button>
          <label className="inline">
            {t.startPickTake}
            <select value={takeId} onChange={(e) => setTakeId(e.target.value)} disabled={project.takes.length === 0}>
              {project.takes.length === 0 && <option value="">—</option>}
              {project.takes.map((tk) => (
                <option key={tk.id} value={tk.id}>
                  {takeName(tk, t.take)}
                </option>
              ))}
            </select>
          </label>
        </div>
      </div>

      <div className="panel">
        <h3>{t.startAnalyze}</h3>
        <div className="analysis-row">
          <button className="primary" onClick={() => void analyze()} disabled={busy || !takeId}>
            🔍 {t.analyze}
            {take ? `: ${takeName(take, t.take)}` : ""}
          </button>
          <span className="muted small">{t.startAnalyzeHint}</span>
        </div>
        {analysis && (
          <div className="start-facts">
            <div>
              <strong>{t.startKey}</strong>
              <div className="style-row">
                {analysis.hypotheses.slice(0, 4).map((h, i) => (
                  <button key={`${h.tonic_pc}-${h.kind}`} className={`ghost small ${chosenKey === i ? "on" : ""}`} onClick={() => setChosenKey(i)}>
                    {t.noteNames[h.tonic_pc]} {t.scaleKinds[h.kind]} · {Math.round(h.probability * 100)}%
                  </button>
                ))}
                <button className={`ghost small ${chosenKey < 0 ? "on" : ""}`} onClick={() => setChosenKey(-1)}>
                  {t.startNoKey}
                </button>
              </div>
              {analysis.hypotheses.length === 0 && <div className="muted small">{t.startNoHypotheses}</div>}
            </div>
            <div>
              <strong>{t.startTempo}</strong>
              <div className="style-row">
                {tempos.slice(0, 3).map((c) => (
                  <button key={c.bpm} className={`ghost small ${chosenTempo === Math.round(c.bpm) ? "on" : ""}`} onClick={() => setChosenTempo(Math.round(c.bpm))}>
                    {Math.round(c.bpm)} BPM · {Math.round(c.fit * 100)}%
                  </button>
                ))}
                {project.timing.bpm !== null && (
                  <button className={`ghost small ${chosenTempo === null ? "on" : ""}`} onClick={() => setChosenTempo(null)}>
                    {project.timing.bpm} BPM · {t.startTempoProject}
                  </button>
                )}
                {tempos.length === 0 && project.timing.bpm === null && <span className="muted small">{t.startNoTempo}</span>}
              </div>
            </div>
            <div>
              <strong>{t.startRegister}</strong>
              <div className="small">{register ? `${noteName(register.low)}–${noteName(register.high)} (${t.startMedian} ${noteName(register.median)}, ${register.count} ${t.notesWord})` : t.startRegisterUnknown}</div>
            </div>
          </div>
        )}
      </div>

      <div className="panel">
        <div className="analysis-row">
          <h3>{t.startVariants}</h3>
          {anyPlaying && (
            <button className="ghost small" onClick={stopAll}>
              {t.startStop}
            </button>
          )}
          {before && (
            <button className="ghost small" onClick={() => void undo()} disabled={busy}>
              ↶ {t.startUndo}
            </button>
          )}
        </div>
        {!analysis && <div className="muted small">{t.startNoAnalysis}</div>}
        {analysis && starters.length === 0 && <div className="muted small">{t.loading}</div>}
        <div className="start-grid">
          {starters.map((st) => (
            <div key={st.id} className={`panel start-card ${tried === st.id ? "selected" : ""}`}>
              <h4>{t[TITLE[st.id]]}</h4>
              <div className="small">{t[DESC[st.id]]}</div>
              <ul className="small muted start-fact-list">
                {st.facts.map((f, i) => (
                  <li key={i}>{factText(f)}</li>
                ))}
                <li>
                  {t.startLayersWord}: {(["drone", "pad", "rhythm", "ambient"] as const).filter((n) => st.session.layers[n]).map((n) => t.layerWords[n]).join(", ") || "—"} ·{" "}
                  {st.session.ambient?.layers.map((l) => t[KIND_LABEL[l.source.kind]]).join(", ")}
                </li>
              </ul>
              <div className="style-row">
                <button className="primary small" onClick={() => void tryStarter(st)} disabled={busy}>
                  {tried === st.id && anyPlaying ? t.startTryAgain : t.startTry}
                </button>
                <button className="ghost small" onClick={() => void save(st)} disabled={busy || savedIds.includes(st.id)}>
                  {savedIds.includes(st.id) ? `✓ ${t.presetSaved}` : t.startSaveVariant}
                </button>
              </div>
            </div>
          ))}
        </div>
        {tried && (
          <div className="muted small">
            {t.startNextHint}
            <button className="ghost small" onClick={() => s.setScreen("compose")}>
              {t.areaAccomp}
            </button>
            <button className="ghost small" onClick={() => s.setScreen("soundscapes")}>
              {t.areaSoundscapes}
            </button>
            <button className="ghost small" onClick={() => s.setScreen("rhythm")}>
              {t.areaRhythm}
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
