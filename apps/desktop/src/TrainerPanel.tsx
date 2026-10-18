import { useEffect, useRef, useState } from "react";
import { api, describeError, LEVELS, type Level, type Note, type TrainerLogEntry, type TrainerReport } from "./api";
import { hzToMidi, noteParts } from "./format";
import { messages } from "./i18n";
import { IntentBadge } from "./IntentPanel";
import { useStore } from "./store";

type Mode = "listen" | "wait" | "rhythm" | "drill";

interface Props {
  /** Expected notes (the loaded take's current notes). */
  notes: Note[];
  /** Live microphone reading (from the tuner / input monitor). */
  hz: number | null;
  confidence: number;
  a4: number;
  /** Synth playback position while "Play the notes" runs, or null. */
  notesPosition: number | null;
  onPlayNotes: () => Promise<void>;
  onStopNotes: () => Promise<void>;
  /** Move the falling-notes view to a time (Wait mode drives it). */
  onPosition: (seconds: number) => void;
  /** Called with the index of the note being waited for (lit), or null. */
  onTarget: (index: number | null, wrong: boolean) => void;
  /** Drill range start (the studio cursor) and the length of one bar when the project has a tempo. */
  cursor: number;
  barSeconds: number | null;
  takeId: string | null;
}

const TICK_MS = 50;
const HOLD_TICKS = 3; // ≈150 ms of the right pitch advances in Wait mode
const CHORD_WINDOW_S = 0.03;
const SEGMENT_CENTS = 80;
const DRILL_START = 0.5;
const DRILL_STEP = 0.1;
const DRILL_CLEAN_PASSES = 3;
const DRILL_CLEAN_RATIO = 0.9;
const DRILL_BARS = 2;
const DRILL_FALLBACK_S = 4;
const DRILL_PAUSE_MS = 1200;

