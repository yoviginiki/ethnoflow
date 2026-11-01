import { TransportBar } from "./TransportBar";
import { FollowMap } from "./FollowMap";
import { messages } from "./i18n";
import { RhythmPanel } from "./RhythmPanel";
import { useStore } from "./store";
import { TimingPanel } from "./TimingPanel";

/** The rhythm section: the "Rhythm" tab of Background (stage 4 of the UX audit). */
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
        <FollowMap />
      </section>
      <RhythmPanel />
      <section className="panel analysis-row">
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
