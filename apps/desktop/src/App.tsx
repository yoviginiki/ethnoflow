import { Ambient } from "./Ambient";
import { Arrange } from "./Arrange";
import { BackgroundTransport } from "./BackgroundTransport";
import { Compose } from "./Compose";
import { Home } from "./Home";
import { useBackgroundLayers } from "./layers";
import { ProjectPicker } from "./ProjectPicker";
import { Studio } from "./Studio";
import { messages, type Lang } from "./i18n";
import { useStore } from "./store";

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
            <div className="muted small">{t.phase}</div>
          </div>
        </div>
        <label className="inline">
          {t.language}
          <select value={lang} onChange={(e) => setPref("lang", e.target.value as Lang)}>
            <option value="bg">BG</option>
            <option value="en">EN</option>
          </select>
        </label>
      </header>

      {screen === "home" && <Home />}
      {screen === "play" && (project ? <Studio /> : <ProjectPicker />)}
      {screen === "compose" && (project ? <Compose /> : <ProjectPicker />)}
      {screen === "studio" && (project ? <Arrange /> : <ProjectPicker />)}
      {screen === "ambient" && (project ? <Ambient /> : <ProjectPicker />)}
      {(layers.drone || layers.pad || layers.ambient) && (
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
        </div>
      )}
    </div>
  );
}
