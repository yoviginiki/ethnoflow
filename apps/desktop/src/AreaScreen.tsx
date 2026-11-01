import { AcademyScreen } from "./AcademyScreen";
import { Ambient } from "./Ambient";
import { ALIAS_SCREENS, areaOf, areaTabs, SCREEN_TITLE, type AreaScreenId } from "./areas";
import { Arrange } from "./Arrange";
import { Compose } from "./Compose";
import { CreateSongScreen } from "./CreateSongScreen";
import { EditScreen } from "./EditScreen";
import { messages } from "./i18n";
import { InstrumentDesigner } from "./InstrumentDesigner";
import { Library } from "./Library";
import { LiveFollowPanel } from "./LiveFollowPanel";
import { ReferencesScreen } from "./ReferencesScreen";
import { RhythmScreen } from "./RhythmScreen";
import { SamplesScreen } from "./SamplesScreen";
import { ScapesScreen } from "./ScapesScreen";
import { ProgressionScreen } from "./ProgressionScreen";
import { SongMapScreen } from "./SongMapScreen";
import { Start } from "./Start";
import { useStore } from "./store";
import { Studio } from "./Studio";
import { TunerScreen } from "./TunerScreen";

/**
 * One area of Home as a screen with tabs (UX audit 2026-10-31, stages 4 and 5):
 * Play (Play & record, Tuner), Background (Drone & pad, Atmosphere, Soundscapes, Rhythm
 * section, Instruments, Live accompanist), Song (My studio, Song map, Create a song,
 * Start from a melody), Learn (Academy, References), Library (Library, My samples).
 * The existing screens mount inside unchanged; the tab is the screen id, so presets
 * (`go`), the assistant bridge, the coach and the smoke test keep their vocabulary.
 * Play / stop and the level of the background live in the "Now playing" bar, not here.
 */
export function AreaScreen({ screen }: { screen: AreaScreenId }) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const setScreen = useStore((s) => s.setScreen);
  const area = areaOf(screen);
  if (!area) return null;
  if (screen in ALIAS_SCREENS) screen = ALIAS_SCREENS[screen];
  return (
    <div className="area-screen" data-area={area.id}>
      <nav className="tabs" role="tablist" aria-label={t[area.title]} data-coach={`${area.id}-tabs`}>
        {areaTabs(area).map((sc) => (
          <button key={sc} role="tab" aria-selected={sc === screen} className={`tab ${sc === screen ? "on" : ""}`} onClick={() => setScreen(sc)}>
            {t[SCREEN_TITLE[sc]]}
          </button>
        ))}
        {area.id === "background" && <span className="muted small tabs-hint">{t.bgTabsHint}</span>}
      </nav>
      {screen === "play" && <Studio />}
      {screen === "tuner" && <TunerScreen />}
      {screen === "edit" && <EditScreen />}
      {screen === "compose" && <Compose />}
      {screen === "ambient" && <Ambient />}
      {screen === "scapes" && <ScapesScreen />}
      {screen === "progression" && <ProgressionScreen />}
      {screen === "rhythm" && <RhythmScreen />}
      {screen === "instrument" && <InstrumentDesigner />}
      {screen === "live" && (
        <main className="compose">
          <section className="panel">
            <h2>{t.liveTitle}</h2>
            <div className="muted small">{t.bgLiveIntro}</div>
          </section>
          <LiveFollowPanel />
        </main>
      )}
      {screen === "studio" && <Arrange />}
      {screen === "songmap" && <SongMapScreen />}
      {screen === "create" && <CreateSongScreen />}
      {screen === "start" && <Start />}
      {screen === "academy" && <AcademyScreen />}
      {screen === "references" && <ReferencesScreen />}
      {screen === "library" && <Library />}
      {screen === "samples" && <SamplesScreen />}
    </div>
  );
}
