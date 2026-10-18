import { useState } from "react";
import { api, describeError } from "./api";
import { BackgroundTransport } from "./BackgroundTransport";
import { SongMapPanel } from "./SongMapPanel";
import { TransportBar } from "./TransportBar";
import { FollowMap } from "./FollowMap";
import { messages } from "./i18n";
import { useStore } from "./store";

/**
 * Song map screen (P3 Phase 1): the editable form of the song on the **song transport**
 * (one position for clips, rhythm and the map). Clicking the ruler seeks the transport;
 * the playhead follows it whether the timeline or only the rhythm is running.
 */
export function SongMapScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const transport = s.transport;
  const [cursor, setCursor] = useState(0);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  const head = transport && (transport.playing || transport.source !== "idle") ? transport.seconds : cursor;
  const seek = (seconds: number) => {
    setCursor(seconds);
    void api.transportSeek(seconds).then(s.setTransport).catch(fail);
  };

  return (
    <section className="songmap-screen">
      <div className="muted small">{t.songMapIntro}</div>
      <div className="analysis-row">
        <TransportBar />
        <BackgroundTransport compact />
      </div>
      <div className="panel analysis-row">
        <FollowMap />
      </div>
      <div className="muted small">{t.songMapHowTo}</div>
      <div className="panel">
        <SongMapPanel headSeconds={head} onSeek={seek} />
      </div>
    </section>
  );
}
