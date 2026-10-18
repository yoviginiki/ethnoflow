import { api } from "./api";
import { useEffect } from "react";
import { Ambient } from "./Ambient";
import { Arrange } from "./Arrange";
import { BackgroundTransport } from "./BackgroundTransport";
import { Compose } from "./Compose";
import { Help } from "./Help";
import { Home } from "./Home";
import { Library } from "./Library";
import { PresetLibrary } from "./PresetLibrary";
import { useAssistantBridge } from "./assistant";
import { applySession, collectSession, useProjectSession } from "./session";
import { useBackgroundLayers, useSongTransport } from "./layers";
import { ProjectMenu } from "./ProjectMenu";
import { ProjectPicker } from "./ProjectPicker";
import { RhythmScreen } from "./RhythmScreen";
import { Soundscapes } from "./Soundscapes";
import { Start } from "./Start";
import { InstrumentDesigner } from "./InstrumentDesigner";
import { SongMapScreen } from "./SongMapScreen";
import { SamplesScreen } from "./SamplesScreen";
import { Studio } from "./Studio";
import { ReferencesScreen } from "./ReferencesScreen";
import { AcademyScreen, GuidedBanner } from "./AcademyScreen";
import { CreateSongScreen } from "./CreateSongScreen";
import { useAcademyGuided } from "./academyGuided";
import { useLiveFollow } from "./LiveFollowPanel";
import { CoachPanel, useCoachHighlight } from "./CoachPanel";
import { messages, type Lang, type StringMessageKey } from "./i18n";
import { useStore, type Screen } from "./store";

/** Title of each area, shown in the top bar so the user always sees where they are. */
const SCREEN_TITLE: Record<Exclude<Screen, "home">, StringMessageKey> = {
  play: "areaPlay",
  compose: "areaAccomp",
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
  help: "areaHelp",
};

export default function App() {
  const lang = useStore((s) => s.prefs.lang);
  const setPref = useStore((s) => s.setPref);
  const screen = useStore((s) => s.screen);
  const coachOn = useStore((s) => s.prefs.coach);
  const setScreen = useStore((s) => s.setScreen);
  const project = useStore((s) => s.project);
  const mode = useStore((s) => s.mode);
  const layers = useStore((s) => s.layers);
  const t = messages[lang];
  const error = useStore((s) => s.error);
  const setError = useStore((s) => s.setError);
  useBackgroundLayers();
  useSongTransport();
  useAcademyGuided();
  useLiveFollow();
  useCoachHighlight();
  useEffect(() => {
    void api
      .fingeringOverridesGet("kaval-d")
      .then((list) => useStore.getState().setFingeringOverrides("kaval-d", list))
      .catch(() => undefined);
  }, []);
  useProjectSession();
  useAssistantBridge();

  return (
    <div className="app">
      <header className="topbar">
        <div className="topbar-left">
          {screen !== "home" && (
            <button
              className="ghost"
              onClick={() => setScreen("home")}
              disabled={mode !== "idle"}
              aria-label={t.back}
            >
              ← {t.back}
            </button>
          )}
          <div>
            <h1>{t.appTitle}</h1>
            <div className="breadcrumb" aria-live="polite">
              {screen === "home" ? (
                t.phase
              ) : (
                <>
                  {t.back} › <strong>{t[SCREEN_TITLE[screen]]}</strong>
                </>
              )}
            </div>
          </div>
        </div>
        <div className="topbar-right">
          {project && (
            <div className="session-bar" title={t.libSessionHint}>
              <span className="muted small">{t.libSession}</span>
              <PresetLibrary kind="session" compact getPayload={collectSession} apply={(p) => void applySession(p, true).catch((e) => useStore.getState().setError(String(e)))} />
            </div>
          )}
          <button className={`ghost small ${coachOn ? "on" : ""}`} onClick={() => setPref("coach", !coachOn)} title={t.coachToggleHint}>
            🧭 {t.coachTitle}
          </button>
          <ProjectMenu />
          <label className="inline">
            {t.language}
            <select value={lang} onChange={(e) => setPref("lang", e.target.value as Lang)}>
              <option value="bg">BG</option>
              <option value="en">EN</option>
            </select>
          </label>
        </div>
      </header>
      <GuidedBanner />
      {error && screen !== "studio" && (
        <div className="banner error" role="alert">
          {error}{" "}
          <button className="ghost small" onClick={() => setError(null)} aria-label={t.cancel}>
            ✕
          </button>
        </div>
      )}

      <div className={`app-body ${coachOn ? "with-coach" : ""}`}>
      <div className="app-main">
      {screen === "home" && <Home />}
      {screen === "play" && (project ? <Studio /> : <ProjectPicker />)}
      {screen === "compose" && (project ? <Compose /> : <ProjectPicker />)}
      {screen === "studio" && (project ? <Arrange /> : <ProjectPicker />)}
      {screen === "ambient" && (project ? <Ambient /> : <ProjectPicker />)}
      {screen === "rhythm" && (project ? <RhythmScreen /> : <ProjectPicker />)}
      {screen === "library" && <Library />}
      {screen === "soundscapes" && (project ? <Soundscapes /> : <ProjectPicker />)}
      {screen === "start" && (project ? <Start /> : <ProjectPicker />)}
      {screen === "help" && <Help />}
      {screen === "instrument" && (project ? <InstrumentDesigner /> : <ProjectPicker />)}
      {screen === "songmap" && (project ? <SongMapScreen /> : <ProjectPicker />)}
      {screen === "samples" && <SamplesScreen />}
      {screen === "references" && <ReferencesScreen />}
      {screen === "academy" && <AcademyScreen />}
      {screen === "create" && <CreateSongScreen />}
      </div>
      {coachOn && <CoachPanel />}
      </div>
      {(layers.drone || layers.pad || layers.ambient || layers.rhythm) && (
        <div className="layers-badge small" role="status">
          <BackgroundTransport compact />
          <button className="ghost small" disabled={mode !== "idle"} onClick={() => setScreen(screen === "compose" ? "play" : "compose")}>
            {screen === "compose" ? t.toStudio : t.areaAccomp}
          </button>
          {screen !== "studio" && (
            <button className="ghost small" disabled={mode !== "idle"} onClick={() => setScreen("studio")}>
              {t.areaStudio}
            </button>
          )}
          {screen !== "ambient" && (
            <button className="ghost small" disabled={mode !== "idle"} onClick={() => setScreen("ambient")}>
              {t.areaAmbient}
            </button>
          )}
          {screen !== "soundscapes" && (
            <button className="ghost small" disabled={mode !== "idle"} onClick={() => setScreen("soundscapes")}>
              {t.areaSoundscapes}
            </button>
          )}
          {screen !== "rhythm" && (
            <button className="ghost small" disabled={mode !== "idle"} onClick={() => setScreen("rhythm")}>
              {t.rhythmShort}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
