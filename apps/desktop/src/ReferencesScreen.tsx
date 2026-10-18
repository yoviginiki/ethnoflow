import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { scaleFromKind } from "./AnalysisPanel";
import { api, describeError, type ReferenceInfo, type ReferenceReport, type Scale, type Section, type WaveformPeaks } from "./api";
import { formatTime } from "./format";
import { messages } from "./i18n";
import { barTicks, ROLE_LABEL } from "./SongMapPanel";
import { useStore } from "./store";
import { Waveform } from "./Waveform";

/** Colour per proposed role (the same family the song map uses). */
const ROLE_COLOR: Record<string, string> = {
  drop: "#e0643c",
  build: "#d8a23a",
  break: "#4f8fd6",
  unknown: "#7a7f8a",
};

const METER_NUMERATORS = [2, 3, 4, 5, 6, 7, 8, 9, 11, 12];
const METER_UNITS = [4, 8, 16];

/**
 * References (P2 Phase B): whole songs in their own library, analysed only on request.
 * Nothing touches the project until the user takes tempo, key or structure explicitly.
 */
export function ReferencesScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project;
  const [list, setList] = useState<ReferenceInfo[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [peaks, setPeaks] = useState<WaveformPeaks | null>(null);
  const [report, setReport] = useState<ReferenceReport | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [playing, setPlaying] = useState(false);
  const [cursor, setCursor] = useState<number | null>(null);
  const [tempoIdx, setTempoIdx] = useState(0);
  const [keyIdx, setKeyIdx] = useState(0);
  const [numerator, setNumerator] = useState<number | null>(project?.timing.numerator ?? null);
  const [unit, setUnit] = useState<number | null>(project?.timing.unit ?? null);
  const [notes, setNotes] = useState("");
  const [name, setName] = useState("");
  const stopTimer = useRef<number | null>(null);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));
  const say = (m: string) => useStore.getState().setMessage(m);

  const current = list.find((r) => r.id === selected) ?? null;

  const refresh = async () => {
    try {
      setList(await api.referenceList());
    } catch (e) {
      fail(e);
    }
  };
  useEffect(() => {
    void refresh();
    return () => {
      void api.samplePreviewStop().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Opening the screen shows the most recent reference instead of an empty pane.
  useEffect(() => {
    if (!selected && list.length > 0) setSelected(list[0].id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [list]);

  useEffect(() => {
    setPeaks(null);
    setReport(null);
    setCursor(null);
    setTempoIdx(0);
    setKeyIdx(0);
    if (!selected) return;
    const r = list.find((x) => x.id === selected);
    setNotes(r?.notes ?? "");
    setName(r?.name ?? "");
    api.referencePeaks(selected).then(setPeaks).catch(fail);
    api.referenceReport(selected).then(setReport).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected]);

  const add = async () => {
    const picked = await open({
      title: t.refAdd,
      multiple: false,
      filters: [{ name: "Audio", extensions: ["wav", "mp3", "flac", "ogg"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    setBusy("add");
    try {
      const info = await api.referenceAdd(picked);
      await refresh();
      setSelected(info.id);
    } catch (e) {
      fail(e);
    } finally {
      setBusy(null);
    }
  };

  const analyze = async () => {
    if (!selected) return;
    setBusy("analyze");
    try {
      const r = await api.referenceAnalyze(selected);
      setReport(r);
      setTempoIdx(0);
      setKeyIdx(0);
      await refresh();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(null);
    }
  };

  const stop = async () => {
    if (stopTimer.current) window.clearTimeout(stopTimer.current);
    stopTimer.current = null;
    setPlaying(false);
    await api.samplePreviewStop().catch(() => undefined);
  };
  const playFrom = async (from: number, seconds: number | null = null) => {
    if (!selected) return;
    await stop();
    try {
      const len = await api.referencePlay(selected, from, seconds, s.prefs.selectedOutput);
      setPlaying(true);
      setCursor(from);
      stopTimer.current = window.setTimeout(() => setPlaying(false), Math.max(0, len * 1000 + 200));
    } catch (e) {
      fail(e);
    }
  };

  const remove = async () => {
    if (!selected || !window.confirm(t.refRemoveConfirm)) return;
    try {
      await stop();
      await api.referenceRemove(selected);
      setSelected(null);
      await refresh();
    } catch (e) {
      fail(e);
    }
  };

  const saveMeta = async () => {
    if (!selected) return;
    try {
      await api.referenceSetMeta(selected, name, notes);
      await refresh();
    } catch (e) {
      fail(e);
    }
  };

  const tempo = report?.tempo[tempoIdx] ?? report?.tempo[0] ?? null;
  const key = report?.key[keyIdx] ?? report?.key[0] ?? null;
  const beatsPerBar = numerator && unit ? (numerator * 4) / unit : null;

  const takeTempo = async () => {
    if (!tempo || !numerator || !unit) return;
    try {
      const keepGrouping = project?.timing.numerator === numerator ? project.timing.grouping : [];
      const pv = await api.setTiming({ bpm: tempo.bpm, numerator, unit, grouping: keepGrouping });
      useStore.getState().setProject(pv);
      say(t.refTookTempo);
    } catch (e) {
      fail(e);
    }
  };
  const takeKey = async () => {
    if (!key) return;
    try {
      const sc: Scale = scaleFromKind({ tonic_pc: key.tonic_pc, degrees_cents: [], kind: key.kind });
      const pv = await api.setScale(sc);
      useStore.getState().setProject(pv);
      say(t.refTookKey);
    } catch (e) {
      fail(e);
    }
  };

  /** Bars a segment spans at the chosen tempo and the project's meter (≥ 1). */
  const barsOf = (seconds: number) => {
    if (!tempo || !beatsPerBar) return null;
    return Math.max(1, Math.round((seconds * tempo.bpm) / 60 / beatsPerBar));
  };

  const copyStructure = async () => {
    if (!project || !report || !tempo) return;
    const map = project.song_map;
    if (!project.timing.bpm) return;
    if (map.sections.length > 0 && !window.confirm(t.refCopyConfirm)) return;
    const bt = barTicks(map, 0);
    const bpb = barTicks(map, 0) / map.ppq; // beats per bar in quarter notes
    let tick = 0;
    const sections: Section[] = report.segments.map((seg, i) => {
      const bars = Math.max(1, Math.round(((seg.end_s - seg.start_s) * tempo.bpm) / 60 / bpb));
      const role = seg.role in ROLE_LABEL ? seg.role : "custom";
      const sec: Section = {
        id: `r${i + 1}`,
        role,
        name: `${t[ROLE_LABEL[role]]} ${i + 1}`,
        start_tick: tick,
        end_tick: tick + bars * bt,
        energy: Math.min(1, Math.max(0, seg.energy)),
        density: Math.min(1, Math.max(0, seg.density)),
        tension: 0.5,
        roles: [],
        transition_out: null,
        locked: false,
        notes: `${current?.name ?? ""}: ${formatTime(seg.start_s)}–${formatTime(seg.end_s)} · ${seg.evidence.join("; ")}`,
      };
      tick += bars * bt;
      return sec;
    });
    try {
      const pv = await api.setSongMap({ ...map, sections });
      useStore.getState().setProject(pv);
      say(t.refCopied);
    } catch (e) {
      fail(e);
    }
  };

  const relLabel = (rel: string) => (rel === "half" ? t.refRelHalf : rel === "double" ? t.refRelDouble : t.refRelPeak);
  const roleLabel = (role: string) => (role in ROLE_LABEL ? t[ROLE_LABEL[role]] : t.refUnknown);

  const duration = current?.duration_s ?? 0;
  const curves = useMemo(() => (report ? [report.energy, report.density, report.low] : null), [report]);

  return (
    <div className="references">
      <aside className="references-list panel">
        <div className="panel-head">
          <strong>{t.refTitle}</strong>
          <button onClick={add} disabled={busy === "add"}>
            {busy === "add" ? t.refAdding : t.refAdd}
          </button>
        </div>
        {list.length === 0 && <p className="muted small">{t.refEmpty}</p>}
        {list.map((r) => (
          <button key={r.id} className={"ghost references-row" + (r.id === selected ? " selected" : "")} onClick={() => setSelected(r.id)}>
            <span className="grow">{r.name}</span>
            <span className="muted small mono-num">{formatTime(r.duration_s)}</span>
            <span className="small">{r.analyzed ? "✓" : "·"}</span>
          </button>
        ))}
        <p className="muted small">{t.refIsolatedHint}</p>
      </aside>

      {current ? (
        <section className="references-main">
          <div className="panel">
            <div className="panel-head">
              <input className="references-name" value={name} onChange={(e) => setName(e.target.value)} onBlur={saveMeta} aria-label={t.refName} />
              <span className="muted small mono-num">
                {formatTime(duration)} · {current.sample_rate} Hz · {current.channels === 1 ? "mono" : "stereo"} · {current.ext.toUpperCase()}
              </span>
              <span className="grow" />
              {playing ? <button onClick={stop}>{t.refStop}</button> : <button onClick={() => playFrom(cursor ?? 0)}>{t.refPlay}</button>}
              <button onClick={analyze} disabled={busy === "analyze"}>
                {busy === "analyze" ? t.refAnalyzing : report ? t.refReanalyze : t.refAnalyze}
              </button>
              <button className="ghost" onClick={remove}>
                {t.refRemove}
              </button>
            </div>
            <div className="muted small path">
              {t.refSource}: {current.source_path}
            </div>
            <Waveform
              peaks={peaks}
              playhead={null}
              cursor={cursor}
              selection={null}
              label={current.name}
              recordingSeconds={null}
              onSeek={(sec) => void playFrom(sec)}
              emptyText={t.refLoading}
            />
            {report && (
              <>
                <StructureLane report={report} duration={duration} roleLabel={roleLabel} onPlay={(a, b) => void playFrom(a, b - a)} />
                <CurveStrip curves={curves!} duration={duration} />
                <div className="muted small">{t.refCurvesHint}</div>
              </>
            )}
            {!report && <p className="muted small">{t.refNotAnalyzed}</p>}
          </div>

          {report && (
            <div className="references-report">
              <div className="panel">
                <div className="panel-head">
                  <strong>{t.refTempo}</strong>
                  <span className="muted small">
                    {report.beats} {t.refBeats}
                  </span>
                </div>
                <div className="tag-row">
                  {report.tempo.map((c, i) => (
                    <button key={i} className={"chip" + (i === tempoIdx ? " selected" : "")} onClick={() => setTempoIdx(i)} title={`${relLabel(c.relation)} · ${Math.round(c.confidence * 100)}%`}>
                      {c.bpm.toFixed(1)} BPM <span className="muted small">{relLabel(c.relation)} {Math.round(c.confidence * 100)}%</span>
                    </button>
                  ))}
                  {report.tempo.length === 0 && <span className="muted small">{t.refNoTempo}</span>}
                </div>
                <p className="muted small">{t.refTempoHint}</p>
                <div className="inline-form">
                  <label className="inline">
                    {t.refMeter}
                    <select value={numerator ?? ""} onChange={(e) => setNumerator(e.target.value ? Number(e.target.value) : null)}>
                      <option value="">–</option>
                      {METER_NUMERATORS.map((n) => (
                        <option key={n} value={n}>
                          {n}
                        </option>
                      ))}
                    </select>
                    /
                    <select value={unit ?? ""} onChange={(e) => setUnit(e.target.value ? Number(e.target.value) : null)}>
                      <option value="">–</option>
                      {METER_UNITS.map((n) => (
                        <option key={n} value={n}>
                          {n}
                        </option>
                      ))}
                    </select>
                  </label>
                  <button onClick={takeTempo} disabled={!project || !tempo || !numerator || !unit} title={!numerator || !unit ? t.refNeedMeter : ""}>
                    {t.refTakeTempo}
                  </button>
                  {!project && <span className="muted small">{t.refNeedProject}</span>}
                </div>
              </div>

              <div className="panel">
                <div className="panel-head">
                  <strong>{t.refKey}</strong>
                </div>
                <div className="tag-row">
                  {report.key.map((h, i) => (
                    <button key={i} className={"chip" + (i === keyIdx ? " selected" : "")} onClick={() => setKeyIdx(i)}>
                      {t.noteNames[h.tonic_pc]} {t.scaleKinds[h.kind]} <span className="muted small">{Math.round(h.probability * 100)}%</span>
                    </button>
                  ))}
                </div>
                <p className="muted small">{t.refKeyHint}</p>
                <button onClick={takeKey} disabled={!project || !key}>
                  {t.refTakeKey}
                </button>
              </div>

              <div className="panel">
                <div className="panel-head">
                  <strong>{t.refStructure}</strong>
                  <span className="muted small">
                    {report.segments.length} {t.refSegments}
                  </span>
                </div>
                <table className="references-table small">
                  <thead>
                    <tr>
                      <th>#</th>
                      <th>{t.refSegment}</th>
                      <th>{t.refRole}</th>
                      <th>{t.refBars}</th>
                      <th>{t.refEnergy}</th>
                      <th>{t.refDensity}</th>
                      <th>{t.refLow}</th>
                      <th>{t.refEvidence}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.segments.map((seg, i) => (
                      <tr key={i} onClick={() => void playFrom(seg.start_s, seg.end_s - seg.start_s)} className="references-seg">
                        <td className="mono-num">{i + 1}</td>
                        <td className="mono-num">
                          {formatTime(seg.start_s)}–{formatTime(seg.end_s)}
                        </td>
                        <td>
                          <span className="class-chip" style={{ background: ROLE_COLOR[seg.role] ?? ROLE_COLOR.unknown }}>
                            {roleLabel(seg.role)} {Math.round(seg.confidence * 100)}%
                          </span>
                        </td>
                        <td className="mono-num">{barsOf(seg.end_s - seg.start_s) ?? "–"}</td>
                        <td>
                          <Bar v={seg.energy} />
                        </td>
                        <td>
                          <Bar v={seg.density} />
                        </td>
                        <td>
                          <Bar v={Math.min(1, seg.low * 2)} />
                        </td>
                        <td className="muted">
                          {seg.evidence.filter((e) => !e.startsWith("repeats segment")).join("; ")}
                          {seg.similar_to !== null && seg.similar_to !== undefined ? ` · ${t.refRepeats} #${seg.similar_to + 1}` : ""}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p className="muted small">{t.refStructureHint}</p>
                <div className="inline-form">
                  <button onClick={copyStructure} disabled={!project || !project.timing.bpm || !tempo || report.segments.length === 0} title={project && !project.timing.bpm ? t.refNeedTiming : ""}>
                    {t.refCopyStructure}
                  </button>
                  {project && !project.timing.bpm && <span className="muted small">{t.refNeedTiming}</span>}
                </div>
                <p className="muted small">{t.refMelodyNote}</p>
              </div>
            </div>
          )}

          <div className="panel">
            <label className="small">
              {t.refNotes}
              <textarea value={notes} onChange={(e) => setNotes(e.target.value)} onBlur={saveMeta} rows={3} />
            </label>
          </div>
        </section>
      ) : (
        <section className="references-main">
          <p className="muted">{t.refPickHint}</p>
        </section>
      )}
    </div>
  );
}

function Bar({ v }: { v: number }) {
  const w = Math.round(Math.min(1, Math.max(0, v)) * 100);
  return (
    <span className="references-bar" title={`${w}%`}>
      <span style={{ width: `${w}%` }} />
    </span>
  );
}

function StructureLane({
  report,
  duration,
  roleLabel,
  onPlay,
}: {
  report: ReferenceReport;
  duration: number;
  roleLabel: (role: string) => string;
  onPlay: (start: number, end: number) => void;
}) {
  if (duration <= 0) return null;
  return (
    <div className="references-lane">
      {report.segments.map((seg, i) => (
        <button
          key={i}
          className="references-lane-seg"
          style={{
            left: `${(seg.start_s / duration) * 100}%`,
            width: `${((seg.end_s - seg.start_s) / duration) * 100}%`,
            background: ROLE_COLOR[seg.role] ?? ROLE_COLOR.unknown,
            opacity: 0.45 + 0.5 * seg.energy,
          }}
          title={`${i + 1} · ${roleLabel(seg.role)} · ${formatTime(seg.start_s)}–${formatTime(seg.end_s)}`}
          onClick={() => onPlay(seg.start_s, seg.end_s)}
        >
          {i + 1}
        </button>
      ))}
      {report.boundaries
        .filter((b) => b.resolution === "fine")
        .map((b, i) => (
          <span key={`b${i}`} className="references-lane-tick" style={{ left: `${(b.seconds / duration) * 100}%`, opacity: 0.3 + 0.7 * b.confidence }} />
        ))}
    </div>
  );
}

function CurveStrip({ curves, duration }: { curves: { step_s: number; values: number[] }[]; duration: number }) {
  const ref = useRef<HTMLCanvasElement | null>(null);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    const w = c.clientWidth;
    const h = 56;
    c.width = Math.max(1, Math.round(w * dpr));
    c.height = Math.round(h * dpr);
    const g = c.getContext("2d");
    if (!g) return;
    g.scale(dpr, dpr);
    g.clearRect(0, 0, w, h);
    const colors = ["#e8e8e8", "#f0a040", "#5aa0ff"];
    curves.forEach((cv, k) => {
      const max = cv.values.reduce((m, v) => Math.max(m, v), 1e-9);
      g.strokeStyle = colors[k];
      g.lineWidth = 1;
      g.beginPath();
      cv.values.forEach((v, i) => {
        const x = duration > 0 ? ((i * cv.step_s) / duration) * w : 0;
        const y = h - (v / max) * (h - 2) - 1;
        if (i === 0) g.moveTo(x, y);
        else g.lineTo(x, y);
      });
      g.stroke();
    });
  }, [curves, duration]);
  return <canvas ref={ref} className="references-curves" style={{ width: "100%", height: 56 }} />;
}
