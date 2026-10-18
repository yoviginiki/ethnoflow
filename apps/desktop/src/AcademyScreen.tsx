import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import kbSource from "../../../docs/kaval-baza-znania.md?raw";
import { startGuided, stopGuided } from "./academyGuided";
import { api, describeError, LEVELS, type AcademyInstance, type AcademyView, type EarRound, type InstanceView, type Level, type LessonView, type LText, type Note, type PlayNote, type RepeatScore, type RhythmScore } from "./api";
import { FingeringDiagram } from "./FingeringDiagram";
import { chartFor, KAVAL_D, nearestFingered } from "./fingerings";
import { hzToMidi, noteParts } from "./format";
import { messages, type Lang } from "./i18n";
import { blockText, parseMarkdown, sections, type Block, type Section } from "./md";
import { BlockView } from "./MdView";
import { useStore } from "./store";

type Tab = "levels" | "kaval" | "knowledge" | "fingerings";

const MIC_POLL_MS = 50;
const HOLD_TICKS = 3;
const TOL_CENTS: Record<Level, number> = { beginner: 30, intermediate: 20, advanced: 10 };

function useT() {
  const lang = useStore((s) => s.prefs.lang);
  return { t: messages[lang], lang };
}

const lt = (x: LText | null | undefined, lang: Lang) => (x ? x[lang] : "");

