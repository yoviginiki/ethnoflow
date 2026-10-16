import { useCallback, useEffect, useRef } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, describeError, type WavFormat } from "./api";
import { messages } from "./i18n";
import { useStore } from "./store";
import { Waveform } from "./Waveform";
import { formatBytes, formatDb, formatTime, linearToDb, meterFraction } from "./format";

const WAVEFORM_BINS = 1600;
const STATUS_POLL_MS = 50;
/** Live waveform refresh while recording, in status ticks (4 × 50 ms = 5 Hz). */
const LIVE_WAVE_EVERY = 4;

export default function App() {
  const s = useStore();
  const t = messages[s.lang];
  const hostRef = useRef<string>("");

  const refreshDevices = useCallback(async () => {
    try {
      const [ins, outs, host] = await Promise.all([
        api.listInputDevices(),
        api.listOutputDevices(),
        api.audioHost(),
      ]);
      hostRef.current = `${host.host} / ${host.os}`;
      s.setDevices(ins, outs);
      if (ins.length === 0) s.setError(t.noInputDevices);
    } catch (e) {
      s.setError(describeError(e));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.setDevices, s.setError, t.noInputDevices]);

  const reloadTake = useCallback(async () => {
    try {
      const [info, peaks] = await Promise.all([api.takeInfo(), api.getWaveform(WAVEFORM_BINS)]);
      s.setTake(info, peaks);
    } catch (e) {
      s.setError(describeError(e));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.setTake, s.setError]);

  useEffect(() => {
    void refreshDevices();
    void reloadTake();
  }, [refreshDevices, reloadTake]);

  // Status polling while recording / playing.
  useEffect(() => {
    if (s.mode === "idle") return;
    let cancelled = false;
    let tickCount = 0;
    const tick = async () => {
      if (cancelled) return;
      try {
        if (s.mode === "recording") {
          const st = await api.recordingStatus();
          if (st) {
            s.setRecorder(st);
            const db = linearToDb(st.peak);
            // peak hold with decay
            s.setMeterDb(Math.max(db, useStore.getState().meterDb - 3));
            if (++tickCount % LIVE_WAVE_EVERY === 0) {
              s.setTake(null, await api.recordingWaveform(WAVEFORM_BINS));
            }
            if (st.capped || st.write_failed) {
              // File limit reached or disk write failed: stop; what is on disk is kept.
              await onStopRecord();
              s.setError(
                st.write_failed
                  ? t.writeFailed
                  : `${t.limitReached} (${Math.round(st.max_seconds / 60)} min)`,
              );
              return;
            }
          }
        } else if (s.mode === "playing") {
          const st = await api.playbackStatus();
          s.setPlayer(st);
          if (!st || st.finished) {
            s.setMode("idle");
            s.setPlayer(null);
            return;
          }
        }
      } catch (e) {
        s.setError(describeError(e));
        s.setMode("idle");
        return;
      }
      setTimeout(tick, STATUS_POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.mode]);

  const onRecord = async () => {
    try {
      s.setError(null);
      await api.startRecording(s.selectedInput);
      s.setTake(null, null);
      s.setPlayer(null);
      s.setMeterDb(-120);
      s.setMode("recording");
    } catch (e) {
      s.setError(describeError(e));
    }
  };

  const onStopRecord = async () => {
    try {
      const info = await api.stopRecording();
      s.setMode("idle");
      s.setRecorder(null);
      s.setMeterDb(-120);
      await reloadTake();
      s.setMessage(
        `${t.duration}: ${formatTime(info.duration_seconds)} · ${t.peak}: ${formatDb(info.peak_dbfs)}` +
          (info.clipped_samples > 0 ? ` · ${t.clip} ×${info.clipped_samples}` : ""),
      );
    } catch (e) {
      s.setError(describeError(e));
      s.setMode("idle");
    }
  };

  const onPlay = async (startSeconds = 0) => {
    try {
      s.setError(null);
      const st = await api.startPlayback(s.selectedOutput, startSeconds);
      s.setPlayer(st);
      s.setMode("playing");
    } catch (e) {
      s.setError(describeError(e));
    }
  };

  const onStopPlay = async () => {
    try {
      await api.stopPlayback();
    } finally {
      s.setMode("idle");
      s.setPlayer(null);
    }
  };

  const onExport = async () => {
    try {
      const path = await save({
        title: t.exportWav,
        defaultPath: `ethnoflow-take-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.wav`,
        filters: [{ name: "WAV", extensions: ["wav"] }],
      });
      if (!path) return;
      const r = await api.exportWav(path, s.exportFormat);
      s.setMessage(`${t.exported}: ${r.path} (${formatBytes(r.bytes)})`);
    } catch (e) {
      s.setError(describeError(e));
    }
  };

  const onImport = async () => {
    try {
      const path = await open({
        title: t.importWav,
        multiple: false,
        directory: false,
        filters: [{ name: "WAV", extensions: ["wav"] }],
      });
      if (!path || Array.isArray(path)) return;
      if (s.mode !== "idle") await onStopPlay();
      const info = await api.importWav(path);
      await reloadTake();
      s.setMessage(`${t.imported}: ${info.label ?? path} · ${formatTime(info.duration_seconds)}`);
    } catch (e) {
      s.setError(describeError(e));
    }
  };

  const onClear = async () => {
    if (s.mode !== "idle") return;
    await api.clearTake();
    s.setTake(null, null);
    s.setMessage(null);
  };

  const busy = s.mode !== "idle";
  const chLabel = (n: number) => (n === 1 ? t.mono : n === 2 ? t.stereo : `${n}`);

  return (
    <div className="app">
      <header className="topbar">
        <div>
          <h1>{t.appTitle}</h1>
          <div className="muted">{t.phase}</div>
        </div>
        <div className="topbar-right">
          <span className="muted small">
            {t.audioHost}: {hostRef.current || "…"}
          </span>
          <label className="lang">
            {t.language}
            <select value={s.lang} onChange={(e) => s.setLang(e.target.value as "bg" | "en")}>
              <option value="bg">BG</option>
              <option value="en">EN</option>
            </select>
          </label>
        </div>
      </header>

      <section className="devices">
        <label>
          {t.inputDevice}
          <select
            value={s.selectedInput ?? ""}
            disabled={busy}
            onChange={(e) => s.setSelectedInput(e.target.value || null)}
          >
            <option value="">{t.defaultDevice}</option>
            {s.inputDevices.map((d) => (
              <option key={d.name} value={d.name}>
                {d.name}
                {d.default_sample_rate ? ` · ${d.default_sample_rate} Hz` : ""}
                {d.default_channels ? ` · ${chLabel(d.default_channels)}` : ""}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t.outputDevice}
          <select
            value={s.selectedOutput ?? ""}
            disabled={busy}
            onChange={(e) => s.setSelectedOutput(e.target.value || null)}
          >
            <option value="">{t.defaultDevice}</option>
            {s.outputDevices.map((d) => (
              <option key={d.name} value={d.name}>
                {d.name}
                {d.default_sample_rate ? ` · ${d.default_sample_rate} Hz` : ""}
              </option>
            ))}
          </select>
        </label>
        <button onClick={refreshDevices} disabled={busy} className="ghost">
          {t.refresh}
        </button>
      </section>

      <section className="meter-row" aria-label={t.level}>
        <span className="muted small">{t.level}</span>
        <div className="meter">
          <div
            className={`meter-fill ${s.meterDb > -3 ? "hot" : s.meterDb > -12 ? "warm" : ""}`}
            style={{ width: `${meterFraction(s.meterDb) * 100}%` }}
          />
        </div>
        <span className="mono-num">{formatDb(s.meterDb)}</span>
        {s.recorder && s.recorder.clipped_samples > 0 && <span className="clip">{t.clip}</span>}
      </section>

      <Waveform
        peaks={s.peaks}
        playhead={s.mode === "playing" && s.player ? s.player.position_seconds : null}
        recordingSeconds={s.mode === "recording" && s.recorder ? s.recorder.seconds_captured : null}
        onSeek={(sec) => {
          if (s.mode === "idle" && s.take) void onPlay(sec);
        }}
        emptyText={t.noTake}
      />

      <section className="transport">
        {s.mode === "recording" ? (
          <button className="danger" onClick={onStopRecord}>
            ■ {t.stop}
          </button>
        ) : (
          <button className="rec" onClick={onRecord} disabled={busy}>
            ● {t.record}
          </button>
        )}
        {s.mode === "playing" ? (
          <button onClick={onStopPlay}>■ {t.pause}</button>
        ) : (
          <button onClick={() => onPlay(0)} disabled={busy || !s.take}>
            ▶ {t.play}
          </button>
        )}
        <span className="mono-num time">
          {s.mode === "recording" && s.recorder
            ? formatTime(s.recorder.seconds_captured)
            : s.mode === "playing" && s.player
              ? `${formatTime(s.player.position_seconds)} / ${formatTime(s.player.duration_seconds)}`
              : s.take
                ? formatTime(s.take.duration_seconds)
                : "0:00.0"}
        </span>
        <span className={`status ${s.mode}`}>
          {s.mode === "recording" ? t.recording : s.mode === "playing" ? t.playing : t.idle}
        </span>
        <span className="spacer" />
        <button className="ghost" onClick={onImport} disabled={busy}>
          {t.importWav}
        </button>
        <label className="inline">
          {t.format}
          <select
            value={s.exportFormat}
            onChange={(e) => s.setExportFormat(e.target.value as WavFormat)}
          >
            <option value="pcm24">24-bit PCM</option>
            <option value="float32">32-bit float</option>
            <option value="pcm16">16-bit PCM</option>
          </select>
        </label>
        <button onClick={onExport} disabled={busy || !s.take}>
          {t.exportWav}
        </button>
        <button className="ghost" onClick={onClear} disabled={busy || !s.take}>
          {t.clear}
        </button>
      </section>

      <section className="info">
        {s.take && (
          <>
            <span>
              {t.sampleRate}: <b>{s.take.sample_rate} Hz</b>
            </span>
            <span>
              {t.channels}: <b>{chLabel(s.take.channels)}</b>
            </span>
            <span>
              {t.peak}: <b>{formatDb(s.take.peak_dbfs)}</b>
            </span>
            <span>
              RMS: <b>{formatDb(s.take.rms_dbfs)}</b>
            </span>
            {s.take.clipped_samples > 0 && (
              <span className="clip">
                {t.clip} ×{s.take.clipped_samples}
              </span>
            )}
          </>
        )}
        {s.recorder && s.recorder.overrun_samples > 0 && (
          <span className="warn">
            {t.overruns}: {s.recorder.overrun_samples}
          </span>
        )}
        <span className="muted small">{s.take && s.mode === "idle" ? t.seekHint : t.headphonesHint}</span>
      </section>

      {s.error && (
        <div className="banner error" role="alert">
          {t.errorPrefix}: {s.error}
        </div>
      )}
      {!s.error && s.message && <div className="banner ok">{s.message}</div>}
    </div>
  );
}
