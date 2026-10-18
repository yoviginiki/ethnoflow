import { api, describeError } from "./api";
import { formatTime } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

const LOOP_CHOICES = [0, 2, 4, 8, 16];

/**
 * Song transport (P3 Phase 1): one position for the whole song. Play/stop drive the
 * arrangement timeline; a running rhythm section is re-anchored to the same bar by the
 * backend; the position is shown as bar:beat (meter map) and time. The loop is in bars
 * from the current bar and is stored in the arrangement (the studio shows it too).
 */
export function TransportBar({ compact = false }: { compact?: boolean }) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const transport = useStore((s) => s.transport);
  const project = useStore((s) => s.project);
  const output = useStore((s) => s.prefs.selectedOutput);
  const setTransport = useStore((s) => s.setTransport);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));
  const playing = !!transport?.playing;
  const seconds = transport?.seconds ?? 0;
  const loopBars = transport?.loop_bars ?? null;
  const loopLen = loopBars ? Number(loopBars[1] - loopBars[0]) : 0;

  const play = () => api.transportPlay(output, null).then(setTransport).catch(fail);
  const stop = () => api.transportStop().then(setTransport).catch(fail);
  const toStart = () => api.transportSeek(0).then(setTransport).catch(fail);
  const setLoop = (bars: number) => {
    const from = transport?.bar ?? 1;
    (bars > 0 ? api.transportLoop(from, bars) : api.transportLoop(null, null)).then(setTransport).catch(fail);
  };

  return (
    <span className={"song-transport" + (compact ? " compact" : "")} title={t.transportHint}>
      <button className="ghost" onClick={() => void toStart()} title={t.toStart} aria-label={t.toStart}>
        ⏮
      </button>
      {playing ? (
        <button className="primary" onClick={() => void stop()}>
          ⏸ {t.pause}
        </button>
      ) : (
        <button className="primary" onClick={() => void play()} disabled={!project}>
          ▶ {t.play}
        </button>
      )}
      <span className="mono-num song-pos" aria-live="off">
        {transport ? `${transport.bar}.${transport.beat}` : "–"} <span className="muted small">/{transport?.beats_in_bar ?? "–"}</span>
      </span>
      <span className="mono-num small muted">{formatTime(seconds)}</span>
      {!compact && transport && transport.bpm > 0 && (
        <span className="small muted">
          {Math.round(transport.bpm)} BPM · {transport.numerator}/{transport.unit}
        </span>
      )}
      <label className="inline small" title={t.loopBarsHint}>
        🔁
        <select value={loopLen} onChange={(e) => setLoop(Number(e.target.value))} disabled={!project}>
          {LOOP_CHOICES.map((n) => (
            <option key={n} value={n}>
              {n === 0 ? t.loopOff : `${n} ${t.refBars}`}
            </option>
          ))}
        </select>
        {loopBars && (
          <span className="mono-num muted">
            {loopBars[0]}–{Number(loopBars[1]) - 1}
          </span>
        )}
      </label>
      {!compact && transport?.source === "rhythm" && <span className="small muted">{t.transportFromRhythm}</span>}
    </span>
  );
}
