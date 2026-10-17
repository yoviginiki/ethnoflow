import { messages } from "./i18n";
import { useStore } from "./store";

/**
 * Standalone metronome (a click on its own, without recording or playback). The
 * stream itself is run by `useBackgroundLayers` from `metronomeOn`; this is only the
 * switch, shown next to the tempo and in the studio transport.
 */
export function MetronomeButton({ compact = false }: { compact?: boolean }) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const project = useStore((s) => s.project);
  const on = useStore((s) => s.metronomeOn);
  const setOn = useStore((s) => s.setMetronomeOn);
  const mode = useStore((s) => s.mode);
  const tm = project?.timing;
  const canClick = !!tm && tm.bpm !== null && tm.numerator !== null && tm.unit !== null;
  return (
    <button
      className={`ghost ${compact ? "small" : ""} ${on ? "on" : ""}`}
      disabled={!canClick || mode === "recording"}
      title={canClick ? (on ? t.metronomeStop : t.metronomeStart) : t.noTiming}
      aria-pressed={on}
      onClick={(e) => {
        // Inside a <summary> a click would also toggle the details: keep it to the button.
        e.preventDefault();
        e.stopPropagation();
        setOn(!on);
      }}
    >
      {on ? "■" : "▶"} ♩ {compact ? "" : t.metronome}
    </button>
  );
}
