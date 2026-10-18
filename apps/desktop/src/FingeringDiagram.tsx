import type { Fingering, FingeringChart } from "./fingerings";
import { fingeringsFor } from "./fingerings";
import { noteParts } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

interface Props {
  chart: FingeringChart;
  midi: number | null;
  /** Compact = one diagram without register text (for the tuner). */
  compact?: boolean;
  /** Show the alternatives next to the primary fingering. */
  showAlternatives?: boolean;
}

const HOLE_R = 7;
const GAP = 22;

/** The kaval drawn top-down: thumb hole (T) left of the tube, holes 2–8 down the front. */
function Tube({ f, label }: { f: Fingering; label?: string }) {
  const front = f.holes.slice(1).split("");
  const thumb = f.holes[0];
  const h = GAP * 8 + 10;
  const style = (s: string) => {
    switch (s) {
      case "c":
        return { fill: "var(--text)", stroke: "var(--text)", strokeDasharray: undefined as string | undefined };
      case "o":
        return { fill: "transparent", stroke: "var(--text)", strokeDasharray: undefined };
      case "x":
        return { fill: "transparent", stroke: "var(--muted)", strokeDasharray: "2 2" };
      default:
        return { fill: "url(#half)", stroke: "var(--text)", strokeDasharray: undefined };
    }
  };
  return (
    <svg className="fingering" width={64} height={h + (label ? 16 : 0)} viewBox={`0 0 64 ${h + (label ? 16 : 0)}`} aria-hidden="true">
      <defs>
        <linearGradient id="half" x1="0" x2="1">
          <stop offset="50%" stopColor="var(--text)" />
          <stop offset="50%" stopColor="transparent" />
        </linearGradient>
      </defs>
      <rect x={30} y={4} width={16} height={h - 8} rx={6} fill="none" stroke="var(--axis)" />
      <circle cx={16} cy={GAP} r={HOLE_R} {...style(thumb)} />
      <text x={16} y={GAP + 18} fontSize={9} textAnchor="middle" fill="var(--muted)">
        T
      </text>
      {front.map((s, i) => (
        <circle key={i} cx={38} cy={GAP * (i + 1)} r={HOLE_R} {...style(s)} />
      ))}
      {front.map((_, i) => (
        <text key={`n${i}`} x={54} y={GAP * (i + 1) + 3} fontSize={8} fill="var(--muted)">
          {i + 2}
        </text>
      ))}
      {label && (
        <text x={32} y={h + 10} fontSize={10} textAnchor="middle" fill="var(--muted)">
          {label}
        </text>
      )}
    </svg>
  );
}

/** Fingering of a note on the owner's kaval chart (F2.6): primary first, alternatives beside it. */
export function FingeringDiagram({ chart, midi, compact = false, showAlternatives = true }: Props) {
  const lang = useStore((s) => s.prefs.lang);
  const t = messages[lang];
  if (midi === null) return null;
  const list = fingeringsFor(chart, midi);
  if (list.length === 0) {
    return <div className="muted small">{t.fingeringNone}</div>;
  }
  const p = noteParts(midi);
  const shown = showAlternatives ? list : list.slice(0, 1);
  return (
    <div className={`fingering-box ${compact ? "compact" : ""}`} title={t.fingeringTitle}>
      <div className="fingering-row">
        {shown.map((f, i) => (
          <Tube key={i} f={f} label={compact ? undefined : f.alt ? t.fingeringAlt : chart.registers[String(f.register)]?.[lang]?.split(" (")[0]} />
        ))}
      </div>
      {!compact && (
        <div className="small">
          <b>
            {t.noteNames[p.pc]}
            {p.octave}
          </b>{" "}
          · {chart.registers[String(list[0].register)]?.[lang]}
          {list[0].note && <div className="muted">{list[0].note[lang]}</div>}
        </div>
      )}
    </div>
  );
}
