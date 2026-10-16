import { messages, type StringMessageKey } from "./i18n";
import { useStore } from "./store";

interface Area {
  title: StringMessageKey;
  desc: StringMessageKey;
  /** Phase in which the area becomes available; null = available now. */
  phase: number | null;
}

// Master plan §9. Only "Play & record" exists in Phase 1; the rest are shown as
// planned, disabled — never as buttons that pretend to do something.
const AREAS: Area[] = [
  { title: "areaPlay", desc: "areaPlayDesc", phase: null },
  { title: "areaLearn", desc: "areaLearnDesc", phase: 6 },
  { title: "areaCreate", desc: "areaCreateDesc", phase: 5 },
  { title: "areaStudio", desc: "areaStudioDesc", phase: 5 },
  { title: "areaAccomp", desc: "areaAccompDesc", phase: 3 },
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
          <button key={a.title} className="area active" onClick={() => setScreen("play")}>
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
