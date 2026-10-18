import { useEffect, useMemo, useState } from "react";
import { api, describeError, type LessonView, type LText } from "./api";
import { stopGuided } from "./academyGuided";
import { scaleFromKind } from "./AnalysisPanel";
import { makeSong } from "./makeSong";
import { messages, type Messages } from "./i18n";
import { useStore, type Screen } from "./store";

/** One piece of advice: what the assistant sees, what to do, and where. */
export interface Tip {
  id: string;
  text: string;
  /** Button label, when the assistant can do it or take you there. */
  action?: string;
  run?: () => void | Promise<void>;
  screen?: Screen;
  /** `data-coach` id of the control to light up after navigating. */
  coach?: string;
  level: "do" | "hint" | "ok";
}

const HIGHLIGHT_MS = 4500;

/** Light up the control carrying `data-coach={id}` for a few seconds (the "lit trail"). */
export function coachHighlight(id: string) {
  const st = useStore.getState();
  st.setCoachHighlight(id);
  window.setTimeout(() => {
    if (useStore.getState().coachHighlight === id) useStore.getState().setCoachHighlight(null);
  }, HIGHLIGHT_MS);
}

/** Mounted once in App: applies/removes the highlight class on the live DOM. */
export function useCoachHighlight() {
  const id = useStore((s) => s.coachHighlight);
  const screen = useStore((s) => s.screen);
  useEffect(() => {
    if (!id) return;
    let el: Element | null = null;
    const t = window.setTimeout(() => {
      el = document.querySelector(`[data-coach="${id}"]`);
      if (el) {
        el.classList.add("coach-lit");
        el.scrollIntoView({ block: "center", behavior: "smooth" });
      }
    }, 150);
    return () => {
      window.clearTimeout(t);
      el?.classList.remove("coach-lit");
    };
  }, [id, screen]);
}

function goAndLight(screen: Screen | undefined, coach: string | undefined) {
  const st = useStore.getState();
  if (screen && st.screen !== screen) st.setScreen(screen);
  if (coach) coachHighlight(coach);
}

/** The rules: what the assistant notices from the real state. Pure, so it can be tested. */
export function computeTips(t: Messages, s: ReturnType<typeof useStore.getState>, wizard: LessonView | null): Tip[] {
  const tips: Tip[] = [];
  const p = s.project;
  if (!p) {
    tips.push({ id: "no-project", text: t.coachNoProject, action: t.coachOpenProject, screen: "home", level: "do" });
    return tips;
  }
  const a = s.analysis;
  const top = a?.hypotheses[0] ?? null;
  // Key: the loaded take against the project's scale.
  if (top && a && a.notes.length >= 5) {
    if (!p.scale) {
      tips.push({
        id: "no-scale",
        text: `${t.coachNoScale} ${t.noteNames[top.tonic_pc]} ${(t.scaleKinds as Record<string, string>)[top.kind] ?? top.kind} (${Math.round(top.probability * 100)} %).`,
        action: t.coachTakeKey,
        run: async () => {
          const pv = await api.setScale(scaleFromKind({ tonic_pc: top.tonic_pc, degrees_cents: [], kind: top.kind }));
          useStore.getState().setProject(pv);
        },
        level: "do",
      });
    } else if (p.scale.tonic_pc !== top.tonic_pc && top.probability >= 0.15) {
      const second = a.hypotheses.find((h) => h.tonic_pc === p.scale!.tonic_pc);
      if (!second || top.probability - second.probability > 0.03) {
        tips.push({
          id: "key-mismatch",
          text: t.coachKeyMismatch.replace("{take}", `${t.noteNames[top.tonic_pc]} ${(t.scaleKinds as Record<string, string>)[top.kind] ?? top.kind}`).replace("{project}", `${t.noteNames[p.scale.tonic_pc]} ${p.scale.kind ? (t.scaleKinds as Record<string, string>)[p.scale.kind] ?? "" : ""}`),
          action: t.coachTakeKey,
          run: async () => {
            const pv = await api.setScale(scaleFromKind({ tonic_pc: top.tonic_pc, degrees_cents: [], kind: top.kind }));
            useStore.getState().setProject(pv);
          },
          level: "do",
        });
      }
    }
  }
  // Tempo: the take's pulse against the project's tempo (when the make-song flow measured it).
  const mk = s.lastMake;
  if (mk && mk.detectedBpm && p.timing.bpm && Math.abs(mk.detectedBpm - p.timing.bpm) > 3 && Math.abs(mk.detectedBpm * 2 - p.timing.bpm) > 3) {
    tips.push({ id: "tempo-free", text: t.coachTempoFree.replace("{take}", String(mk.detectedBpm)).replace("{project}", String(p.timing.bpm)), level: "hint", screen: "compose", coach: "timing", action: t.coachShow });
  }
  if (!p.timing.bpm) tips.push({ id: "no-timing", text: t.coachNoTiming, action: t.coachShow, screen: "compose", coach: "timing", level: "do" });
  // Background.
  const anyLayer = s.layers.drone || s.layers.pad || s.layers.ambient || s.layers.rhythm;
  const hasTake = p.takes.some((tk) => tk.state === "complete");
  if (hasTake && !anyLayer && s.mode === "idle") {
    tips.push({ id: "no-background", text: t.coachNoBackground, action: t.coachMakeSong, screen: "create", level: "do" });
  }
  // Melody track strip.
  const arrTrack = s.coachArrangement?.tracks.find((tr) => tr.clips.length > 0 && !tr.mute);
  if (arrTrack && arrTrack.highpass_hz < 80 && anyLayer) {
    tips.push({
      id: "melody-hp",
      text: t.coachMelodyHp,
      action: t.coachApply,
      run: async () => {
        const v = await api.arrangementGet();
        const tracks = v.arrangement.tracks.map((tr) => (tr.id === arrTrack.id ? { ...tr, highpass_hz: 140, send: Math.max(tr.send, 0.15) } : tr));
        await api.setArrangement({ ...v.arrangement, tracks });
        useStore.getState().setCoachArrangement({ ...v.arrangement, tracks });
      },
      level: "hint",
    });
  }
  // Song map.
  if (p.song_map.sections.length < 2 && hasTake) tips.push({ id: "no-map", text: t.coachNoMap, action: t.coachShow, screen: "songmap", coach: "songmap", level: "hint" });
  // Latency and profile.
  if (s.prefs.latencyMs === null) tips.push({ id: "latency", text: t.coachLatency, action: t.coachShow, screen: "play", coach: "latency", level: "hint" });
  if (!p.instrument) tips.push({ id: "profile", text: t.coachProfile, action: t.coachShow, screen: "play", coach: "instrument", level: "hint" });
  // Live follower.
  if (anyLayer && !s.liveRunning && (s.layers.pad || s.layers.drone)) tips.push({ id: "live", text: t.coachLive, action: t.coachShow, screen: "compose", coach: "live", level: "hint" });
  if (tips.length === 0) tips.push({ id: "ok", text: wizard?.progress.passed_at_ms ? t.coachAllGoodDone : t.coachAllGood, level: "ok" });
  return tips;
}

