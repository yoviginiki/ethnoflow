import { api } from "./api";
import { useEffect } from "react";
import { AreaScreen } from "./AreaScreen";
import { Help } from "./Help";
import { Home } from "./Home";
import { NowPlayingBar } from "./NowPlayingBar";
import { PresetLibrary } from "./PresetLibrary";
import { ScreenBoundary } from "./ScreenBoundary";
import { useAssistantBridge } from "./assistant";
import { applySession, collectSession, useProjectSession } from "./session";
import { useBackgroundLayers, useProgressionFollow, useSongTransport } from "./layers";
import { ProjectMenu } from "./ProjectMenu";
import { ProjectPicker } from "./ProjectPicker";
import { GuidedBanner } from "./AcademyScreen";
import { useAcademyGuided } from "./academyGuided";
import { useLiveFollow } from "./LiveFollowPanel";
import { CoachPanel, useCoachHighlight } from "./CoachPanel";
import { messages, type Lang } from "./i18n";
import { areaOf, NO_PROJECT_SCREENS, SCREEN_TITLE } from "./areas";
import { useStore } from "./store";


export default function App() {
  const lang = useStore((s) => s.prefs.lang);
  const setPref = useStore((s) => s.setPref);
  const screen = useStore((s) => s.screen);
  const coachOn = useStore((s) => s.prefs.coach);
  const setScreen = useStore((s) => s.setScreen);
  const project = useStore((s) => s.project);
  const mode = useStore((s) => s.mode);
  const t = messages[lang];
  const error = useStore((s) => s.error);
  const area = screen === "home" ? null : areaOf(screen);
  const setError = useStore((s) => s.setError);
  useBackgroundLayers();
  useProgressionFollow();
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
                  {t.back} › {area && t[area.title] !== t[SCREEN_TITLE[screen]] && <>{t[area.title]} › </>}
                  <strong>{t[SCREEN_TITLE[screen]]}</strong>
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
          <button className={`ghost small ${screen === "help" ? "on" : ""}`} onClick={() => setScreen("help")} title={t.areaHelpDesc}>
            ? {t.helpButton}
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
      <ScreenBoundary screen={screen}>
      {screen === "home" && <Home />}
      {screen === "help" && <Help />}
      {area && screen !== "home" && (project || NO_PROJECT_SCREENS.includes(screen) ? <AreaScreen screen={screen} /> : <ProjectPicker />)}
      </ScreenBoundary>
      </div>
      {coachOn && <CoachPanel />}
      </div>
      <NowPlayingBar />
    </div>
  );
}
