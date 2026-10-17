import { messages, type StringMessageKey } from "./i18n";
import { useStore, type Screen } from "./store";

interface Area {
  title: StringMessageKey;
  desc: StringMessageKey;
  /** Phase in which the area becomes available; null = available now. */
  phase: number | null;
  screen?: Screen;
}

// Master plan §9. "Play & record" (Phase 1) and the accompanist's Compose First
// (Phase 3) exist; the rest are shown as planned, disabled — never as buttons that
// pretend to do something.
const AREAS: Area[] = [
  { title: "areaPlay", desc: "areaPlayDesc", phase: null, screen: "play" },
  { title: "areaAccomp", desc: "areaAccompDesc", phase: null, screen: "compose" },
  { title: "areaLearn", desc: "areaLearnDesc", phase: 6 },
  { title: "areaCreate", desc: "areaCreateDesc", phase: 5 },
  { title: "areaStudio", desc: "areaStudioDesc", phase: 5 },
  { title: "areaSamples", desc: "areaSamplesDesc", phase: 5 },
  { title: "areaHelp", desc: "areaHelpDesc", phase: 6 },
];

export function Home() {
  const t = messages[useStore((s) => s.prefs.lang)];
  const setScreen = useStore((s) => s.setScreen);
  return (
    <nav className="home-grid" aria-label={t.appTitle}>
      {AREAS.map((a) =>
        a.phase === null ? (
          <button key={a.title} className="area active" onClick={() => setScreen(a.screen ?? "play")}>
            <span className="area-title">{t[a.title]}</span>
            <span className="muted small">{t[a.desc]}</span>
          </button>
        ) : (
          <div key={a.title} className="area" aria-disabled="true">
            <span className="area-title">{t[a.title]}</span>
            <span className="muted small">{t[a.desc]}</span>
            <span className="badge">
              {t.availableFromPhase} {a.phase}
            </span>
          </div>
        ),
      )}
    </nav>
  );
}