/**
 * The assistant (F6.3 Coach, first slice): a rail on every screen that knows the state,
 * says what to do next, does it or takes you to the control and lights it up, and shows
 * the running guided lesson. Nothing it suggests is cosmetic: every action is a real call.
 */
export function CoachPanel() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [wizard, setWizard] = useState<LessonView | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const run = s.academyGuided;
  useEffect(() => {
    void api
      .academyLessons()
      .then((v) => setWizard(v.lessons.find((l) => l.id === (run?.lessonId ?? "l5-06-first-song")) ?? null))
      .catch(() => setWizard(null));
  }, [run?.lessonId, run?.done.filter(Boolean).length]); // eslint-disable-line react-hooks/exhaustive-deps
  // Keep a light copy of the arrangement for the strip tips (refreshed on screen changes).
  useEffect(() => {
    if (!s.project) return;
    void api
      .arrangementGet()
      .then((v) => useStore.getState().setCoachArrangement(v.arrangement))
      .catch(() => undefined);
  }, [s.screen, s.project?.takes.length]); // eslint-disable-line react-hooks/exhaustive-deps
  const tips = useMemo(() => computeTips(t, s, wizard), [t, s, wizard]);
  const steps: LText[] = wizard?.exercise.steps?.map((x) => x.text) ?? [];
  const current = run ? run.done.findIndex((d) => !d) : -1;
  const doTip = async (tip: Tip) => {
    setBusy(tip.id);
    try {
      if (tip.run) await tip.run();
      if (tip.screen || tip.coach) goAndLight(tip.screen, tip.coach);
    } catch (e) {
      useStore.getState().setError(describeError(e));
    } finally {
      setBusy(null);
    }
  };
  const quickMake = async () => {
    setBusy("make");
    try {
      const r = await makeSong(s.prefs.makeStyle, null);
      useStore.getState().setMessage(`${t.makeDone}: ${r.facts.length} ${t.makeFactsWord}`);
    } catch (e) {
      useStore.getState().setError(describeError(e));
    } finally {
      setBusy(null);
    }
  };
  return (
    <aside className="coach-rail" aria-label={t.coachTitle}>
      <div className="analysis-row">
        <b>{t.coachTitle}</b>
        <span className="spacer" />
        <button className="ghost small" onClick={() => s.setPref("coach", false)} title={t.coachHide}>
          ✕
        </button>
      </div>
      {run && (
        <div className="coach-card guided">
          <div className="small muted">{t.academyGuidedActive}</div>
          {current >= 0 ? (
            <div>
              <b>
                {t.academyGuidedStep} {current + 1}/{run.done.length}
              </b>
              <div className="small">{steps[current]?.[s.prefs.lang] ?? ""}</div>
            </div>
          ) : (
            <div className="small">{t.academyGuidedDone}</div>
          )}
          <div className="analysis-row">
            <button className="ghost small" onClick={() => s.setScreen(run.lessonId === "l5-06-first-song" ? "create" : "academy")}>
              {t.academyGuidedBack}
            </button>
            <button className="ghost small" onClick={stopGuided}>
              {t.academyGuidedStop}
            </button>
          </div>
        </div>
      )}
      {tips.map((tip) => (
        <div key={tip.id} className={`coach-card ${tip.level}`}>
          <div className="small">{tip.text}</div>
          {tip.action && (
            <button className="ghost small" disabled={busy !== null} onClick={() => void doTip(tip)}>
              {busy === tip.id ? "…" : tip.action}
            </button>
          )}
        </div>
      ))}
      {s.project && s.project.takes.length > 0 && (
        <div className="coach-card">
          <div className="small muted">{t.makeQuickHint}</div>
          <button disabled={busy !== null || s.mode !== "idle"} onClick={() => void quickMake()}>
            {busy === "make" ? t.makeWorking : `▶ ${t.makeButton}`}
          </button>
        </div>
      )}
      {s.lastMake && (
        <div className="coach-card">
          <div className="small muted">{t.makeWhatIDid}</div>
          <ul className="small coach-facts">
            {s.lastMake.facts.map((f, i) => (
              <li key={i}>
                {t[f.key] as string}
                {f.detail ? `: ${f.detail}` : ""}
              </li>
            ))}
          </ul>
        </div>
      )}
    </aside>
  );
}
