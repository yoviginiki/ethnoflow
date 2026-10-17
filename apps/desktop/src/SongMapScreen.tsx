import { useEffect, useState } from "react";
import { api, describeError, type ArrangeStatus } from "./api";
import { BackgroundTransport } from "./BackgroundTransport";
import { SongMapPanel } from "./SongMapPanel";
import { messages } from "./i18n";
import { useStore } from "./store";

/**
 * Song map screen (P3 Phase 1 slice): the editable form of the song with the studio
 * transport (clips) for position, play/pause and seek. The transport is still the
 * arrangement's seconds-based one; a unified beat transport is the next phase.
 */
export function SongMapScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [status, setStatus] = useState<ArrangeStatus | null>(null);
  const [cursor, setCursor] = useState(0);
  const output = s.prefs.selectedOutput;
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  useEffect(() => {
    let alive = true;
    const tick = () => {
      api
        .arrangementStatus()
        .then((st) => alive && setStatus(st))
        .catch(() => undefined);
    };
    tick();
    const id = window.setInterval(tick, 150);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, []);

  const playing = !!status?.playing;
  const head = playing && status ? status.position_seconds : cursor;
  const play = () => api.arrangementPlay(output, cursor).then(setStatus).catch(fail);
  const pause = () =>
    api
      .arrangementPause()
      .then((st) => {
        setStatus(st);
        if (st) setCursor(st.position_seconds);
      })
      .catch(fail);
  const seek = (seconds: number) => {
    setCursor(seconds);
    if (playing) void api.arrangementSeek(seconds).then(setStatus).catch(fail);
  };

  return (
    <section className="songmap-screen">
      <div className="muted small">{t.songMapIntro}</div>
      <div className="analysis-row">
        {playing ? (
          <button className="primary" onClick={() => void pause()}>
            ⏸ {t.pause}
          </button>
        ) : (
          <button className="primary" onClick={() => void play()} disabled={!s.project}>
            ▶ {t.play}
          </button>
        )}
        <button className="ghost" onClick={() => seek(0)} title={t.toStart}>
          ⏮
        </button>
        <span className="mono-num">{head.toFixed(1)} s</span>
        <BackgroundTransport compact />
      </div>
      <div className="panel">
        <SongMapPanel headSeconds={head} onSeek={seek} />
      </div>
    </section>
  );
}
