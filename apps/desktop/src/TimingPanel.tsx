import { useEffect, useState } from "react";
import { api, describeError } from "./api";
import { formatGrouping, parseGrouping } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

const UNITS = [2, 4, 8, 16];

/** Tempo and meter are project data and start empty: nothing is assumed. */
export function TimingPanel() {
  const t = messages[useStore((s) => s.prefs.lang)];
  const project = useStore((s) => s.project)!;
  const prefs = useStore((s) => s.prefs);
  const setPref = useStore((s) => s.setPref);
  const setProject = useStore((s) => s.setProject);
  const setError = useStore((s) => s.setError);
  const mode = useStore((s) => s.mode);
  const timing = project.timing;

  const [bpm, setBpm] = useState("");
  const [num, setNum] = useState("");
  const [unit, setUnit] = useState("");
  const [group, setGroup] = useState("");

  useEffect(() => {
    setBpm(timing.bpm != null ? String(timing.bpm) : "");
    setNum(timing.numerator != null ? String(timing.numerator) : "");
    setUnit(timing.unit != null ? String(timing.unit) : "");
    setGroup(formatGrouping(timing.grouping));
  }, [timing.bpm, timing.numerator, timing.unit, timing.grouping]);

  const grouping = parseGrouping(group);
  const hasClick = timing.bpm != null && timing.numerator != null && timing.unit != null;

  const apply = async (e: React.FormEvent) => {
    e.preventDefault();
    if (grouping === null) return;
    try {
      setError(null);
      setProject(
        await api.setTiming({
          bpm: bpm.trim() === "" ? null : Number(bpm.replace(",", ".")),
          numerator: num.trim() === "" ? null : Number(num),
          unit: unit === "" ? null : Number(unit),
          grouping,
        }),
      );
    } catch (err) {
      setError(describeError(err));
    }
  };

  const busy = mode !== "idle";
  return (
    <details className="panel timing">
      <summary>
        {t.timing}:{" "}
        <span className="mono-num">
          {hasClick
            ? `${timing.bpm} · ${timing.numerator}/${timing.unit}${timing.grouping.length ? ` (${formatGrouping(timing.grouping)})` : ""}`
            : t.noTiming}
        </span>
      </summary>
      <form className="timing-form" onSubmit={apply}>
        <label>
          {t.tempo}
          <input inputMode="decimal" value={bpm} onChange={(e) => setBpm(e.target.value)} size={6} disabled={busy} />
        </label>
        <label>
          {t.meter}
          <span className="meter-inputs">
            <input inputMode="numeric" value={num} onChange={(e) => setNum(e.target.value)} size={3} aria-label={t.meter} disabled={busy} />
            /
            <select value={unit} onChange={(e) => setUnit(e.target.value)} aria-label={t.meter} disabled={busy}>
              <option value="">—</option>
              {UNITS.map((u) => (
                <option key={u} value={u}>
                  {u}
                </option>
              ))}
            </select>
          </span>
        </label>
        <label>
          {t.grouping}
          <input
            value={group}
            onChange={(e) => setGroup(e.target.value)}
            placeholder={t.groupingHint}
            size={10}
            aria-invalid={grouping === null}
            disabled={busy}
          />
        </label>
        <button type="submit" disabled={busy || grouping === null}>
          {t.apply}
        </button>
        <span className="muted small">{t.bpmUnitNote}</span>
      </form>
      <div className="timing-toggles">
        <label className="inline">
          <input type="checkbox" checked={prefs.clickRec} disabled={!hasClick || busy} onChange={(e) => setPref("clickRec", e.target.checked)} />
          {t.clickWhileRecording}
        </label>
        <label className="inline">
          {t.countIn}
          <select value={prefs.countIn} disabled={!hasClick || busy} onChange={(e) => setPref("countIn", Number(e.target.value))}>
            {[0, 1, 2, 4].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
        <label className="inline">
          <input type="checkbox" checked={prefs.clickPlay} disabled={!hasClick} onChange={(e) => setPref("clickPlay", e.target.checked)} />
          {t.clickOnPlayback}
        </label>
      </div>
    </details>
  );
}
