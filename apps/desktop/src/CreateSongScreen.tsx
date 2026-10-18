import { useEffect, useState } from "react";
import { startGuided, stopGuided } from "./academyGuided";
import { api, describeError, type AcademyCheck, type LessonView } from "./api";
import { messages, type Lang } from "./i18n";
import { useStore, type Screen } from "./store";
import { makeSong, STARTER_TITLE } from "./makeSong";
import { STARTER_IDS, type StarterId } from "./starters";

const WIZARD_ID = "l5-06-first-song";

/** Where each wizard step is done (the "Go" button). */
function screenFor(c: AcademyCheck): Screen {
  switch (c.kind) {
    case "new_take":
    case "scale_chosen":
    case "tuner_stable":
      return "play";
    case "timing_set":
    case "background_on":
    case "rhythm_running":
    case "metronome_on":
      return "compose";
    case "song_map_sections":
      return "songmap";
    case "arrangement_clips":
    case "song_exported":
      return "studio";
    case "space_at_least":
    case "preset_saved":
      return "ambient";
    case "screen":
      return c.name as Screen;
  }
}

/**
 * "Create a song" (Phase 5 guide): the Academy's guided engine run on the whole path
 * melody → scale → tempo → map → background → studio → export. Steps tick themselves from
 * the real project state; the lesson is recorded as passed when all are done.
 */
export function CreateSongScreen() {
  const lang = useStore((s) => s.prefs.lang) as Lang;
  const t = messages[lang];
  const project = useStore((s) => s.project);
  const run = useStore((s) => s.academyGuided);
  const setScreen = useStore((s) => s.setScreen);
  const [lesson, setLesson] = useState<LessonView | null>(null);
  const [making, setMaking] = useState(false);
  const makeStyle = useStore((s) => s.prefs.makeStyle);
  const setPref = useStore((s) => s.setPref);
  const lastMake = useStore((s) => s.lastMake);

  useEffect(() => {
    void api
      .academyLessons()
      .then((v) => setLesson(v.lessons.find((l) => l.id === WIZARD_ID) ?? null))
      .catch((e) => useStore.getState().setError(describeError(e)));
  }, [run?.done.filter(Boolean).length]); // eslint-disable-line react-hooks/exhaustive-deps
  const steps = lesson?.exercise.steps ?? [];
  const mine = run && run.lessonId === WIZARD_ID ? run : null;
  const current = mine ? mine.done.findIndex((d) => !d) : -1;
  const allDone = mine ? mine.done.every(Boolean) : false;
  const doMake = async () => {
    setMaking(true);
    try {
      if (!mine) startGuided(WIZARD_ID, steps.map((s) => s.check));
      const r = await makeSong(makeStyle, null);
      useStore.getState().setMessage(`${t.makeDone}: ${r.facts.length} ${t.makeFactsWord}`);
    } catch (e) {
      useStore.getState().setError(describeError(e));
    } finally {
      setMaking(false);
    }
  };
  return (
    <section className="create-song">
      <div className="panel">
        <h3>{t.createTitle}</h3>
        <p className="muted small">{t.createIntro}</p>
        <div className="analysis-row make-row">
          <label className="inline small">
            {t.makeStyle}
            <select value={makeStyle} disabled={making} onChange={(e) => setPref("makeStyle", e.target.value as StarterId)}>
              {STARTER_IDS.map((id) => (
                <option key={id} value={id}>
                  {t[STARTER_TITLE[id]] as string}
                </option>
              ))}
            </select>
          </label>
          <button disabled={!project || making || project.takes.length === 0} onClick={() => void doMake()} data-coach="make">
            {making ? t.makeWorking : `▶ ${t.makeButton}`}
          </button>
          <span className="muted small">{t.makeHint}</span>
        </div>
        {lastMake && (
          <div className="banner ok small">
            <b>{t.makeWhatIDid}</b>
            <ul className="coach-facts">
              {lastMake.facts.map((f, i) => (
                <li key={i}>
                  {t[f.key] as string}
                  {f.detail ? `: ${f.detail}` : ""}
                </li>
              ))}
            </ul>
          </div>
        )}
        {!project && <div className="banner warnb">{t.pickProjectFirst}</div>}
        <div className="analysis-row">
          {!mine ? (
            <button disabled={!project || steps.length === 0} onClick={() => startGuided(WIZARD_ID, steps.map((s) => s.check))}>
              ▶ {t.createStart}
            </button>
          ) : allDone ? (
            <>
              <span className="banner ok">{t.createDone}</span>
              <button className="ghost small" onClick={stopGuided}>
                ↺ {t.createAgain}
              </button>
            </>
          ) : (
            <button className="danger" onClick={stopGuided}>
              ■ {t.createStop}
            </button>
          )}
          {lesson?.progress.passed_at_ms && <span className="badge ok">✓ {t.academyPassed}</span>}
          <span className="spacer" />
          <button className="ghost small" onClick={() => setScreen("academy")}>
            {t.createToAcademy}
          </button>
        </div>
      </div>
      <ol className="create-steps">
        {steps.map((s, k) => {
          const done = mine ? mine.done[k] : false;
          const isCurrent = mine ? k === current : false;
          return (
            <li key={k} className={`panel create-step ${done ? "done" : ""} ${isCurrent ? "current" : ""}`}>
              <div className="create-step-head">
                <span className="create-step-num">{done ? "✓" : k + 1}</span>
                <span className="grow">{s.text[lang]}</span>
                <button className={`ghost small ${isCurrent ? "on" : ""}`} onClick={() => setScreen(screenFor(s.check))}>
                  {t.createGo} →
                </button>
              </div>
            </li>
          );
        })}
      </ol>
      {lesson?.mini_project && (
        <div className="muted small">
          <b>{t.academyMiniProject}:</b> {lesson.mini_project[lang]}
        </div>
      )}
    </section>
  );
}
