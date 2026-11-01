import { Ambient } from "./Ambient";
import { Compose } from "./Compose";
import { messages, type StringMessageKey } from "./i18n";
import { InstrumentDesigner } from "./InstrumentDesigner";
import { LiveFollowPanel } from "./LiveFollowPanel";
import { RhythmScreen } from "./RhythmScreen";
import { Soundscapes } from "./Soundscapes";
import { useStore, type Screen } from "./store";

/** The tabs of "Background", in order; each tab keeps its old `Screen` id. */
export const BACKGROUND_TABS: { screen: Screen; title: StringMessageKey }[] = [
  { screen: "compose", title: "bgTabDronePad" },
  { screen: "ambient", title: "areaAmbient" },
  { screen: "soundscapes", title: "areaSoundscapes" },
  { screen: "rhythm", title: "areaRhythm" },
  { screen: "instrument", title: "areaForge" },
  { screen: "live", title: "bgTabLive" },
];

/**
 * "Background": everything that sounds behind the melody, as one screen with tabs
 * (UX audit 2026-10-31, stage 4): Drone & pad (the former Compose), Atmosphere,
 * Soundscapes, Rhythm section, Instruments and the Live accompanist. The existing
 * screens mount inside unchanged; the tab is the screen id, so presets (`go`), the
 * assistant bridge, the coach and the smoke test keep their vocabulary. Play / stop and
 * the level of the background live in the "Now playing" bar, not here.
 */
export function BackgroundScreen({ tab }: { tab: Screen }) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const setScreen = useStore((s) => s.setScreen);
  return (
    <div className="background-screen">
      <nav className="tabs" role="tablist" aria-label={t.hBackground} data-coach="background-tabs">
        {BACKGROUND_TABS.map((tb) => (
          <button key={tb.screen} role="tab" aria-selected={tb.screen === tab} className={`tab ${tb.screen === tab ? "on" : ""}`} onClick={() => setScreen(tb.screen)}>
            {t[tb.title]}
          </button>
        ))}
        <span className="muted small tabs-hint">{t.bgTabsHint}</span>
      </nav>
      {tab === "compose" && <Compose />}
      {tab === "ambient" && <Ambient />}
      {tab === "soundscapes" && <Soundscapes />}
      {tab === "rhythm" && <RhythmScreen />}
      {tab === "instrument" && <InstrumentDesigner />}
      {tab === "live" && (
        <main className="compose">
          <section className="panel">
            <h2>{t.liveTitle}</h2>
            <div className="muted small">{t.bgLiveIntro}</div>
          </section>
          <LiveFollowPanel />
        </main>
      )}
    </div>
  );
}
