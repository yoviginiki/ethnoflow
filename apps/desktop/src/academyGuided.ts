import { useEffect } from "react";
import { api, type AcademyCheck } from "./api";
import { hzToMidi } from "./format";
import { useStore, type GuidedRun } from "./store";

const POLL_MS = 400;
const STABLE_CONFIDENCE = 0.6;

/** Start a guided lesson: its checks are evaluated on every screen until all are done. */
export function startGuided(lessonId: string, checks: AcademyCheck[]) {
  const st = useStore.getState();
  st.setAcademyGuided({
    lessonId,
    startedAtMs: Date.now(),
    checks,
    done: checks.map(() => false),
    visited: [st.screen],
  });
}

export function stopGuided() {
  useStore.getState().setAcademyGuided(null);
}

async function evaluate(c: AcademyCheck, r: GuidedRun, stable: { since: number | null; midi: number | null }): Promise<boolean> {
  const st = useStore.getState();
  switch (c.kind) {
    case "screen":
      return r.visited.includes(c.name) || st.screen === c.name;
    case "metronome_on":
      return st.metronomeOn;
    case "scale_chosen":
      return !!st.project?.scale;
    case "space_at_least":
      return (st.ambient?.macros.space ?? 0) >= c.value;
    case "tuner_stable": {
      const m = st.monitor;
      const midi = m && m.pitch_hz !== null && m.pitch_confidence >= STABLE_CONFIDENCE ? Math.round(hzToMidi(m.pitch_hz, st.project?.a4_hz ?? 440)) : null;
      const now = performance.now();
      if (midi !== null && midi === stable.midi) {
        if (stable.since === null) stable.since = now;
        return now - stable.since >= c.seconds * 1000;
      }
      stable.midi = midi;
      stable.since = midi === null ? null : now;
      return false;
    }
    case "rhythm_running": {
      try {
        const rs = await api.rhythmStatus();
        return rs.playing && (c.bpm === null || Math.abs(rs.bpm - c.bpm) < 0.6);
      } catch {
        return false;
      }
    }
    case "new_take": {
      try {
        return (await api.academyNewTake(r.startedAtMs, c.min_notes)) !== null;
      } catch {
        return false;
      }
    }
    case "preset_saved": {
      try {
        const list = await api.libraryList(null, null, null, "date_desc");
        return list.some((e) => e.created_ms >= r.startedAtMs);
      } catch {
        return false;
      }
    }
  }
}

/**
 * Mounted once in App (F6.3 "overlay mentor", first slice): while a guided lesson runs,
 * its checks are evaluated against the real app state on whatever screen the user is
 * on; when all are done the lesson is recorded as passed.
 */
export function useAcademyGuided() {
  const lessonId = useStore((s) => s.academyGuided?.lessonId ?? null);
  const screen = useStore((s) => s.screen);

  useEffect(() => {
    const st = useStore.getState();
    const r = st.academyGuided;
    if (r && !r.visited.includes(screen)) st.setAcademyGuided({ ...r, visited: [...r.visited, screen] });
  }, [screen, lessonId]);

  useEffect(() => {
    if (!lessonId) return;
    let cancelled = false;
    let recorded = false;
    const stable = { since: null as number | null, midi: null as number | null };
    const tick = async () => {
      if (cancelled) return;
      const r = useStore.getState().academyGuided;
      if (!r || r.lessonId !== lessonId) return;
      const done = [...r.done];
      let changed = false;
      for (let i = 0; i < r.checks.length; i++) {
        if (done[i]) continue;
        if (await evaluate(r.checks[i], r, stable)) {
          done[i] = true;
          changed = true;
        }
      }
      if (cancelled) return;
      const latest = useStore.getState().academyGuided;
      if (!latest || latest.lessonId !== lessonId) return;
      if (changed) useStore.getState().setAcademyGuided({ ...latest, done });
      if (done.every(Boolean) && !recorded) {
        recorded = true;
        try {
          await api.academyRecord(lessonId, 1, true);
        } catch {
          // progress file unavailable: the UI still shows the ticks
        }
        return;
      }
      setTimeout(tick, POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
    };
  }, [lessonId]);
}
