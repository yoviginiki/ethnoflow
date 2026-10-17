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
  const masterDb = useStore((s) => s.prefs.masterDb ?? 0);
  const setLayer = useStore((s) => s.setLayer);
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
      {(layers.drone || layers.pad) && (
        <div className="layers-badge small" role="status">
          <label className="inline">
            <input type="checkbox" checked={layers.drone} onChange={(e) => setLayer("drone", e.target.checked)} />
            {t.drone}
          </label>
          <label className="inline">
            <input type="checkbox" checked={layers.pad} onChange={(e) => setLayer("pad", e.target.checked)} />
            {t.pad}
          </label>
          <label className="inline" title={t.backgroundLevel}>
            🔊
            <input type="range" min={-40} max={6} step={1} value={masterDb} onChange={(e) => setPref("masterDb", Number(e.target.value))} style={{ width: 90 }} />
            <span className="mono-num">{masterDb > 0 ? "+" : ""}{masterDb} dB</span>
          </label>
          <button className="ghost small" onClick={() => setScreen(screen === "compose" ? "play" : "compose")}>
            {screen === "compose" ? t.toStudio : t.areaAccomp}
          </button>
          <button
            className="ghost small"
            onClick={() => {
              setLayer("drone", false);
              setLayer("pad", false);
            }}
          >
            {t.stopLayers}
          </button>
        </div>
      )}
    </div>
  );
}
