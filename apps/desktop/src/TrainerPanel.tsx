import { useEffect, useRef, useState } from "react";
import { api, describeError, LEVELS, type Level, type Note, type TrainerReport } from "./api";
import { hzToMidi, noteParts } from "./format";
import { messages } from "./i18n";
import { IntentBadge } from "./IntentPanel";
import { useStore } from "./store";

type Mode = "listen" | "wait" | "rhythm";

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
}

const TICK_MS = 50;
const HOLD_TICKS = 3; // ≈150 ms of the right pitch advances in Wait mode
const CHORD_WINDOW_S = 0.03;
const SEGMENT_CENTS = 80;

/** Trainer modes driven by the microphone pitch (F2.8.2); evaluation is in Rust. */
export function TrainerPanel({ notes, hz, confidence, a4, notesPosition, onPlayNotes, onStopNotes, onPosition, onTarget }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [mode, setMode] = useState<Mode>("wait");
  const [level, setLevel] = useState<Level>("beginner");
  const [running, setRunning] = useState(false);
  const [group, setGroup] = useState(0);
  const [wrong, setWrong] = useState(false);
  const [report, setReport] = useState<TrainerReport | null>(null);
  const [done, setDone] = useState(false);
  const hold = useRef(0);
  const played = useRef<Note[]>([]);
  const current = useRef<{ start: number; midi: number; cents: number[] } | null>(null);
  const unvoiced = useRef(0);

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

  const stop = async () => {
    setRunning(false);
    onTarget(null, false);
    if (mode === "rhythm") await onStopNotes();
  };

  const start = async () => {
    setReport(null);
    setDone(false);
    setWrong(false);
    setGroup(0);
    hold.current = 0;
    played.current = [];
    current.current = null;
    if (!s.prefs.tuner && !midiInputOn) s.setPref("tuner", true);
    setRunning(true);
    if (mode === "wait" && groups.length) {
      onPosition(groups[0][0].start);
      onTarget(sorted.indexOf(groups[0][0]), false);
    }
    if (mode === "rhythm" || mode === "listen") await onPlayNotes();
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

  // Rhythm mode: segment the microphone pitch into played notes on the synth clock.
  useEffect(() => {
    if (!running || mode !== "rhythm") return;
    if (notesPosition === null) {
      // Playback finished: close the last note and evaluate.
      const c = current.current;
      if (c) {
        played.current.push(toNote(c, notes.length ? Math.max(...notes.map((n) => n.end)) : c.start + 0.1));
        current.current = null;
      }
      setRunning(false);
      onTarget(null, false);
      void api
        .evaluatePerformance(sorted, played.current, level, 0)
        .then(setReport)
        .catch((e) => s.setError(describeError(e)));
      return;
    }
    const tpos = notesPosition;
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

  return (
    <details className="panel trainer" open>
      <summary>
        {t.trainer} · {midiInputOn ? t.inputMidi : t.inputMic} · <IntentBadge />
      </summary>
      <div className="analysis-row">
        {(["listen", "wait", "rhythm"] as Mode[]).map((m) => (
          <label key={m} className="inline">
            <input type="radio" name="trainerMode" checked={mode === m} disabled={running} onChange={() => setMode(m)} />
            {m === "listen" ? t.modeListen : m === "wait" ? t.modeWait : t.modeRhythm}
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
          <button onClick={() => void start()} disabled={notes.length === 0 || s.mode !== "idle"}>
            ▶ {t.trainerStart}
          </button>
        )}
      </div>
      <div className="muted small">
        {mode === "wait" ? t.waitHint : mode === "rhythm" ? t.rhythmHint : t.listenHint} {!s.prefs.tuner && !midiInputOn && t.trainerNeedsTuner}
      </div>
      {running && mode === "wait" && groups[group] && (
        <div className={`banner ${wrong ? "error" : "ok"}`} role="status" aria-live="polite">
          {t.waitProgress} {group + 1}/{groups.length}: {groups[group].map(name).join(" + ")}
        </div>
      )}
      {done && <div className="banner ok">{t.waitDone}</div>}
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
                {sorted[r.index] ? name(sorted[r.index]) : r.index} @ {sorted[r.index]?.start.toFixed(2)} s: {verdictText(r)}
              </li>
            ))}
          </ul>
          <div className="muted small">{t.nextDrill}</div>
        </div>
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
