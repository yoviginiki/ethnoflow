import { useCallback, useEffect, useRef } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, describeError, IMPORT_EXTENSIONS, type WavFormat } from "./api";
import { formatBytes, formatDb, formatTime, isTypingTarget, linearToDb, meterFraction, takeName } from "./format";
import { messages } from "./i18n";
import { TakeList } from "./TakeList";
import { TimingPanel } from "./TimingPanel";
import { useStore } from "./store";
import { Waveform } from "./Waveform";

const WAVEFORM_BINS = 1600;
const STATUS_POLL_MS = 50;
/** Live waveform refresh while recording, in status ticks (4 × 50 ms = 5 Hz). */
const LIVE_WAVE_EVERY = 4;

export function Studio() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const timing = project.timing;
  const hasClick = timing.bpm != null && timing.numerator != null && timing.unit != null;
  const loadedTake = project.takes.find((tk) => tk.id === s.loaded?.id) ?? null;
  const modeRef = useRef(s.mode);
  modeRef.current = s.mode;

  const fail = useCallback((e: unknown) => useStore.getState().setError(describeError(e)), []);

  const refreshDevices = useCallback(async () => {
    try {
      const [ins, outs, host] = await Promise.all([api.listInputDevices(), api.listOutputDevices(), api.audioHost()]);
      useStore.getState().setDevices(ins, outs, `${host.host} / ${host.os}`);
      if (ins.length === 0) useStore.getState().setError(messages[useStore.getState().prefs.lang].noInputDevices);
    } catch (e) {
      fail(e);
    }
  }, [fail]);

  useEffect(() => {
    void refreshDevices();
  }, [refreshDevices]);

  const selectTake = useCallback(
    async (id: string) => {
      try {
        const st = useStore.getState();
        st.setError(null);
        if (st.mode === "playing") {
          await api.stopPlayback();
          st.setMode("idle");
          st.setPlayer(null);
        }
        const info = await api.loadTake(id);
        const peaks = await api.getWaveform(WAVEFORM_BINS);
        useStore.getState().setLoaded(info, peaks);
      } catch (e) {
        fail(e);
        try {
          useStore.getState().setProject(await api.projectState());
        } catch {
          /* keep the current view */
        }
      }
    },
    [fail],
  );

  // ---------- recording ----------

  const onStopRecord = useCallback(async () => {
    const st = useStore.getState();
    const tt = messages[st.prefs.lang];
    try {
      const r = await api.stopRecording();
      st.setMode("idle");
      st.setRec(null);
      st.setMeterDb(-120);
      st.setProject(await api.projectState());
      await selectTake(r.take.id);
      const parts = [
        `${takeName(r.take, tt.take)}`,
        `${tt.duration}: ${formatTime(r.take.duration_seconds ?? 0)}`,
      ];
      const info = useStore.getState().loaded;
      if (info) parts.push(`${tt.peak}: ${formatDb(info.peak_dbfs)}`);
      if (r.take.clipped_samples > 0) parts.push(`${tt.clip} ×${r.take.clipped_samples}`);
      if (r.frames_lost > 0) parts.push(`${r.frames_lost} ${tt.framesLost}`);
      useStore.getState().setMessage(parts.join(" · "));
      if (r.device_lost) useStore.getState().setError(tt.deviceLost);
      else if (r.write_error) useStore.getState().setError(`${tt.writeFailed} (${r.write_error})`);
    } catch (e) {
      fail(e);
      st.setMode("idle");
    }
  }, [fail, selectTake]);

  const onRecord = useCallback(async () => {
    const st = useStore.getState();
    try {
      st.setError(null);
      st.setMessage(null);
      const p = st.prefs;
      const clickable = hasClick;
      await api.startRecording({
        input: p.selectedInput,
        output: p.selectedOutput,
        sample_rate: p.sampleRate,
        click: clickable && p.clickRec,
        count_in_bars: clickable ? p.countIn : 0,
      });
      st.setLoaded(null, null);
      st.setPlayer(null);
      st.setMeterDb(-120);
      st.setMode("recording");
    } catch (e) {
      fail(e);
    }
  }, [fail, hasClick]);

  // ---------- playback ----------

  const onPlay = useCallback(
    async (from?: number) => {
      const st = useStore.getState();
      if (!st.loaded) return;
      try {
        st.setError(null);
        const start = from ?? st.cursor;
        const ps = await api.startPlayback({
          device: st.prefs.selectedOutput,
          start_seconds: start >= st.loaded.duration_seconds - 0.01 ? 0 : start,
          loop_seconds: st.prefs.loop ? [0, st.loaded.duration_seconds] : null,
          click: hasClick && st.prefs.clickPlay,
        });
        st.setPlayer(ps);
        st.setMode("playing");
      } catch (e) {
        fail(e);
      }
    },
    [fail, hasClick],
  );

  const onStopPlay = useCallback(async () => {
    const st = useStore.getState();
    try {
      await api.stopPlayback();
    } finally {
      if (st.player) st.setCursor(Math.min(st.player.position_seconds, st.loaded?.duration_seconds ?? 0));
      st.setMode("idle");
      st.setPlayer(null);
    }
  }, []);

  // Loop / click toggles while playing.
  useEffect(() => {
    if (modeRef.current !== "playing" || !s.loaded) return;
    void api.setPlaybackLoop(s.prefs.loop ? [0, s.loaded.duration_seconds] : null).catch(fail);
  }, [s.prefs.loop, s.loaded, fail]);
  useEffect(() => {
    if (modeRef.current !== "playing") return;
    // The click is built when playback starts: restart from the current position.
    const pos = useStore.getState().player?.position_seconds ?? 0;
    void onPlay(pos);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.prefs.clickPlay]);

  // ---------- status polling ----------

  useEffect(() => {
    if (s.mode === "idle") return;
    let cancelled = false;
    let tick = 0;
    const loop = async () => {
      if (cancelled) return;
      const st = useStore.getState();
      const tt = messages[st.prefs.lang];
      try {
        if (st.mode === "recording") {
          const r = await api.recordingStatus();
          if (r) {
            st.setRec(r);
            st.setMeterDb(Math.max(linearToDb(r.peak), useStore.getState().meterDb - 3));
            if (++tick % LIVE_WAVE_EVERY === 0) st.setPeaks(await api.recordingWaveform(WAVEFORM_BINS));
            if (r.capped || r.write_failed || r.device_lost) {
              await onStopRecord();
              useStore
                .getState()
                .setError(r.device_lost ? tt.deviceLost : r.write_failed ? tt.writeFailed : `${tt.limitReached} (${Math.round(r.max_seconds / 60)} min)`);
              return;
            }
          }
        } else if (st.mode === "playing") {
          const p = await api.playbackStatus();
          st.setPlayer(p);
          if (!p || p.finished) {
            st.setMode("idle");
            st.setPlayer(null);
            st.setCursor(0);
            return;
          }
        }
      } catch (e) {
        fail(e);
        useStore.getState().setMode("idle");
        return;
      }
      setTimeout(loop, STATUS_POLL_MS);
    };
    void loop();
    return () => {
      cancelled = true;
    };
  }, [s.mode, onStopRecord, fail]);

  // ---------- keyboard ----------

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTypingTarget(e.target) || e.ctrlKey || e.altKey || e.metaKey) return;
      if (document.querySelector(".modal")) return;
      const mode = useStore.getState().mode;
      if (e.code === "Space") {
        e.preventDefault();
        if (mode === "playing") void onStopPlay();
        else if (mode === "idle") void onPlay();
      } else if (e.code === "KeyR") {
        e.preventDefault();
        if (mode === "recording") void onStopRecord();
        else if (mode === "idle") void onRecord();
      } else if (e.code === "Escape") {
        if (mode === "recording") void onStopRecord();
        else if (mode === "playing") void onStopPlay();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onPlay, onStopPlay, onRecord, onStopRecord]);

  // ---------- trim / import / export ----------

  const trim = async (kind: "start" | "end" | "clear") => {
    if (!s.loaded || !loadedTake) return;
    try {
      s.setError(null);
      const at = s.loaded.offset_seconds + s.cursor;
      const curIn = loadedTake.trim_in_seconds;
      const curOut = loadedTake.trim_out_seconds;
      const [a, b] = kind === "start" ? [at, curOut] : kind === "end" ? [curIn, at] : [0, null];
      s.setProject(await api.setTrim(loadedTake.id, a, b));
      await selectTake(loadedTake.id);
    } catch (e) {
      fail(e);
    }
  };

  const onImport = async () => {
    try {
      const path = await open({
        title: t.importAudio,
        multiple: false,
        directory: false,
        filters: [{ name: t.audioFiles, extensions: IMPORT_EXTENSIONS }],
      });
      if (!path || Array.isArray(path)) return;
      s.setError(null);
      const pv = await api.importAudio(path);
      s.setProject(pv);
      if (pv.loaded_take) await selectTake(pv.loaded_take);
      s.setMessage(`${t.importedOk}: ${path}`);
    } catch (e) {
      fail(e);
    }
  };

  const onExport = async () => {
    if (!loadedTake) return;
    try {
      const base = takeName(loadedTake, t.take).replace(/[<>:"/\\|?*]/g, "_");
      const path = await save({
        title: t.exportWav,
        defaultPath: `${project.name} - ${base}.wav`,
        filters: [{ name: "WAV", extensions: ["wav"] }],
      });
      if (!path) return;
      const r = await api.exportWav(path, s.prefs.exportFormat);
      s.setMessage(`${t.exported}: ${r.path} (${formatBytes(r.bytes)})`);
    } catch (e) {
      fail(e);
    }
  };

  const onClose = async () => {
    await api.closeProject();
    s.setLoaded(null, null);
    s.setProject(null, null);
    s.setMessage(null);
  };

  // ---------- render ----------

  const busy = s.mode !== "idle";
  const chLabel = (n: number) => (n === 1 ? t.mono : n === 2 ? t.stereo : `${n}`);
  const inputDev = s.inputDevices.find((d) => d.name === s.prefs.selectedInput) ?? s.inputDevices.find((d) => d.is_default);
  const rates = inputDev?.sample_rates ?? [];
  const rec = s.rec;
  const countingIn = rec != null && rec.count_in_seconds > 0 && rec.seconds_captured < rec.count_in_seconds;
  const report = s.report;
  const missing = project.takes.filter((tk) => tk.health === "missing").length;
  const changed = project.takes.filter((tk) => tk.health === "checksum_mismatch").length;

  return (
    <div className="studio">
      <div className="project-bar">
        <div>
          <strong>{project.name}</strong> <span className="muted small path">{project.root}</span>
          <div className="muted small">{t.projectSaved}</div>
        </div>
        <button className="ghost" onClick={onClose} disabled={busy}>
          {t.closeProject}
        </button>
      </div>

      {report && report.recovered.length > 0 && (
        <div className="banner warnb" role="status">
          {t.recoveredTakes}: {report.recovered.length}
        </div>
      )}
      {report && report.unrecoverable.length > 0 && (
        <div className="banner error" role="alert">
          {t.unrecoverableTakes}: {report.unrecoverable.length}
        </div>
      )}
      {(missing > 0 || changed > 0) && (
        <div className="banner error" role="alert">
          {missing > 0 && `${t.missingTakes}: ${missing}. `}
          {changed > 0 && `${t.changedTakes}: ${changed}.`}
        </div>
      )}

      <div className="studio-body">
        <TakeList onSelect={(id) => void selectTake(id)} />

        <main className="studio-main">
          <section className="devices">
            <label>
              {t.inputDevice}
              <select value={s.prefs.selectedInput ?? ""} disabled={busy} onChange={(e) => {
                s.setPref("selectedInput", e.target.value || null);
                s.setPref("sampleRate", null);
              }}>
                <option value="">{t.defaultDevice}</option>
                {s.inputDevices.map((d) => (
                  <option key={d.name} value={d.name}>
                    {d.name}
                    {d.default_channels ? ` · ${chLabel(d.default_channels)}` : ""}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t.sampleRate}
              <select value={s.prefs.sampleRate ?? ""} disabled={busy || rates.length <= 1} onChange={(e) => s.setPref("sampleRate", e.target.value ? Number(e.target.value) : null)}>
                <option value="">
                  {inputDev?.default_sample_rate ? `${inputDev.default_sample_rate} Hz (${t.deviceDefault})` : t.defaultDevice}
                </option>
                {rates
                  .filter((r) => r !== inputDev?.default_sample_rate)
                  .map((r) => (
                    <option key={r} value={r}>
                      {r} Hz
                    </option>
                  ))}
              </select>
            </label>
            <label>
              {t.outputDevice}
              <select value={s.prefs.selectedOutput ?? ""} disabled={busy} onChange={(e) => s.setPref("selectedOutput", e.target.value || null)}>
                <option value="">{t.defaultDevice}</option>
                {s.outputDevices.map((d) => (
                  <option key={d.name} value={d.name}>
                    {d.name}
                    {d.default_sample_rate ? ` · ${d.default_sample_rate} Hz` : ""}
                  </option>
                ))}
              </select>
            </label>
            <button className="ghost" onClick={() => void api.playTestTone(s.prefs.selectedOutput).catch(fail)} disabled={busy}>
              {t.testTone}
            </button>
            <button className="ghost" onClick={refreshDevices} disabled={busy}>
              {t.refresh}
            </button>
          </section>

          <TimingPanel />

          <section className="meter-row" aria-label={t.level}>
            <span className="muted small">{t.level}</span>
            <div className="meter" role="meter" aria-valuemin={-60} aria-valuemax={0} aria-valuenow={Math.round(s.meterDb)} aria-label={t.level}>
              <div className={`meter-fill ${s.meterDb > -3 ? "hot" : s.meterDb > -12 ? "warm" : ""}`} style={{ width: `${meterFraction(s.meterDb) * 100}%` }} />
            </div>
            <span className="mono-num">{formatDb(s.meterDb)}</span>
            {rec && rec.clipped_samples > 0 && <span className="clip">{t.clip}</span>}
            {rec?.noise_floor != null && (
              <span className="muted small mono-num">
                {t.noiseFloor}: {formatDb(linearToDb(rec.noise_floor))}
              </span>
            )}
          </section>

          <Waveform
            peaks={s.peaks}
            playhead={s.mode === "playing" && s.player ? s.player.position_seconds : null}
            cursor={s.mode === "idle" && s.loaded ? s.cursor : null}
            recordingSeconds={s.mode === "recording" && rec ? rec.seconds_captured : null}
            onSeek={(sec) => {
              s.setCursor(sec);
              if (s.mode === "playing") void onPlay(sec);
            }}
            emptyText={t.noTakes}
            label={loadedTake ? takeName(loadedTake, t.take) : t.noTakes}
          />

          <section className="transport">
            {s.mode === "recording" ? (
              <button className="danger" onClick={onStopRecord} aria-keyshortcuts="R Escape">
                ■ {t.stop}
              </button>
            ) : (
              <button className="rec" onClick={onRecord} disabled={busy} aria-keyshortcuts="R">
                ● {t.record}
              </button>
            )}
            {s.mode === "playing" ? (
              <button onClick={onStopPlay} aria-keyshortcuts="Space Escape">
                ■ {t.pause}
              </button>
            ) : (
              <button onClick={() => onPlay()} disabled={busy || !s.loaded} aria-keyshortcuts="Space">
                ▶ {t.play}
              </button>
            )}
            <label className="inline">
              <input type="checkbox" checked={s.prefs.loop} onChange={(e) => s.setPref("loop", e.target.checked)} />
              {t.loop}
            </label>
            <span className="mono-num time">
              {s.mode === "recording" && rec
                ? formatTime(rec.seconds_captured)
                : s.mode === "playing" && s.player
                  ? `${formatTime(s.player.position_seconds)} / ${formatTime(s.player.duration_seconds)}`
                  : s.loaded
                    ? `${formatTime(s.cursor)} / ${formatTime(s.loaded.duration_seconds)}`
                    : "0:00.0"}
            </span>
            <span className={`status ${countingIn ? "countin" : s.mode}`}>
              {countingIn
                ? `${t.countingIn} ${rec?.metronome?.position ? `${rec.metronome.position.bar}.${rec.metronome.position.beat}` : ""}`
                : s.mode === "recording"
                  ? t.recording
                  : s.mode === "playing"
                    ? t.playing
                    : t.idle}
            </span>
          </section>

          <section className="transport">
            <button className="ghost" onClick={() => trim("start")} disabled={busy || !loadedTake || s.cursor <= 0}>
              ⇤ {t.trimStartHere}
            </button>
            <button className="ghost" onClick={() => trim("end")} disabled={busy || !loadedTake || s.cursor <= 0}>
              {t.trimEndHere} ⇥
            </button>
            <button
              className="ghost"
              onClick={() => trim("clear")}
              disabled={busy || !loadedTake || (loadedTake.trim_in_seconds === 0 && loadedTake.trim_out_seconds == null)}
            >
              {t.clearTrim}
            </button>
            <span className="spacer" />
            <button className="ghost" onClick={onImport} disabled={busy}>
              {t.importAudio}
            </button>
            <label className="inline">
              {t.format}
              <select value={s.prefs.exportFormat} onChange={(e) => s.setPref("exportFormat", e.target.value as WavFormat)}>
                <option value="float32">32-bit float</option>
                <option value="pcm24">24-bit PCM</option>
                <option value="pcm16">16-bit PCM</option>
              </select>
            </label>
            <button onClick={onExport} disabled={busy || !s.loaded}>
              {t.exportWav}
            </button>
          </section>

          <section className="info">
            {s.loaded && (
              <>
                <span>
                  {t.sampleRate}: <b>{s.loaded.sample_rate} Hz</b>
                </span>
                <span>
                  {t.channels}: <b>{chLabel(s.loaded.channels)}</b>
                </span>
                <span>
                  {t.peak}: <b>{formatDb(s.loaded.peak_dbfs)}</b>
                </span>
                <span>
                  RMS: <b>{formatDb(s.loaded.rms_dbfs)}</b>
                </span>
                {s.loaded.clipped_samples > 0 && (
                  <span className="clip">
                    {t.clip} ×{s.loaded.clipped_samples}
                  </span>
                )}
              </>
            )}
            {rec && rec.overrun_samples > 0 && (
              <span className="warn">
                {t.overruns}: {rec.overrun_samples}
              </span>
            )}
            <span className="muted small">{s.loaded && s.mode === "idle" ? `${t.cursorHint} ${t.trimHint}` : t.headphonesHint}</span>
          </section>
          <div className="muted small">
            {t.shortcuts} · {t.audioHost}: {s.host || "…"}
          </div>

          {s.error && (
            <div className="banner error" role="alert">
              {t.errorPrefix}: {s.error}
            </div>
          )}
          {!s.error && s.message && (
            <div className="banner ok" role="status">
              {s.message}
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
