import { useState } from "react";
import { AREAS } from "./areas";
import { describeError } from "./api";
import { takeName } from "./format";
import { messages, type StringMessageKey } from "./i18n";
import { ProjectPicker } from "./ProjectPicker";
import { useStore, type Screen } from "./store";
import { continueLast, latestTake } from "./takeActions";

/** Title of each screen for the "also here" links (same keys as the top-bar breadcrumb). */
const SCREEN_TITLE: Partial<Record<Screen, StringMessageKey>> = {
  play: "areaPlay",
  tuner: "areaTuner",
  compose: "bgTabDronePad",
  live: "bgTabLive",
  studio: "areaStudio",
  ambient: "areaAmbient",
  rhythm: "areaRhythm",
  library: "areaLibrary",
  soundscapes: "areaSoundscapes",
  instrument: "areaForge",
  songmap: "areaSongMap",
  samples: "areaSamples",
  references: "areaReferences",
  academy: "areaLearn",
  create: "areaCreate",
  start: "areaStart",
};

/**
 * Home: "Continue: <project>" (opens Play with the latest take loaded), "I have a melody →"
 * (Start from a melody) and one card per area — Play / Background / Song / Learn / Library
 * (UX audit 2026-10-31, stage 3). The 16 screens are still here, as "also here" links
 * under their area, until stages 4–5 turn them into tabs. Help lives in the top bar.
 */
export function Home() {
  const t = messages[useStore((s) => s.prefs.lang)];
  const setScreen = useStore((s) => s.setScreen);
  const project = useStore((s) => s.project);
  const mode = useStore((s) => s.mode);
  const [busy, setBusy] = useState(false);
  const latest = project ? latestTake(project.takes) : null;

  const onContinue = () => {
    setBusy(true);
    continueLast()
      .catch((e) => useStore.getState().setError(describeError(e)))
      .finally(() => setBusy(false));
  };

  return (
    <div className="home">
      {project ? (
        <div className="home-continue" data-coach="home-continue">
          <div className="home-continue-text">
            📁 <strong>{project.name}</strong>
            <span className="muted small">
              {project.takes.length === 0 ? t.homeNoTakesYet : latest ? t.homeContinueHint(takeName(latest, t.take)) : ""}
            </span>
          </div>
          <button className="primary" onClick={onContinue} disabled={busy || mode !== "idle"} title={t.homeContinueTitle}>
            {latest ? `▶ ${t.homeContinue}` : `● ${t.homeRecordFirst}`}
          </button>
          <button className="ghost" onClick={() => setScreen("start")} title={t.areaStartDesc}>
            {t.homeHaveMelody}
          </button>
        </div>
      ) : (
        <>
          <h2>{t.pickProjectFirst}</h2>
          <ProjectPicker />
        </>
      )}

      <nav className="home-areas" aria-label={t.phase}>
        {AREAS.map((a) => (
          <div key={a.id} className="home-area" data-area={a.id}>
            <button className="home-area-main" onClick={() => setScreen(a.main)} data-coach={`home-${a.id}`}>
              <span className="home-area-title">{t[a.title]}</span>
              <span className="muted small">{t[a.desc]}</span>
            </button>
            <div className="home-area-also">
              <span className="muted small">{t.homeAlso}</span>
              {a.also.map((sc) => {
                const key = SCREEN_TITLE[sc];
                return key ? (
                  <button key={sc} className="ghost small" onClick={() => setScreen(sc)}>
                    {t[key]}
                  </button>
                ) : null;
              })}
            </div>
          </div>
        ))}
      </nav>
    </div>
  );
}