/** Microphone pitch/level while `active`: starts the input monitor and polls it. */
function useMic(active: boolean) {
  const [m, setM] = useState<{ hz: number | null; confidence: number; peak: number } | null>(null);
  useEffect(() => {
    if (!active) {
      setM(null);
      void api.stopMonitor().catch(() => undefined);
      return;
    }
    let cancelled = false;
    const p = useStore.getState().prefs;
    void api.startMonitor(p.selectedInput, p.sampleRate).catch((e) => useStore.getState().setError(describeError(e)));
    const tick = async () => {
      if (cancelled) return;
      try {
        const st = await api.monitorStatus();
        if (!cancelled) setM(st ? { hz: st.pitch_hz, confidence: st.pitch_confidence, peak: st.peak } : null);
      } catch {
        // monitor not running yet
      }
      if (!cancelled) setTimeout(tick, MIC_POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
      void api.stopMonitor().catch(() => undefined);
    };
  }, [active]);
  return m;
}

/** Play notes through the synth; `playing` follows the player until it finishes. */
function usePlayer() {
  const [playing, setPlaying] = useState(false);
  const [position, setPosition] = useState<number | null>(null);
  const token = useRef(0);
  const play = useCallback(async (notes: PlayNote[]) => {
    const my = ++token.current;
    const st = useStore.getState();
    try {
      await api.academyPlay(notes, st.prefs.selectedOutput, "flute");
    } catch (e) {
      st.setError(describeError(e));
      return;
    }
    setPlaying(true);
    const poll = async () => {
      if (token.current !== my) return;
      const s = await api.notesPlaybackStatus().catch(() => null);
      if (token.current !== my) return;
      if (!s || s.finished) {
        setPlaying(false);
        setPosition(null);
        return;
      }
      setPosition(s.position_seconds);
      setTimeout(poll, 40);
    };
    void poll();
  }, []);
  const stop = useCallback(async () => {
    token.current++;
    setPlaying(false);
    setPosition(null);
    await api.stopNotes().catch(() => undefined);
  }, []);
  useEffect(() => () => void api.stopNotes().catch(() => undefined), []);
  return { playing, position, play, stop };
}

function choiceLabel(key: string, lang: Lang): string {
  const t = messages[lang];
  const [kind, ...rest] = key.split(":");
  const v = rest.join(":");
  if (kind === "interval") return t.academyChoiceInterval[Number(v)] ?? v;
  if (kind === "chord") return (t.academyChoiceChord as Record<string, string>)[v] ?? v;
  if (kind === "mode") return (t.scaleKinds as Record<string, string>)[v] ?? v;
  if (kind === "meter") {
    const [m, g] = v.split(":");
    return g ? `${m} (${g})` : m;
  }
  return key;
}

function noteName(midi: number, lang: Lang) {
  const p = noteParts(midi);
  return `${messages[lang].noteNames[p.pc]}${p.octave}`;
}

// ---------------------------------------------------------------- exercises

interface ExProps {
  lesson: LessonView;
  inst: InstanceView;
  level: Level;
  onResult: (score: number) => void;
  onAgain: () => void;
}

function HoldToneEx({ inst, level, onResult }: ExProps) {
  const { t, lang } = useT();
  const i = inst.instance as Extract<AcademyInstance, { kind: "hold_tone" }>;
  const [on, setOn] = useState(false);
  const mic = useMic(on);
  const a4 = useStore((s) => s.project?.a4_hz ?? 440);
  const chart = chartFor(useStore((s) => s.project?.instrument?.instrument));
  const [ref, setRef] = useState<number | null>(i.target_midi);
  const [secs, setSecs] = useState(0);
  const cents = useRef<number[]>([]);
  const lastT = useRef<number | null>(null);
  const finished = useRef(false);
  const tol = TOL_CENTS[level];
  const midiFrac = mic && mic.hz !== null && mic.confidence >= 0.5 ? hzToMidi(mic.hz, a4) : null;

  useEffect(() => {
    if (!on || finished.current) return;
    const now = performance.now() / 1000;
    let target = ref;
    if (target === null && midiFrac !== null) {
      target = Math.round(midiFrac);
      setRef(target);
    }
    const c = midiFrac !== null && target !== null ? (midiFrac - target) * 100 : null;
    if (c !== null && Math.abs(c) <= tol) {
      cents.current.push(c);
      if (lastT.current !== null) {
        const add = Math.min(0.2, now - lastT.current);
        const next = secs + add;
        setSecs(next);
        if (next >= i.seconds) {
          finished.current = true;
          const arr = cents.current;
          const mean = arr.reduce((a, b) => a + b, 0) / Math.max(1, arr.length);
          const sigma = Math.sqrt(arr.reduce((a, b) => a + (b - mean) ** 2, 0) / Math.max(1, arr.length));
          const sigmaOk = i.max_sigma_cents === null || sigma <= i.max_sigma_cents;
          setOn(false);
          onResult(sigmaOk ? 1 : 0.5);
        }
      }
    }
    lastT.current = now;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [midiFrac, on]);

  const arr = cents.current;
  const mean = arr.reduce((a, b) => a + b, 0) / Math.max(1, arr.length);
  const sigma = Math.sqrt(arr.reduce((a, b) => a + (b - mean) ** 2, 0) / Math.max(1, arr.length));
  return (
    <div className="academy-ex">
      <div className="muted small">{t.academyMicNeeded}</div>
      <div className="analysis-row">
        <b>
          {t.academyHoldTarget}: {ref !== null ? noteName(ref, lang) : t.academyHoldAny}
        </b>
        <span className="mono-num">
          {secs.toFixed(1)} / {i.seconds} {t.academyHoldInTol}
        </span>
        {i.max_sigma_cents !== null && (
          <span className="mono-num small muted">
            {t.academyHoldStability} {arr.length ? sigma.toFixed(0) : "–"} c (≤ {i.max_sigma_cents})
          </span>
        )}
        <span className="spacer" />
        {!on ? (
          <button
            onClick={() => {
              finished.current = false;
              cents.current = [];
              lastT.current = null;
              setSecs(0);
              setRef(i.target_midi);
              setOn(true);
            }}
          >
            ▶ {t.academyStart}
          </button>
        ) : (
          <button className="danger" onClick={() => setOn(false)}>
            ■ {t.stop}
          </button>
        )}
      </div>
      <div className="meter">
        <div className="meter-fill" style={{ width: `${Math.min(100, (secs / i.seconds) * 100)}%` }} />
      </div>
      {on && (
        <div className="small mono-num">
          {midiFrac !== null ? `${noteName(Math.round(midiFrac), lang)} ${((midiFrac - Math.round(midiFrac)) * 100).toFixed(0)} c` : t.tunerSilent}
        </div>
      )}
      {chart && ref !== null && <FingeringDiagram chart={chart} midi={ref} />}
    </div>
  );
}

function SightReadingEx({ inst, level, onResult }: ExProps) {
  const { t, lang } = useT();
  const i = inst.instance as Extract<AcademyInstance, { kind: "sight_reading" }>;
  const [on, setOn] = useState(false);
  const [idx, setIdx] = useState(0);
  const [shownAt, setShownAt] = useState(0);
  const [scores, setScores] = useState<number[]>([]);
  const [last, setLast] = useState<number | null>(null);
  const hold = useRef(0);
  const mic = useMic(on);
  const a4 = useStore((s) => s.project?.a4_hz ?? 440);
  const chart = chartFor(useStore((s) => s.project?.instrument?.instrument));
  const tol = TOL_CENTS[level];
  const midiFrac = mic && mic.hz !== null && mic.confidence >= 0.6 ? hzToMidi(mic.hz, a4) : null;
  const target = i.targets[idx];

  useEffect(() => {
    if (!on || target === undefined) return;
    const ok = midiFrac !== null && Math.abs((midiFrac - target) * 100) <= tol;
    if (!ok) {
      hold.current = 0;
      return;
    }
    if (++hold.current >= HOLD_TICKS) {
      hold.current = 0;
      const reaction = (performance.now() - shownAt) / 1000;
      const s = reaction < 4 ? 1 : reaction < 8 ? 0.5 : 0.25;
      const next = [...scores, s];
      setScores(next);
      setLast(reaction);
      if (idx + 1 >= i.targets.length) {
        setOn(false);
        onResult(next.reduce((a, b) => a + b, 0) / next.length);
      } else {
        setIdx(idx + 1);
        setShownAt(performance.now());
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [midiFrac, on]);

  const nearest = chart && midiFrac !== null ? nearestFingered(chart, midiFrac) : null;
  return (
    <div className="academy-ex">
      <div className="muted small">{t.academyMicNeeded}</div>
      <div className="analysis-row">
        {on && target !== undefined ? (
          <b className="academy-big">
            {t.academySightPlay}: {noteName(target, lang)}
          </b>
        ) : (
          <span className="muted">
            {i.targets.length} × {t.academySightPlay.toLowerCase()}
          </span>
        )}
        <span className="mono-num small">
          {idx + (on ? 0 : scores.length ? 1 : 0)}/{i.targets.length}
          {last !== null && ` · ${t.academySightReaction} ${last.toFixed(1)} s`}
        </span>
        <span className="spacer" />
        {!on ? (
          <button
            onClick={() => {
              setIdx(0);
              setScores([]);
              setLast(null);
              setShownAt(performance.now());
              setOn(true);
            }}
          >
            ▶ {t.academyStart}
          </button>
        ) : (
          <button className="danger" onClick={() => setOn(false)}>
            ■ {t.stop}
          </button>
        )}
      </div>
      {on && (
        <div className="small mono-num">
          {midiFrac !== null ? `${noteName(Math.round(midiFrac), lang)} ${((midiFrac - Math.round(midiFrac)) * 100).toFixed(0)} c` : t.tunerSilent}
        </div>
      )}
      {chart && on && target !== undefined && (
        <div className="analysis-row">
          <FingeringDiagram chart={chart} midi={target} />
          {nearest && nearest.midi !== target && Math.round(midiFrac!) !== target && <FingeringDiagram chart={chart} midi={nearest.midi} compact />}
        </div>
      )}
    </div>
  );
}

function segmentPitch(stream: { t: number; midi: number | null }[]): Note[] {
  const out: Note[] = [];
  let cur: { start: number; midi: number; cents: number[]; last: number } | null = null;
  let unvoiced = 0;
  const close = (end: number) => {
    if (!cur) return;
    const sorted = [...cur.cents].sort((a, b) => a - b);
    if (end - cur.start >= 0.08)
      out.push({ start: cur.start, end, midi: cur.midi, cents: sorted[Math.floor(sorted.length / 2)] ?? 0, confidence: 1, velocity: 0.7, bend: [] });
    cur = null;
  };
  for (const s of stream) {
    if (s.midi === null) {
      if (++unvoiced >= 2) close(cur?.last ?? s.t);
      continue;
    }
    unvoiced = 0;
    if (!cur || Math.abs((s.midi - cur.midi) * 100) > 80) {
      close(cur?.last ?? s.t);
      cur = { start: s.t, midi: Math.round(s.midi), cents: [(s.midi - Math.round(s.midi)) * 100], last: s.t };
    } else {
      cur.cents.push((s.midi - cur.midi) * 100);
      cur.last = s.t;
    }
  }
  close(cur ? (cur as { last: number }).last + 0.05 : 0);
  return out;
}

function HearRepeatEx({ inst, level, onResult }: ExProps) {
  const { t, lang } = useT();
  const i = inst.instance as Extract<AcademyInstance, { kind: "hear_and_repeat" }>;
  const player = usePlayer();
  const [phase, setPhase] = useState<"idle" | "listen" | "repeat" | "done">("idle");
  const mic = useMic(phase === "repeat");
  const a4 = useStore((s) => s.project?.a4_hz ?? 440);
  const chart = chartFor(useStore((s) => s.project?.instrument?.instrument));
  const stream = useRef<{ t: number; midi: number | null }[]>([]);
  const started = useRef(0);
  const [result, setResult] = useState<RepeatScore | null>(null);
  const phraseLen = Math.max(...i.expected.map((n) => n.end)) - Math.min(...i.expected.map((n) => n.start));

  useEffect(() => {
    if (phase === "listen" && !player.playing) setPhase("repeat");
    if (phase === "repeat") {
      started.current = performance.now();
      stream.current = [];
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [phase, player.playing]);

  useEffect(() => {
    if (phase !== "repeat" || !mic) return;
    const tsec = (performance.now() - started.current) / 1000;
    stream.current.push({ t: tsec, midi: mic.hz !== null && mic.confidence >= 0.6 ? hzToMidi(mic.hz, a4) : null });
    const voiced = stream.current.some((s) => s.midi !== null);
    const firstVoiced = stream.current.find((s) => s.midi !== null)?.t ?? 0;
    if (tsec > 12 || (voiced && tsec - firstVoiced > phraseLen + 1.5)) void finish();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mic]);

  const finish = async () => {
    setPhase("done");
    const played = segmentPitch(stream.current);
    try {
      const r = await api.academyScoreRepeat(i.expected, played, level);
      setResult(r);
      onResult(r.score);
    } catch (e) {
      useStore.getState().setError(describeError(e));
    }
  };

  const start = async () => {
    setResult(null);
    setPhase("listen");
    await player.play(i.expected);
  };

  return (
    <div className="academy-ex">
      <div className="muted small">{t.academyMicNeeded}</div>
      <div className="analysis-row">
        <span className="mono-num small">{i.expected.map((n) => noteName(n.midi, lang)).join(" ")}</span>
        <span className="spacer" />
        {phase === "idle" || phase === "done" ? (
          <button onClick={() => void start()}>▶ {t.academyStart}</button>
        ) : phase === "listen" ? (
          <span className="badge">{t.academyRepeatListen}</span>
        ) : (
          <>
            <span className="badge ok">{t.academyRepeatNow}</span>
            <button className="ghost small" onClick={() => void finish()}>
              {t.academyRepeatStopEarly}
            </button>
          </>
        )}
      </div>
      {chart && (
        <div className="fingering-row">
          {i.expected.map((n, k) => (
            <FingeringDiagram key={k} chart={chart} midi={n.midi} compact showAlternatives={false} />
          ))}
        </div>
      )}
      {result && (
        <div className="trainer-report small">
          <b>{t.report}</b> · {t.hits} {result.report.summary.hits}/{result.report.summary.expected} · {t.wrongPitch} {result.report.summary.wrong_pitch} · {t.missedNotes}{" "}
          {result.report.summary.missed} · {t.meanCents} {result.report.summary.mean_abs_cents.toFixed(0)} c
          <ul>
            {result.report.per_note.map((r) => (
              <li key={r.index} className={r.verdict.kind === "hit" ? "ok" : "bad"}>
                {noteName(i.expected[r.index]?.midi ?? 0, lang)}: {r.verdict.kind === "hit" ? "✓" : r.verdict.kind === "missed" ? "—" : "✗"}{" "}
                {"cents" in r.verdict ? `${r.verdict.cents >= 0 ? "+" : ""}${r.verdict.cents.toFixed(0)} c` : ""}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function EarEx({ inst, onResult }: ExProps) {
  const { t, lang } = useT();
  const i = inst.instance as Extract<AcademyInstance, { kind: "ear" }>;
  const player = usePlayer();
  const [round, setRound] = useState(0);
  const [picked, setPicked] = useState<number | null>(null);
  const [correct, setCorrect] = useState(0);
  const [started, setStarted] = useState(false);
  const r: EarRound | undefined = i.rounds[round];
  const begin = async (k: number) => {
    setPicked(null);
    setRound(k);
    await player.play(i.rounds[k].prompt);
  };
  const pick = (c: number) => {
    if (!r || picked !== null) return;
    setPicked(c);
    const ok = c === r.answer;
    const total = correct + (ok ? 1 : 0);
    setCorrect(total);
    if (round + 1 >= i.rounds.length) {
      setTimeout(() => {
        setStarted(false);
        onResult(total / i.rounds.length);
      }, 900);
    } else {
      setTimeout(() => void begin(round + 1), 900);
    }
  };
  return (
    <div className="academy-ex">
      <div className="analysis-row">
        {started && r ? (
          <>
            <b>
              {t.academyEarRound} {round + 1}/{i.rounds.length}
            </b>
            <button className="ghost small" disabled={player.playing} onClick={() => void player.play(r.prompt)}>
              🔊 {t.academyEarListenAgain}
            </button>
            <span className="mono-num small muted">
              {correct} ✓
            </span>
          </>
        ) : (
          <span className="muted small">{i.rounds.length} × {t.academyEarPick.toLowerCase()}</span>
        )}
        <span className="spacer" />
        {!started && (
          <button
            onClick={() => {
              setCorrect(0);
              setStarted(true);
              void begin(0);
            }}
          >
            ▶ {t.academyStart}
          </button>
        )}
      </div>
      {started && r && (
        <div className="academy-choices">
          {r.choices.map((c, k) => (
            <button key={k} className={`ghost ${picked !== null ? (k === r.answer ? "on" : k === picked ? "wrong" : "") : ""}`} onClick={() => pick(k)} disabled={picked !== null}>
              {choiceLabel(c, lang)}
            </button>
          ))}
        </div>
      )}
      {picked !== null && r && <div className={`banner ${picked === r.answer ? "ok" : "error"}`}>{picked === r.answer ? t.academyCorrect : `${t.academyWrong}: ${choiceLabel(r.choices[r.answer], lang)}`}</div>}
    </div>
  );
}

function ClapEx({ inst, level, onResult }: ExProps) {
  const { t } = useT();
  const i = inst.instance as Extract<AcademyInstance, { kind: "rhythm_clap" }>;
  const [on, setOn] = useState(false);
  const [result, setResult] = useState<RhythmScore | null>(null);
  const [count, setCount] = useState(0);
  const [pos, setPos] = useState<number | null>(null);
  const onsets = useRef<number[]>([]);
  const floor = useRef(0.02);
  const lastOnset = useRef(-1);
  const token = useRef(0);

  const start = async () => {
    const my = ++token.current;
    setResult(null);
    onsets.current = [];
    lastOnset.current = -1;
    floor.current = 0.02;
    setCount(0);
    const st = useStore.getState();
    try {
      await api.startMonitor(st.prefs.selectedInput, st.prefs.sampleRate);
      await api.academyPlay(i.clicks, st.prefs.selectedOutput, "sine");
    } catch (e) {
      st.setError(describeError(e));
      return;
    }
    setOn(true);
    const tick = async () => {
      if (token.current !== my) return;
      const [m, p] = await Promise.all([api.monitorStatus().catch(() => null), api.notesPlaybackStatus().catch(() => null)]);
      if (token.current !== my) return;
      if (m && p && !p.finished) {
        const t0 = p.position_seconds;
        setPos(t0);
        const peak = m.peak;
        if (peak > Math.max(0.08, floor.current * 3) && t0 - lastOnset.current > 0.12) {
          onsets.current.push(t0);
          lastOnset.current = t0;
          setCount(onsets.current.length);
        }
        floor.current = floor.current * 0.9 + peak * 0.1;
        setTimeout(tick, 30);
      } else {
        token.current++;
        setOn(false);
        setPos(null);
        await api.stopMonitor().catch(() => undefined);
        try {
          const r = await api.academyScoreRhythm(onsets.current, i.grid, level);
          setResult(r);
          onResult(r.score);
        } catch (e) {
          useStore.getState().setError(describeError(e));
        }
      }
    };
    void tick();
  };
  useEffect(() => () => void api.stopNotes().catch(() => undefined), []);
  const beat = 60 / i.bpm;
  const inCountIn = pos !== null && pos < i.grid[0] - beat * 0.5;
  return (
    <div className="academy-ex">
      <div className="muted small">{t.academyClapIntro}</div>
      <div className="analysis-row">
        {on ? <span className={`badge ${inCountIn ? "" : "ok"}`}>{inCountIn ? `${Math.max(1, Math.ceil((i.grid[0] - (pos ?? 0)) / beat))}…` : t.academyClapGo}</span> : null}
        <span className="mono-num small">{count} ♪</span>
        <span className="spacer" />
        {!on ? (
          <button onClick={() => void start()}>▶ {t.academyStart}</button>
        ) : (
          <button className="danger" onClick={() => void api.stopNotes()}>
            ■ {t.stop}
          </button>
        )}
      </div>
      <div className="clap-grid" aria-hidden="true">
        {i.grid.map((g, k) => {
          const d = result?.deviations_ms[k];
          const hit = d !== undefined && d !== null;
          return <span key={k} className={`clap-dot ${pos !== null && Math.abs(pos - g) < beat * 0.25 ? "now" : ""} ${result ? (hit ? "hit" : "miss") : ""}`} title={hit ? `${d! >= 0 ? "+" : ""}${d!.toFixed(0)} ms` : ""} />;
        })}
      </div>
      {result && (
        <div className="small">
          {t.academyClapHits} {result.deviations_ms.filter((d) => d !== null).length}/{result.deviations_ms.length} · {t.academyClapExtra} {result.extra} · {t.academyClapMean}{" "}
          {result.mean_abs_ms.toFixed(0)} ms
        </div>
      )}
    </div>
  );
}

function PlayAlongEx({ inst, lesson }: ExProps) {
  const { t, lang } = useT();
  const i = inst.instance as Extract<AcademyInstance, { kind: "play_along" }>;
  const project = useStore((s) => s.project);
  const go = async () => {
    const st = useStore.getState();
    try {
      const pv = await api.academyPlayAlong(i.notes, lesson.id);
      st.setProject(pv);
      st.setAcademyPlayAlong(lesson.id);
      st.setScreen("play");
    } catch (e) {
      st.setError(describeError(e));
    }
  };
  return (
    <div className="academy-ex">
      <div className="muted small">{t.academyPlayAlongHint}</div>
      <div className="analysis-row">
        <span className="mono-num small">{i.notes.map((n) => noteName(n.midi, lang)).join(" ")}</span>
        <span className="spacer" />
        <button onClick={() => void go()} disabled={!project}>
          ▶ {t.academyPlayAlongGo}
        </button>
      </div>
      {!project && <div className="muted small">{t.academyNoProject}</div>}
    </div>
  );
}

function GuidedEx({ lesson }: ExProps) {
  const { t, lang } = useT();
  const run = useStore((s) => s.academyGuided);
  const steps = lesson.exercise.steps ?? [];
  const mine = run && run.lessonId === lesson.id ? run : null;
  const allDone = mine ? mine.done.every(Boolean) : false;
  return (
    <div className="academy-ex">
      <div className="muted small">{t.academyGuidedSteps}</div>
      <ol className="academy-steps">
        {steps.map((s, k) => (
          <li key={k} className={mine ? (mine.done[k] ? "done" : mine.done.findIndex((d) => !d) === k ? "current" : "") : ""}>
            {mine ? (mine.done[k] ? "✓ " : "○ ") : ""}
            {lt(s.text, lang)}
          </li>
        ))}
      </ol>
      <div className="analysis-row">
        {!mine ? (
          <button
            onClick={() =>
              startGuided(
                lesson.id,
                steps.map((s) => s.check),
              )
            }
          >
            ▶ {t.academyGuidedStart}
          </button>
        ) : allDone ? (
          <>
            <span className="banner ok">{t.academyGuidedDone}</span>
            <button className="ghost small" onClick={stopGuided}>
              ✕
            </button>
          </>
        ) : (
          <button className="danger" onClick={stopGuided}>
            ■ {t.academyGuidedStop}
          </button>
        )}
      </div>
    </div>
  );
}

function QuizBlock({ lesson, onScore }: { lesson: LessonView; onScore: (score: number) => void }) {
  const { t, lang } = useT();
  const [picked, setPicked] = useState<(number | null)[]>(lesson.quiz.map(() => null));
  const [checked, setChecked] = useState(false);
  useEffect(() => {
    setPicked(lesson.quiz.map(() => null));
    setChecked(false);
  }, [lesson.id, lesson.quiz]);
  if (lesson.quiz.length === 0) return null;
  const correct = lesson.quiz.filter((q, k) => picked[k] === q.answer).length;
  return (
    <div className="academy-quiz">
      <b>{t.academyQuiz}</b>
      {lesson.quiz.map((q, k) => (
        <div key={k} className="academy-q">
          <div>
            {k + 1}. {lt(q.q, lang)}
          </div>
          <div className="academy-choices">
            {q.choices.map((c, j) => (
              <button
                key={j}
                className={`ghost small ${picked[k] === j ? "on" : ""} ${checked && j === q.answer ? "right" : ""} ${checked && picked[k] === j && j !== q.answer ? "wrong" : ""}`}
                disabled={checked}
                onClick={() => setPicked(picked.map((p, idx) => (idx === k ? j : p)))}
              >
                {lt(c, lang)}
              </button>
            ))}
          </div>
          {checked && q.why && <div className="muted small">{lt(q.why, lang)}</div>}
        </div>
      ))}
      <div className="analysis-row">
        {!checked ? (
          <button
            disabled={picked.some((p) => p === null)}
            onClick={() => {
              setChecked(true);
              onScore(correct / lesson.quiz.length);
            }}
          >
            {t.academyQuizCheck}
          </button>
        ) : (
          <>
            <span className={`badge ${correct === lesson.quiz.length ? "ok" : ""}`}>
              {correct}/{lesson.quiz.length} {t.academyCorrect.toLowerCase()}
            </span>
            <button
              className="ghost small"
              onClick={() => {
                setPicked(lesson.quiz.map(() => null));
                setChecked(false);
              }}
            >
              ↺
            </button>
          </>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- lesson card

function LessonCard({ lesson, view, onProgress, onOpenReading, onNext }: { lesson: LessonView; view: AcademyView; onProgress: () => void; onOpenReading: (id: string) => void; onNext: (id: string) => void }) {
  const { t, lang } = useT();
  const [level, setLevel] = useState<Level>("beginner");
  const [inst, setInst] = useState<InstanceView | null>(null);
  const [outcome, setOutcome] = useState<{ score: number; passed: boolean } | null>(null);
  const player = usePlayer();
  const [seedN, setSeedN] = useState(0);

  useEffect(() => {
    setOutcome(null);
    setInst(null);
    void api
      .academyInstance(lesson.id, seedN === 0 ? null : seedN)
      .then(setInst)
      .catch((e) => useStore.getState().setError(describeError(e)));
    return () => void player.stop();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lesson.id, seedN]);

  const record = async (score: number) => {
    const passed = score >= lesson.pass_score;
    setOutcome({ score, passed });
    try {
      await api.academyRecord(lesson.id, Math.max(0, Math.min(1, score)), passed);
      onProgress();
    } catch (e) {
      useStore.getState().setError(describeError(e));
    }
  };
  const again = () => setSeedN(Date.now() % 1_000_000);
  const kind = lesson.exercise.type;
  const exProps: ExProps | null = inst ? { lesson, inst, level, onResult: (s) => void record(s), onAgain: again } : null;
  const needsLevel = kind !== "quiz" && kind !== "guided" && kind !== "play_along";
  const scaleName = view.has_scale ? `${t.noteNames[view.context.tonic_pc]} ${(t.scaleKinds as Record<string, string>)[view.context.kind] ?? view.context.kind}` : null;
  return (
    <div className="panel academy-card">
      <div className="analysis-row">
        <h3 className="grow">{lt(lesson.title, lang)}</h3>
        <span className={`badge ${lesson.progress.passed_at_ms ? "ok" : ""}`}>{lesson.progress.passed_at_ms ? `✓ ${t.academyPassed}` : t.academyNotPassed}</span>
        {lesson.progress.attempts > 0 && (
          <span className="muted small mono-num">
            {lesson.progress.attempts} {t.academyAttempts} · {t.academyBest} {(lesson.progress.best_score * 100).toFixed(0)} %
          </span>
        )}
      </div>
      <div className="academy-two">
        <div>
          <b>{t.academyObjectives}</b>
          <ul className="small">
            {lesson.objectives.map((o, k) => (
              <li key={k}>{lt(o, lang)}</li>
            ))}
          </ul>
        </div>
        <div>
          <p>{lt(lesson.explanation, lang)}</p>
          {lesson.glossary.length > 0 && (
            <div className="small muted">
              {t.academyGlossary}: {lesson.glossary.join(" · ")}
            </div>
          )}
          {lesson.reading.length > 0 && (
            <div className="analysis-row small">
              <span className="muted">{t.academyReading}:</span>
              {lesson.reading.map((r) => (
                <button key={r} className="ghost small" onClick={() => onOpenReading(r)}>
                  📖 {r.replace(/-/g, " ").slice(0, 48)}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      {inst?.demo && (
        <div className="analysis-row">
          {!player.playing ? (
            <button className="ghost" onClick={() => void player.play(inst.demo!)}>
              🔊 {t.academyDemo}
            </button>
          ) : (
            <button className="ghost" onClick={() => void player.stop()}>
              ■ {t.academyDemoStop}
            </button>
          )}
          <span className="muted small mono-num">{inst.demo.map((n) => noteName(n.midi, lang)).join(" ")}</span>
        </div>
      )}
      <div className="academy-exercise">
        <div className="analysis-row">
          <b>{t.academyExercise}</b>
          <span className="muted small">{scaleName ? `${t.academyContext}: ${scaleName}, ${noteName(view.context.lo_midi, lang)}–${noteName(view.context.hi_midi, lang)}` : view.has_project ? t.academyContextNeutral : t.academyNoProject}</span>
          <span className="spacer" />
          {needsLevel && (
            <label className="inline small">
              {t.trainerLevel}
              <select value={level} onChange={(e) => setLevel(e.target.value as Level)}>
                {LEVELS.map((l) => (
                  <option key={l} value={l}>
                    {t.levels[l]}
                  </option>
                ))}
              </select>
            </label>
          )}
          {kind !== "quiz" && kind !== "guided" && (
            <button className="ghost small" onClick={again} title={t.academyOnceMore}>
              🎲 {t.academyOnceMore}
            </button>
          )}
        </div>
        {exProps && kind === "hold_tone" && <HoldToneEx key={inst!.seed} {...exProps} />}
        {exProps && kind === "sight_reading" && <SightReadingEx key={inst!.seed} {...exProps} />}
        {exProps && kind === "hear_and_repeat" && <HearRepeatEx key={inst!.seed} {...exProps} />}
        {exProps && kind.startsWith("ear_") && <EarEx key={inst!.seed} {...exProps} />}
        {exProps && kind === "rhythm_clap" && <ClapEx key={inst!.seed} {...exProps} />}
        {exProps && kind === "play_along" && <PlayAlongEx key={inst!.seed} {...exProps} />}
        {exProps && kind === "guided" && <GuidedEx {...exProps} />}
        {outcome && (
          <div className={`banner ${outcome.passed ? "ok" : "warnb"}`}>
            {t.academyScore} {(outcome.score * 100).toFixed(0)} % · {outcome.passed ? t.academyPassedNow : t.academyFailedNow}
          </div>
        )}
      </div>
      <QuizBlock lesson={lesson} onScore={(s) => (kind === "quiz" ? void record(s) : undefined)} />
      {lesson.mini_project && (
        <div className="small">
          <b>{t.academyMiniProject}:</b> {lt(lesson.mini_project, lang)}
        </div>
      )}
      <div className="analysis-row">
        <button
          className="ghost small"
          onClick={() => void api.academyReset(lesson.id).then(onProgress).catch((e) => useStore.getState().setError(describeError(e)))}
          disabled={lesson.progress.attempts === 0}
        >
          {t.academyReset}
        </button>
        <span className="spacer" />
        {lesson.next && <button onClick={() => onNext(lesson.next!)}>{t.academyNext} →</button>}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- knowledge base

function KnowledgeReader({ target }: { target: string | null }) {
  const { t } = useT();
  const parsed = useMemo(() => {
    const blocks = parseMarkdown(kbSource);
    const secs = sections(blocks, 3);
    const parts: { title: string; id: string; chapters: Section[] }[] = [];
    for (const s of secs) {
      if (s.level <= 2) parts.push({ title: s.title, id: s.id, chapters: [s] });
      else if (parts.length) parts[parts.length - 1].chapters.push(s);
      else parts.push({ title: s.title, id: s.id, chapters: [s] });
    }
    const idToSection = new Map<string, string>();
    for (const s of secs) {
      idToSection.set(s.id, s.id);
      for (const b of s.blocks) if (b.kind === "heading") idToSection.set(b.id, s.id);
    }
    return { secs, parts, idToSection };
  }, []);
  const [active, setActive] = useState<string>(parsed.secs[1]?.id ?? parsed.secs[0]?.id ?? "top");
  const [query, setQuery] = useState("");
  useEffect(() => {
    if (!target) return;
    const sec = parsed.idToSection.get(target);
    if (sec) {
      setActive(sec);
      setQuery("");
      setTimeout(() => document.getElementById(target)?.scrollIntoView({ block: "start", behavior: "smooth" }), 50);
    }
  }, [target, parsed]);
  const q = query.trim().toLowerCase();
  const shown: Section[] = q ? parsed.secs.filter((s) => s.text.includes(q)) : parsed.secs.filter((s) => s.id === active);
  const snippet = (s: Section): string => {
    const text = s.blocks.map(blockText).join(" ");
    const i = text.toLowerCase().indexOf(q);
    return i < 0 ? "" : "…" + text.slice(Math.max(0, i - 60), i + 90) + "…";
  };
  return (
    <div className="help-layout">
      <nav className="panel help-toc kb-toc" aria-label={t.helpContents}>
        {parsed.parts.map((p) => (
          <div key={p.id}>
            <div className="small muted kb-part">{p.title}</div>
            {p.chapters.map((c) => (
              <button key={c.id} className={`ghost small ${active === c.id && !q ? "on" : ""}`} onClick={() => { setQuery(""); setActive(c.id); }}>
                {c.title.replace(/^Скан /, "").slice(0, 44)}
              </button>
            ))}
          </div>
        ))}
      </nav>
      <div className="panel help-body">
        <div className="analysis-row">
          <input className="grow" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t.kbSearch} aria-label={t.search} />
          {q && <span className="muted small">{shown.length}</span>}
        </div>
        <div className="muted small">{t.kbHint}</div>
        {shown.length === 0 && <div className="muted">{t.helpNoMatch}</div>}
        {q
          ? shown.map((s) => (
              <div key={s.id} className="help-section">
                <button className="ghost" onClick={() => { setQuery(""); setActive(s.id); }}>
                  {s.title}
                </button>
                <div className="muted small">{snippet(s)}</div>
              </div>
            ))
          : shown.map((s) => (
              <div key={s.id} className="help-section">
                <h3 id={s.id}>{s.title}</h3>
                {s.blocks.map((b: Block, i: number) => (
                  <BlockView key={i} b={b} />
                ))}
              </div>
            ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- fingering chart

function FingeringChartView() {
  const { t, lang } = useT();
  const player = usePlayer();
  const [sel, setSel] = useState<number>(62);
  const byRegister = useMemo(() => {
    const m = new Map<number, number[]>();
    for (const f of KAVAL_D.fingerings) {
      if (f.alt) continue;
      const arr = m.get(f.register) ?? [];
      if (!arr.includes(f.midi)) arr.push(f.midi);
      m.set(f.register, arr);
    }
    return [...m.entries()].sort((a, b) => a[0] - b[0]);
  }, []);
  return (
    <div className="panel">
      <div className="muted small">{t.fingeringChartHint}</div>
      {byRegister.map(([reg, notes]) => (
        <div key={reg} className="analysis-row">
          <span className="small muted" style={{ minWidth: 220 }}>
            {KAVAL_D.registers[String(reg)]?.[lang]}
          </span>
          {notes.map((m) => (
            <button
              key={m}
              className={`ghost small ${sel === m ? "on" : ""}`}
              onClick={() => {
                setSel(m);
                void player.play([{ start: 0.05, end: 1.0, midi: m, velocity: 0.7 }]);
              }}
            >
              {noteName(m, lang)}
            </button>
          ))}
        </div>
      ))}
      <FingeringDiagram chart={KAVAL_D} midi={sel} />
    </div>
  );
}

// ---------------------------------------------------------------- screen

export function AcademyScreen() {
  const { t, lang } = useT();
  const [view, setView] = useState<AcademyView | null>(null);
  const [tab, setTab] = useState<Tab>("levels");
  const [sel, setSel] = useState<string | null>(null);
  const [reading, setReading] = useState<string | null>(null);
  const guided = useStore((s) => s.academyGuided);

  const load = useCallback(() => {
    void api
      .academyLessons()
      .then(setView)
      .catch((e) => useStore.getState().setError(describeError(e)));
  }, []);
  useEffect(load, [load]);
  useEffect(() => {
    if (guided && guided.done.every(Boolean)) load();
  }, [guided, load]);

  const lessons = view?.lessons ?? [];
  const levelLessons = lessons.filter((l) => !l.track);
  const kavalLessons = lessons.filter((l) => l.track === "kaval");
  const selected = lessons.find((l) => l.id === sel) ?? null;
  const list = tab === "kaval" ? kavalLessons : levelLessons;
  const passed = (ls: LessonView[]) => ls.filter((l) => l.progress.passed_at_ms).length;
  const levels = [...new Set(levelLessons.map((l) => l.level))].sort((a, b) => a - b);

  return (
    <section className="academy">
      <div className="analysis-row">
        <div className="muted small grow">{t.academyIntro}</div>
        {(["levels", "kaval", "fingerings", "knowledge"] as Tab[]).map((k) => (
          <button key={k} className={`ghost ${tab === k ? "on" : ""}`} onClick={() => setTab(k)}>
            {k === "levels" ? t.academyTabLevels : k === "kaval" ? t.academyTabKaval : k === "fingerings" ? t.academyTabFingerings : t.academyTabKnowledge}
          </button>
        ))}
      </div>
      {tab === "knowledge" && <KnowledgeReader target={reading} />}
      {tab === "fingerings" && <FingeringChartView />}
      {(tab === "levels" || tab === "kaval") && (
        <div className="academy-layout">
          <nav className="panel academy-list" aria-label={t.academyTitle}>
            <div className="small muted">
              {passed(list)}/{list.length} {t.academyStats}
            </div>
            {tab === "levels"
              ? levels.map((lv) => (
                  <div key={lv}>
                    <div className="small muted academy-level">
                      {t.academyLevel} {lv} · {t.academyLevelNames[lv]}
                    </div>
                    {levelLessons
                      .filter((l) => l.level === lv)
                      .map((l) => (
                        <button key={l.id} className={`ghost small ${sel === l.id ? "on" : ""}`} onClick={() => setSel(l.id)}>
                          {l.progress.passed_at_ms ? "✓ " : "○ "}
                          {lt(l.title, lang)}
                        </button>
                      ))}
                  </div>
                ))
              : kavalLessons.map((l) => (
                  <button key={l.id} className={`ghost small ${sel === l.id ? "on" : ""}`} onClick={() => setSel(l.id)}>
                    {l.progress.passed_at_ms ? "✓ " : "○ "}
                    {lt(l.title, lang)}
                  </button>
                ))}
          </nav>
          {selected && view ? (
            <LessonCard
              key={selected.id}
              lesson={selected}
              view={view}
              onProgress={load}
              onNext={(id) => {
                setSel(id);
                const n = lessons.find((l) => l.id === id);
                if (n) setTab(n.track === "kaval" ? "kaval" : "levels");
              }}
              onOpenReading={(id) => {
                setReading(id);
                setTab("knowledge");
              }}
            />
          ) : (
            <div className="panel muted">{t.academyPickLesson}</div>
          )}
        </div>
      )}
    </section>
  );
}

/** Small bar on every screen while a guided lesson runs (F6.3 mentor, first slice). */
export function GuidedBanner() {
  const { t, lang } = useT();
  const run = useStore((s) => s.academyGuided);
  const screen = useStore((s) => s.screen);
  const setScreen = useStore((s) => s.setScreen);
  const [steps, setSteps] = useState<LText[] | null>(null);
  useEffect(() => {
    if (!run) {
      setSteps(null);
      return;
    }
    void api
      .academyLessons()
      .then((v) => setSteps(v.lessons.find((l) => l.id === run.lessonId)?.exercise.steps?.map((s) => s.text) ?? null))
      .catch(() => setSteps(null));
  }, [run?.lessonId]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!run || screen === "academy") return null;
  const k = run.done.findIndex((d) => !d);
  const all = k < 0;
  return (
    <div className={`banner ${all ? "ok" : "warnb"} guided-banner`} role="status">
      <b>{t.academyGuidedActive}</b>{" "}
      {all ? t.academyGuidedDone : `${t.academyGuidedStep} ${k + 1}/${run.done.length}: ${steps ? lt(steps[k], lang) : ""}`}
      <span className="spacer" />
      <button className="ghost small" onClick={() => setScreen("academy")}>
        {t.academyGuidedBack}
      </button>
      <button className="ghost small" onClick={stopGuided} aria-label={t.academyGuidedStop}>
        ✕
      </button>
    </div>
  );
}