/** Trainer modes driven by the microphone pitch (F2.8.2); evaluation is in Rust. */
export function TrainerPanel({ notes, hz, confidence, a4, notesPosition, onPlayNotes, onStopNotes, onPosition, onTarget, cursor, barSeconds, takeId }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [mode, setMode] = useState<Mode>("wait");
  const [level, setLevel] = useState<Level>("beginner");
  const [running, setRunning] = useState(false);
  const [group, setGroup] = useState(0);
  const [wrong, setWrong] = useState(false);
  const [report, setReport] = useState<TrainerReport | null>(null);
  const [done, setDone] = useState(false);
  const [measuring, setMeasuring] = useState(false);
  const [latencyNote, setLatencyNote] = useState<string | null>(null);
  const [history, setHistory] = useState<TrainerLogEntry[] | null>(null);
  // Drill state: tempo factor, clean passes at this tempo, pass log.
  const [drillFactor, setDrillFactor] = useState(DRILL_START);
  const [drillClean, setDrillClean] = useState(0);
  const [drillPasses, setDrillPasses] = useState<{ factor: number; ratio: number }[]>([]);
  const drillTimer = useRef<number | null>(null);
  const hold = useRef(0);
  const played = useRef<Note[]>([]);
  const current = useRef<{ start: number; midi: number; cents: number[] } | null>(null);
  const unvoiced = useRef(0);
  const latencyMs = s.prefs.latencyMs ?? 0;

  // Expected notes grouped by (near-)simultaneous start: a chord counts as one step.
  const groups: Note[][] = [];
  const sorted = [...notes].sort((a, b) => a.start - b.start);
  for (const n of sorted) {
    const g = groups[groups.length - 1];
    if (g && n.start - g[0].start <= CHORD_WINDOW_S) g.push(n);
    else groups.push([n]);
  }
  const tol = level === "beginner" ? 30 : level === "intermediate" ? 20 : 10;
  // Input: a held MIDI key wins over the microphone when a port is open.
  const midiInputOn = s.prefs.midiInput !== null;
  const midiHeld = s.midiHeld;
  const midiFrac = midiInputOn
    ? midiHeld.length
      ? midiHeld[midiHeld.length - 1]
      : null
    : hz !== null && confidence >= 0.6
      ? hzToMidi(hz, a4)
      : null;

  // Drill range: from the first note at/after the cursor, two bars (or 4 s) long.
  const drillStart = sorted.find((n) => n.start >= cursor - 1e-6)?.start ?? sorted[0]?.start ?? 0;
  const drillLen = barSeconds ? barSeconds * DRILL_BARS : DRILL_FALLBACK_S;
  const drillEnd = drillStart + drillLen;
  const drillNotes = sorted.filter((n) => n.start >= drillStart - 1e-6 && n.start < drillEnd);

  // Academy play-along (F6): a finished Wait run or a Rhythm report counts for the lesson.
  const reportAcademy = (score: number) => {
    const id = useStore.getState().academyPlayAlong;
    if (!id) return;
    useStore.getState().setAcademyPlayAlong(null);
    void api.academyRecord(id, Math.max(0, Math.min(1, score)), score >= 0.7).catch((e) => s.setError(describeError(e)));
  };

  const logHistory = (r: TrainerReport, m: Mode, tempoPercent: number) => {
    const e: TrainerLogEntry = {
      at_ms: 0,
      take_id: takeId,
      mode: m,
      level,
      tempo_percent: tempoPercent,
      expected: r.summary.expected,
      hits: r.summary.hits,
      wrong_pitch: r.summary.wrong_pitch,
      early: r.summary.early,
      late: r.summary.late,
      missed: r.summary.missed,
      extra: r.summary.extra,
      mean_abs_cents: r.summary.mean_abs_cents,
      mean_abs_ms: r.summary.mean_abs_ms,
      latency_ms: latencyMs,
    };
    void api
      .academyTrainerLog(e)
      .then(() => api.academyTrainerHistory(5))
      .then(setHistory)
      .catch(() => undefined);
  };

  useEffect(() => {
    void api.academyTrainerHistory(5).then(setHistory).catch(() => setHistory(null));
  }, []);

  const stop = async () => {
    setRunning(false);
    onTarget(null, false);
    if (drillTimer.current !== null) {
      window.clearTimeout(drillTimer.current);
      drillTimer.current = null;
    }
    if (mode === "rhythm" || mode === "drill") await onStopNotes();
  };

  const resetCapture = () => {
    hold.current = 0;
    played.current = [];
    current.current = null;
    unvoiced.current = 0;
  };

  const startDrillPass = async (factor: number) => {
    resetCapture();
    setReport(null);
    const st = useStore.getState();
    try {
      const ps = await api.playNotesDrill(drillStart, drillEnd, factor, st.prefs.selectedOutput, st.prefs.voice);
      st.setNotesPlayer(ps);
    } catch (e) {
      s.setError(describeError(e));
      setRunning(false);
    }
  };

  const start = async () => {
    setReport(null);
    setDone(false);
    setWrong(false);
    setGroup(0);
    resetCapture();
    if (!s.prefs.tuner && !midiInputOn) s.setPref("tuner", true);
    setRunning(true);
    if (mode === "wait" && groups.length) {
      onPosition(groups[0][0].start);
      onTarget(sorted.indexOf(groups[0][0]), false);
    }
    if (mode === "rhythm" || mode === "listen") await onPlayNotes();
    if (mode === "drill") {
      setDrillFactor(DRILL_START);
      setDrillClean(0);
      setDrillPasses([]);
      onPosition(drillStart);
      await startDrillPass(DRILL_START);
    }
  };

  // Wait mode: advance when the lit note is held; flash on a wrong one.
  useEffect(() => {
    if (!running || mode !== "wait") return;
    const id = setTimeout(() => {
      const g = groups[group];
      if (!g) return;
      if (midiFrac === null) {
        hold.current = 0;
        setWrong(false);
        return;
      }
      const ok = g.some((n) => Math.abs((midiFrac - (n.midi + n.cents / 100)) * 100) <= tol);
      if (ok) {
        setWrong(false);
        if (++hold.current >= HOLD_TICKS) {
          hold.current = 0;
          const next = group + 1;
          if (next >= groups.length) {
            setDone(true);
            void stop();
            reportAcademy(1);
          } else {
            setGroup(next);
            onPosition(groups[next][0].start);
            onTarget(sorted.indexOf(groups[next][0]), false);
          }
        }
      } else {
        hold.current = 0;
        setWrong(true);
        onTarget(sorted.indexOf(g[0]), true);
      }
    }, TICK_MS);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running, mode, group, midiFrac, tol]);

  // Rhythm and drill modes: segment the microphone pitch into played notes on the synth
  // clock. In a drill the clock is slowed, so positions are mapped back to the take.
  useEffect(() => {
    if (!running || (mode !== "rhythm" && mode !== "drill")) return;
    const toTake = (pos: number) => (mode === "drill" ? drillStart + pos * drillFactor : pos);
    if (notesPosition === null) {
      // Playback finished: close the last note and evaluate.
      const c = current.current;
      const expected = mode === "drill" ? drillNotes : sorted;
      if (c) {
        played.current.push(toNote(c, expected.length ? Math.max(...expected.map((n) => n.end)) : c.start + 0.1));
        current.current = null;
      }
      if (played.current.length === 0 && mode === "drill" && expected.length === 0) {
        setRunning(false);
        return;
      }
      const tolMs = mode === "drill" ? latencyMs * drillFactor : latencyMs;
      void api
        .evaluatePerformance(expected, played.current, level, tolMs)
        .then((r) => {
          setReport(r);
          const ratio = r.summary.expected ? r.summary.hits / r.summary.expected : 0;
          if (mode === "rhythm") {
            setRunning(false);
            onTarget(null, false);
            reportAcademy(ratio);
            logHistory(r, "rhythm", 100);
            return;
          }
          // Drill: count clean passes, step the tempo up, queue the next pass.
          logHistory(r, "drill", Math.round(drillFactor * 100));
          const clean = ratio >= DRILL_CLEAN_RATIO ? drillClean + 1 : 0;
          setDrillPasses((p) => [...p, { factor: drillFactor, ratio }].slice(-12));
          let next = drillFactor;
          if (clean >= DRILL_CLEAN_PASSES) {
            next = Math.min(1, Math.round((drillFactor + DRILL_STEP) * 100) / 100);
            setDrillClean(0);
            if (drillFactor >= 1) {
              setDone(true);
              setRunning(false);
              onTarget(null, false);
              reportAcademy(1);
              return;
            }
          } else {
            setDrillClean(clean);
          }
          setDrillFactor(next);
          drillTimer.current = window.setTimeout(() => {
            drillTimer.current = null;
            if (useStore.getState().mode === "idle") void startDrillPass(next);
          }, DRILL_PAUSE_MS);
        })
        .catch((e) => {
          setRunning(false);
          s.setError(describeError(e));
        });
      return;
    }
    const tpos = toTake(notesPosition);
    if (midiFrac === null) {
      if (++unvoiced.current >= 2 && current.current) {
        played.current.push(toNote(current.current, tpos));
        current.current = null;
      }
      return;
    }
    unvoiced.current = 0;
    const c = current.current;
    if (!c || Math.abs((midiFrac - c.midi) * 100) > SEGMENT_CENTS) {
      if (c) played.current.push(toNote(c, tpos));
      current.current = { start: tpos, midi: Math.round(midiFrac), cents: [(midiFrac - Math.round(midiFrac)) * 100] };
    } else {
      c.cents.push((midiFrac - c.midi) * 100);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running, mode, notesPosition, midiFrac]);

  useEffect(() => {
    if (running && mode === "listen" && notesPosition === null) setRunning(false);
  }, [running, mode, notesPosition]);

  useEffect(
    () => () => {
      if (drillTimer.current !== null) window.clearTimeout(drillTimer.current);
    },
    [],
  );

  const measureLatency = async () => {
    setMeasuring(true);
    setLatencyNote(null);
    const p = useStore.getState().prefs;
    try {
      const r = await api.measureLatency(p.selectedInput, p.selectedOutput, p.sampleRate);
      s.setPref("latencyMs", Math.round(r.ms * 10) / 10);
      setLatencyNote(`${r.ms.toFixed(0)} ms ±${(r.spread_ms / 2).toFixed(0)} (${r.heard}/${r.clicks})`);
    } catch (e) {
      s.setError(describeError(e));
    } finally {
      setMeasuring(false);
    }
  };

  const name = (n: Note) => {
    const p = noteParts(n.midi);
    return `${t.noteNames[p.pc]}${p.octave}`;
  };
  const verdictText = (r: TrainerReport["per_note"][number]) => {
    const v = r.verdict;
    switch (v.kind) {
      case "hit":
        return `✓ ${t.hits} (${v.cents >= 0 ? "+" : ""}${v.cents.toFixed(0)} c, ${v.ms >= 0 ? "+" : ""}${v.ms.toFixed(0)} ms)`;
      case "wrong_pitch":
        return `✗ ${t.wrongPitch} (${v.cents >= 0 ? "+" : ""}${v.cents.toFixed(0)} c)`;
      case "early":
        return `◂ ${t.earlyNotes} (${v.ms.toFixed(0)} ms)`;
      case "late":
        return `▸ ${t.lateNotes} (+${v.ms.toFixed(0)} ms)`;
      default:
        return `— ${t.missedNotes}`;
    }
  };
  const reportNotes = mode === "drill" ? drillNotes : sorted;
  const modeLabel = (m: Mode) => (m === "listen" ? t.modeListen : m === "wait" ? t.modeWait : m === "rhythm" ? t.modeRhythm : t.modeDrill);
  const hint = mode === "wait" ? t.waitHint : mode === "rhythm" ? t.rhythmHint : mode === "drill" ? t.drillHint : t.listenHint;

  return (
    <details className="panel trainer" open>
      <summary>
        {t.trainer} · {midiInputOn ? t.inputMidi : t.inputMic} · <IntentBadge />
      </summary>
      <div className="analysis-row">
        {(["listen", "wait", "rhythm", "drill"] as Mode[]).map((m) => (
          <label key={m} className="inline">
            <input type="radio" name="trainerMode" checked={mode === m} disabled={running} onChange={() => setMode(m)} />
            {modeLabel(m)}
          </label>
        ))}
        <label className="inline">
          {t.trainerLevel}
          <select value={level} disabled={running} onChange={(e) => setLevel(e.target.value as Level)}>
            {LEVELS.map((l) => (
              <option key={l} value={l}>
                {t.levels[l]}
              </option>
            ))}
          </select>
        </label>
        <span className="spacer" />
        {running ? (
          <button className="danger" onClick={() => void stop()}>
            ■ {t.trainerStop}
          </button>
        ) : (
          <button onClick={() => void start()} disabled={notes.length === 0 || s.mode !== "idle" || (mode === "drill" && drillNotes.length === 0)}>
            ▶ {t.trainerStart}
          </button>
        )}
      </div>
      <div className="analysis-row small">
        <span className="muted" title={t.latencyHint}>
          {t.latencyLabel}: <span className="mono-num">{s.prefs.latencyMs !== null ? `${s.prefs.latencyMs.toFixed(0)} ms` : t.latencyUnknown}</span>
          {latencyNote && <span className="mono-num"> ({latencyNote})</span>}
        </span>
        <button className="ghost small" disabled={measuring || running || s.mode !== "idle"} onClick={() => void measureLatency()}>
          {measuring ? t.latencyMeasuring : t.latencyMeasure}
        </button>
        {s.prefs.latencyMs !== null && (
          <button className="ghost small" disabled={measuring || running} onClick={() => s.setPref("latencyMs", null)} title={t.latencyClear}>
            ✕
          </button>
        )}
      </div>
      <div className="muted small">
        {hint} {!s.prefs.tuner && !midiInputOn && t.trainerNeedsTuner}
      </div>
      {mode === "drill" && (
        <div className="analysis-row small">
          <span className="mono-num">
            {t.drillRange}: {drillStart.toFixed(1)}–{drillEnd.toFixed(1)} s · {drillNotes.length} {t.waitProgress}
          </span>
          <span className="mono-num">
            {t.drillTempo}: {Math.round(drillFactor * 100)} % · {t.drillClean} {drillClean}/{DRILL_CLEAN_PASSES}
          </span>
          {drillPasses.length > 0 && (
            <span className="drill-passes" aria-hidden="true">
              {drillPasses.map((p, i) => (
                <span key={i} className={`clap-dot ${p.ratio >= DRILL_CLEAN_RATIO ? "hit" : "miss"}`} title={`${Math.round(p.factor * 100)} %: ${(p.ratio * 100).toFixed(0)} %`} />
              ))}
            </span>
          )}
        </div>
      )}
      {running && mode === "wait" && groups[group] && (
        <div className={`banner ${wrong ? "error" : "ok"}`} role="status" aria-live="polite">
          {t.waitProgress} {group + 1}/{groups.length}: {groups[group].map(name).join(" + ")}
        </div>
      )}
      {done && <div className="banner ok">{mode === "drill" ? t.drillDone : t.waitDone}</div>}
      {report && (
        <div className="trainer-report">
          <b>{t.report}</b>
          <div className="small">
            {t.hits} {report.summary.hits}/{report.summary.expected} · {t.wrongPitch} {report.summary.wrong_pitch} · {t.earlyNotes} {report.summary.early} ·{" "}
            {t.lateNotes} {report.summary.late} · {t.missedNotes} {report.summary.missed} · {t.extraNotes} {report.summary.extra} · {t.meanCents}{" "}
            {report.summary.mean_abs_cents.toFixed(0)} c · {t.meanMs} {report.summary.mean_abs_ms.toFixed(0)} ms
          </div>
          <ul className="small">
            {report.per_note.map((r) => (
              <li key={r.index} className={r.verdict.kind === "hit" ? "ok" : r.verdict.kind === "missed" || r.verdict.kind === "wrong_pitch" ? "bad" : ""}>
                {reportNotes[r.index] ? name(reportNotes[r.index]) : r.index} @ {reportNotes[r.index]?.start.toFixed(2)} s: {verdictText(r)}
              </li>
            ))}
          </ul>
          <div className="muted small">{t.nextDrill}</div>
        </div>
      )}
      {history && history.length > 0 && (
        <details className="small">
          <summary>{t.trainerHistory}</summary>
          <ul className="small trainer-history">
            {history.map((h, i) => (
              <li key={i} className="mono-num">
                {new Date(h.at_ms).toLocaleString()} · {h.mode} {h.tempo_percent !== 100 ? `${h.tempo_percent} %` : ""} · {t.levels[h.level as Level] ?? h.level} · {t.hits} {h.hits}/{h.expected} ·{" "}
                {h.mean_abs_cents.toFixed(0)} c · {h.mean_abs_ms.toFixed(0)} ms
              </li>
            ))}
          </ul>
        </details>
      )}
    </details>
  );
}

function toNote(c: { start: number; midi: number; cents: number[] }, end: number): Note {
  const sortedC = [...c.cents].sort((a, b) => a - b);
  return {
    start: c.start,
    end: Math.max(end, c.start + 0.02),
    midi: c.midi,
    cents: sortedC.length ? sortedC[Math.floor(sortedC.length / 2)] : 0,
    confidence: 1,
    velocity: 0.7,
    bend: [],
  };
}
