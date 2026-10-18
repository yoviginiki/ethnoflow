import { BackgroundTransport } from "./BackgroundTransport";
import { TransportBar } from "./TransportBar";
import { messages } from "./i18n";
import { RhythmPanel } from "./RhythmPanel";
import { useStore } from "./store";
import { TimingPanel } from "./TimingPanel";

/** The rhythm section as a screen of its own (also inside "Prepare background"). */
export function RhythmScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  return (
    <main className="compose">
      <section className="panel">
        <h2>{t.rhythmTitle}</h2>
        <div className="muted small">{t.rhythmScreenIntro}</div>
        {!project.scale && <div className="muted small">{t.noScaleYet}</div>}
      </section>
      <TimingPanel />
      <section className="panel analysis-row">
        <strong>{t.transportTitle}</strong>
        <TransportBar />
      </section>
      <RhythmPanel />
      <section className="panel">
        <h3>{t.backgroundTransport}</h3>
        <div className="analysis-row">
          <BackgroundTransport />
        </div>
      </section>
      <section className="panel analysis-row">
        <button className="ghost" onClick={() => s.setScreen("compose")}>
          {t.areaAccomp}
        </button>
        <button className="ghost" onClick={() => s.setScreen("play")}>
          {t.toStudio}
        </button>
        <button className="ghost" onClick={() => s.setScreen("studio")}>
          {t.areaStudio}
        </button>
      </section>
    </main>
  );
}
