import { useCallback } from "react";
import { api, describeError } from "./api";
import { BackgroundTransport } from "./BackgroundTransport";
import { formatTime, takeName } from "./format";
import { messages } from "./i18n";
import { MetronomeButton } from "./MetronomeButton";
import { playNotes, stopAll, stopNotes, useNowPlayingPolls } from "./nowPlaying";
import { useStore } from "./store";
import { playLoaded, stopPlaying } from "./takeActions";

/**
 * "Now playing": one bar at the bottom of every screen with every source of sound as a
 * chip (take, notes, background, song, metronome), "Stop all", and the background's
 * level and mixer. Replaces the floating layers badge that only appeared while a
 * background played (UX audit 2026-10-31, stage 2). Each chip plays or stops its
 * source with one click and shows its position while it sounds.
 */
export function NowPlayingBar() {
  useNowPlayingPolls();
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project;
  const fail = useCallback((e: unknown) => useStore.getState().setError(describeError(e)), []);

  const loadedTake = project?.takes.find((tk) => tk.id === s.loaded?.id) ?? null;
  const takePlaying = s.mode === "playing";
  const recording = s.mode === "recording";
  const notesOn = s.notesPlayer !== null;
  const songOn = !!s.transport?.playing;
  const bg = s.layers;
  const anySound = recording || takePlaying || notesOn || songOn || bg.drone || bg.pad || bg.ambient || bg.rhythm || bg.scapes || s.metronomeOn || s.liveRunning;

  const onTake = () => {
    if (takePlaying) return void stopPlaying().catch(fail);
    if (s.loaded) return void playLoaded(undefined, false).catch(fail);
    s.setScreen("play");
  };
  const onNotes = () => {
    if (notesOn) return void stopNotes();
    if (s.analysis && s.analysis.notes.length > 0 && s.mode === "idle") return void playNotes().catch(fail);
    s.setScreen("play");
  };
  const onSong = () => {
    if (songOn) return void api.transportStop().then((st) => useStore.getState().setTransport(st)).catch(fail);
    void api.transportPlay(s.prefs.selectedOutput, null).then((st) => useStore.getState().setTransport(st)).catch(fail);
  };

  if (!project) {
    return (
      <div className="now-playing" role="region" aria-label={t.nowPlaying}>
        <span className="muted small">{t.npNoProject}</span>
      </div>
    );
  }

  const takeLabel = loadedTake ? takeName(loadedTake, t.take) : t.npNoTake;
  return (
    <div className="now-playing" role="region" aria-label={t.nowPlaying} data-coach="now-playing">
      <span className="muted small np-title">{t.nowPlaying}</span>

      {recording ? (
        <button className="np-chip rec on" onClick={() => s.setScreen("play")} title={t.npToPlay}>
          ● {t.recording} <span className="mono-num">{s.rec ? formatTime(s.rec.seconds_captured) : ""}</span>
        </button>
      ) : (
        <button className={`np-chip ${takePlaying ? "on" : ""}`} onClick={onTake} disabled={s.pending !== null} title={takePlaying ? t.pause : s.loaded ? t.play : t.npToPlay}>
          {takePlaying ? "■" : "▶"} {takeLabel}
          {takePlaying && s.player && (
            <span className="mono-num">
              {" "}
              {formatTime(s.player.position_seconds)} / {formatTime(s.player.duration_seconds)}
            </span>
          )}
        </button>
      )}

      <button className={`np-chip ${notesOn ? "on" : ""}`} onClick={onNotes} disabled={recording} title={notesOn ? t.stopNotes : t.playNotes}>
        {notesOn ? "■" : "♪"} {t.npNotes}
        {notesOn && s.notesPlayer && <span className="mono-num"> {formatTime(s.notesPlayer.position_seconds)}</span>}
      </button>

      <span className={`np-chip np-bg ${bg.drone || bg.pad || bg.ambient || bg.rhythm || bg.scapes ? "on" : ""}`}>
        <BackgroundTransport compact />
      </span>

      <button className={`np-chip ${songOn ? "on" : ""}`} onClick={onSong} title={songOn ? t.pause : t.play}>
        {songOn ? "■" : "▶"} {t.npSong}
        {songOn && s.transport && (
          <span className="mono-num">
            {" "}
            {s.transport.bar}.{s.transport.beat}
          </span>
        )}
      </button>

      <span className="np-chip">
        <MetronomeButton compact />
      </span>

      {s.liveRunning && <span className="np-chip on">{t.npLive}</span>}

      <button className={`np-chip ${s.prefs.keyboardOn ? "on" : ""}`} onClick={() => s.setPref("keyboardOn", !s.prefs.keyboardOn)} title={t.kbToggleHint} aria-pressed={s.prefs.keyboardOn} data-coach="keyboard-toggle">
        ⌨ {t.kbDock}
      </button>

      <span className="spacer" />
      <button className={anySound ? "danger" : "ghost"} disabled={!anySound} onClick={() => void stopAll().catch(fail)} title={t.npStopAllHint}>
        ■ {t.npStopAll}
      </button>
      {!anySound && <span className="muted small">{t.npSilent}</span>}
    </div>
  );
}
