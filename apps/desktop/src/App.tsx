import { Ambient } from "./Ambient";
import { Arrange } from "./Arrange";
import { BackgroundTransport } from "./BackgroundTransport";
import { Compose } from "./Compose";
import { Home } from "./Home";
import { useBackgroundLayers } from "./layers";
import { ProjectMenu } from "./ProjectMenu";
import { ProjectPicker } from "./ProjectPicker";
import { RhythmScreen } from "./RhythmScreen";
import { Studio } from "./Studio";
import { messages, type Lang, type StringMessageKey } from "./i18n";
import { useStore, type Screen } from "./store";

/** Title of each area, shown in the top bar so the user always sees where they are. */
const SCREEN_TITLE: Record<Exclude<Screen, "home">, StringMessageKey> = {
  play: "areaPlay",
  compose: "areaAccomp",
  studio: "areaStudio",
  ambient: "areaAmbient",
  rhythm: "areaRhythm",
};

export default function App() {
  const lang = useStore((s) => s.prefs.lang);
  const setPref = useStore((s) => s.setPref);
  const screen = useStore((s) => s.screen);
  const setScreen = useStore((s) => s.setScreen);
  const project = useStore((s) => s.project);
  const mode = useStore((s) => s.mode);
  const layers = useStore((s) => s.layers);
  const t = messages[lang];
  useBackgroundLayers();

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

      {screen === "home" && <Home />}
      {screen === "play" && (project ? <Studio /> : <ProjectPicker />)}
      {screen === "compose" && (project ? <Compose /> : <ProjectPicker />)}
      {screen === "studio" && (project ? <Arrange /> : <ProjectPicker />)}
      {screen === "ambient" && (project ? <Ambient /> : <ProjectPicker />)}
      {screen === "rhythm" && (project ? <RhythmScreen /> : <ProjectPicker />)}
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
