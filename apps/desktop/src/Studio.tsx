import { useCallback, useEffect, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, describeError, IMPORT_EXTENSIONS, type Note, type WavFormat } from "./api";
import { AnalysisPanel } from "./AnalysisPanel";
import { FallingNotes } from "./FallingNotes";
import { InstrumentPanel } from "./InstrumentPanel";
import { KeyboardPiano } from "./KeyboardPiano";
import { MidiPanel } from "./MidiPanel";
import { formatBytes, formatDb, formatTime, isTypingTarget, linearToDb, meterFraction, snapToBars, takeName } from "./format";
import { messages } from "./i18n";
import { NEW_NOTE_S, PianoRoll } from "./PianoRoll";
import { TakeList } from "./TakeList";
import { TimingPanel } from "./TimingPanel";
import { TrainerPanel } from "./TrainerPanel";
import { Tuner } from "./Tuner";
import { useStore } from "./store";
import { Waveform } from "./Waveform";

const MONITOR_POLL_MS = 50;

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

  /** Run a slow command with a visible busy label; errors land in the banner. */
  const withPending = useCallback(
    async <T,>(label: string, fn: () => Promise<T>): Promise<T | undefined> => {
      const st = useStore.getState();
      st.setPending(label);
      try {
        return await fn();
      } catch (e) {
        fail(e);
        return undefined;
      } finally {
        useStore.getState().setPending(null);
      }
    },
    [fail],
  );

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
      const st = useStore.getState();
      st.setError(null);
      if (st.mode === "playing") {
        await api.stopPlayback().catch(fail);
        st.setMode("idle");
        st.setPlayer(null);
      }
      const ok = await withPending(messages[st.prefs.lang].loading, async () => {
        const info = await api.loadTake(id);
        const peaks = await api.getWaveform(WAVEFORM_BINS);
        useStore.getState().setLoaded(info, peaks);
        useStore.getState().setAnalysis(await api.getAnalysis());
        useStore.getState().setMidi(await api.midiTake());
        return true;
      });
      if (!ok) {
        try {
          useStore.getState().setProject(await api.projectState());
        } catch {
          /* keep the current view */
        }
      }
    },
    [fail, withPending],
  );

  // ---------- recording ----------

  const onStopRecord = useCallback(async () => {
    const st = useStore.getState();
    const tt = messages[st.prefs.lang];
    st.setPending(tt.finishing);
    try {
      const r = await api.stopRecording();
      st.setMode("idle");
      st.setRec(null);
      st.setMeterDb(-120);
      st.setProject(await api.projectState());
      st.setPending(null);
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
    } finally {
      useStore.getState().setPending(null);
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
        const dur = st.loaded.duration_seconds;
        const sel = st.selection;
        let start = from ?? st.cursor;
        // With a selection, play it: start inside it unless told otherwise.
        if (from === undefined && sel && (start < sel[0] || start >= sel[1])) start = sel[0];
        if (start >= dur - 0.01) start = sel ? sel[0] : 0;
        const ps = await api.startPlayback({
          device: st.prefs.selectedOutput,
          start_seconds: start,
          loop_seconds: st.prefs.loop ? (sel ?? [0, dur]) : null,
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
    void api.setPlaybackLoop(s.prefs.loop ? (s.selection ?? [0, s.loaded.duration_seconds]) : null).catch(fail);
  }, [s.prefs.loop, s.selection, s.loaded, fail]);
  useEffect(() => {
    if (modeRef.current !== "playing") return;
    // The click is built when playback starts: restart from the current position.
    const pos = useStore.getState().player?.position_seconds ?? 0;
    void onPlay(pos);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.prefs.clickPlay]);

  // ---------- tuner: input monitor while idle ----------

  useEffect(() => {
    const on = s.prefs.tuner && s.mode === "idle";
    if (!on) {
      void api.stopMonitor().catch(() => undefined);
      useStore.getState().setMonitor(null);
      return;
    }
    let cancelled = false;
    const p = useStore.getState().prefs;
    void api.startMonitor(p.selectedInput, p.sampleRate).catch(fail);
    const tick = async () => {
      if (cancelled) return;
      try {
        useStore.getState().setMonitor(await api.monitorStatus());
      } catch (e) {
        fail(e);
        return;
      }
      setTimeout(tick, MONITOR_POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
    };
  }, [s.prefs.tuner, s.mode, s.prefs.selectedInput, s.prefs.sampleRate, fail]);

  // ---------- "play the notes" (synth) ----------

  const onPlayNotes = useCallback(async () => {
    const st = useStore.getState();
    try {
      st.setError(null);
      const ps = await api.playNotes(st.prefs.selectedOutput, st.prefs.voice);
      st.setNotesPlayer(ps);
    } catch (e) {
      fail(e);
    }
  }, [fail]);

  const onStopNotes = useCallback(async () => {
    await api.stopNotes().catch(() => undefined);
    useStore.getState().setNotesPlayer(null);
  }, []);

  useEffect(() => {
    if (!s.notesPlayer) return;
    let cancelled = false;
    const tick = async () => {
      if (cancelled) return;
      try {
        const p = await api.notesPlaybackStatus();
        if (!p || p.finished) {
          useStore.getState().setNotesPlayer(null);
          return;
        }
        useStore.getState().setNotesPlayer(p);
      } catch (e) {
        fail(e);
        useStore.getState().setNotesPlayer(null);
        return;
      }
      setTimeout(tick, STATUS_POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.notesPlayer !== null, fail]);

  /** After a MIDI re-render: refresh waveform, analysis and MIDI view. */
  const reloadRendered = useCallback(async () => {
    const st = useStore.getState();
    const peaks = await api.getWaveform(WAVEFORM_BINS);
    st.setPeaks(peaks);
    st.setAnalysis(await api.getAnalysis());
    if (st.loaded) {
      const m = await api.midiTake();
      st.setMidi(m);
      if (m) useStore.setState({ loaded: { ...st.loaded, duration_seconds: m.duration_seconds } });
    }
  }, []);

  // ---------- note editing (piano roll) ----------

  const commitNotes = useCallback(
    async (notes: Note[]) => {
      const st = useStore.getState();
      if (!st.analysis) return;
      st.pushUndo(st.analysis.notes);
      const v = await withPending(messages[st.prefs.lang].saving, () => api.updateNotes(notes));
      if (v) useStore.getState().patchAnalysis(v);
    },
    [withPending],
  );

  const undoNotes = useCallback(async () => {
    const st = useStore.getState();
    const prev = st.popUndo();
    if (!prev || !st.analysis) return;
    st.pushRedo(st.analysis.notes);
    const v = await withPending(messages[st.prefs.lang].saving, () => api.updateNotes(prev));
    if (v) useStore.getState().patchAnalysis(v);
  }, [withPending]);

  const redoNotes = useCallback(async () => {
    const st = useStore.getState();
    const next = st.popRedo();
    if (!next || !st.analysis) return;
    // Keep the undo chain without clearing redo.
    useStore.setState({ notesUndo: [...st.notesUndo.slice(-49), st.analysis.notes] });
    const v = await withPending(messages[st.prefs.lang].saving, () => api.updateNotes(next));
    if (v) useStore.getState().patchAnalysis(v);
  }, [withPending]);

  const onDragNote = useCallback(
    (index: number, start: number, end: number) => {
      const st = useStore.getState();
      if (!st.analysis || st.mode !== "idle") return;
      const notes = st.analysis.notes.map((n) => ({ ...n }));
      notes[index] = { ...notes[index], start, end };
      void commitNotes(notes);
    },
    [commitNotes],
  );

  const onAddNote = useCallback(
    (start: number, midi: number) => {
      const st = useStore.getState();
      if (!st.analysis || st.mode !== "idle" || !st.loaded) return;
      const end = Math.min(st.loaded.duration_seconds, start + NEW_NOTE_S);
      if (end - start < 0.03) return;
      const notes = [...st.analysis.notes, { start, end, midi: Math.max(0, Math.min(127, midi)), cents: 0, confidence: 1, velocity: 0.7, bend: [] as [number, number][] }];
      notes.sort((a, b) => a.start - b.start);
      void commitNotes(notes);
      st.setSelectedNote(notes.findIndex((n) => n.start === start && n.midi === midi));
    },
    [commitNotes],
  );

  const onPianoKey = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    const st = useStore.getState();
    const a = st.analysis;
    if (!a || st.mode !== "idle") return;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
      e.preventDefault();
      e.stopPropagation();
      if (e.shiftKey) void redoNotes();
      else void undoNotes();
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
      e.preventDefault();
      e.stopPropagation();
      void redoNotes();
      return;
    }
    const i = st.selectedNote;
    if (e.key === "Escape") {
      st.setSelectedNote(null);
      return;
    }
    if (i === null) return;
    const notes = a.notes.map((n) => ({ ...n, bend: n.bend.map((b) => [b[0], b[1]] as [number, number]) }));
    const n = notes[i];
    let next: Note[] | null = null;
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      const step = (e.shiftKey ? 12 : 1) * (e.key === "ArrowUp" ? 1 : -1);
      const midi = Math.max(0, Math.min(127, n.midi + step));
      notes[i] = { ...n, midi, cents: 0 };
      next = notes;
    } else if (e.key === "Delete" || e.key === "Backspace") {
      notes.splice(i, 1);
      st.setSelectedNote(null);
      next = notes;
    } else if (e.key.toLowerCase() === "s") {
      const at = st.cursor;
      if (at > n.start + 0.02 && at < n.end - 0.02) {
        const left = { ...n, end: at, bend: n.bend.filter((b) => b[0] < at - n.start) };
        const right = {
          ...n,
          start: at,
          bend: n.bend.filter((b) => b[0] >= at - n.start).map((b) => [b[0] - (at - n.start), b[1]] as [number, number]),
        };
        notes.splice(i, 1, left, right);
        next = notes;
      }
    } else if (e.key.toLowerCase() === "m") {
      const j = i + 1;
      if (j < notes.length) {
        const m = notes[j];
        const merged = {
          ...n,
          end: Math.max(n.end, m.end),
          confidence: (n.confidence + m.confidence) / 2,
          velocity: Math.max(n.velocity, m.velocity),
          bend: [...n.bend, ...m.bend.map((b) => [b[0] + (m.start - n.start), b[1] + (m.midi - n.midi) * 100] as [number, number])],
        };
        notes.splice(i, 2, merged);
        next = notes;
      }
    }
    if (next) {
      e.preventDefault();
      e.stopPropagation();
      void commitNotes(next);
    }
  };

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

  // ---------- selection / trim / import / export ----------

  /** Set one edge of the selection at the cursor (keyboard-friendly alternative to dragging). */
  const selectEdge = (edge: "start" | "end") => {
    if (!s.loaded) return;
    const dur = s.loaded.duration_seconds;
    const at = s.cursor;
    const cur = s.selection ?? [0, dur];
    const next: [number, number] = edge === "start" ? [at, Math.max(cur[1], at + 0.01)] : [Math.min(cur[0], at - 0.01), at];
    next[0] = Math.max(0, next[0]);
    next[1] = Math.min(dur, next[1]);
    if (next[1] - next[0] >= 0.01) s.setSelection(next);
  };

  /** Non-destructive: hide everything outside the selection (stored as trim points). */
  const cutToSelection = async () => {
    if (!s.loaded || !loadedTake || !s.selection) return;
    const off = s.loaded.offset_seconds;
    const [a, b] = s.selection;
    const id = loadedTake.id;
    const atEnd = b >= s.loaded.duration_seconds - 0.005;
    const pv = await withPending(t.saving, () => api.setTrim(id, off + a, atEnd ? loadedTake.trim_out_seconds : off + b));
    if (!pv) return;
    s.setProject(pv);
    await selectTake(id);
  };

  const clearTrim = async () => {
    if (!loadedTake) return;
    const pv = await withPending(t.saving, () => api.setTrim(loadedTake.id, 0, null));
    if (!pv) return;
    s.setProject(pv);
    await selectTake(loadedTake.id);
  };

  const saveSelection = async () => {
    if (!s.selection || !loadedTake) return;
    const [a, b] = s.selection;
    const pv = await withPending(t.saving, () => api.saveSelectionAsTake(a, b, null));
    if (!pv) return;
    s.setProject(pv);
    const created = pv.takes[pv.takes.length - 1];
    s.setMessage(`${t.savedAsNew}: ${takeName(created, t.take)} (${formatTime(b - a)})`);
  };

  const onImport = async () => {
    const path = await open({
      title: t.importAudio,
      multiple: false,
      directory: false,
      filters: [{ name: t.audioFiles, extensions: IMPORT_EXTENSIONS }],
    });
    if (!path || Array.isArray(path)) return;
    s.setError(null);
    const pv = await withPending(t.importing, () => api.importAudio(path));
    if (!pv) return;
    s.setProject(pv);
    if (pv.loaded_take) await selectTake(pv.loaded_take);
    s.setMessage(`${t.importedOk}: ${path}`);
  };

  const onExport = async () => {
    if (!loadedTake) return;
    const base = takeName(loadedTake, t.take).replace(/[<>:"/\\|?*]/g, "_");
    const path = await save({
      title: t.exportWav,
      defaultPath: `${project.name} - ${base}.wav`,
      filters: [{ name: "WAV", extensions: ["wav"] }],
    });
    if (!path) return;
    const r = await withPending(t.exporting, () => api.exportWav(path, s.prefs.exportFormat));
    if (r) s.setMessage(`${t.exported}: ${r.path} (${formatBytes(r.bytes)})`);
  };

  const onClose = async () => {
    await withPending(t.saving, () => api.closeProject());
    s.setLoaded(null, null);
    s.setProject(null, null);
    s.setMessage(null);
  };

  // ---------- render ----------

  const [trainerTarget, setTrainerTarget] = useState<{ index: number; wrong: boolean } | null>(null);
  // Bar length in playback seconds for a MIDI take with a known tempo and meter.
  const barSeconds =
    s.midi && s.midi.time_signature && s.midi.bpm > 0
      ? ((s.midi.time_signature[0] * (4 / s.midi.time_signature[1]) * 60) / s.midi.bpm) / s.midi.render.tempo_factor
      : null;
  const busy = s.mode !== "idle" || s.pending !== null;
  const hasTrim = loadedTake != null && (loadedTake.trim_in_seconds > 0 || loadedTake.trim_out_seconds != null);
  const chLabel = (n: number) => (n === 1 ? t.mono : n === 2 ? t.stereo : `${n}`);
  const inputDev = s.inputDevices.find((d) => d.name === s.prefs.selectedInput) ?? s.inputDevices.find((d) => d.is_default);
  const rates = inputDev?.sample_rates ?? [];
  const rec = s.rec;
  const countingIn = rec != null && rec.count_in_seconds > 0 && rec.seconds_captured < rec.count_in_seconds;
  const report = s.report;
  const missing = project.takes.filter((tk) => tk.health === "missing").length;
  const changed = project.takes.filter((tk) => tk.health === "checksum_mismatch").length;

  return (
    <div className="studio" aria-busy={s.pending !== null}>
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
            <label className="inline">
              <input type="checkbox" checked={s.prefs.tuner} onChange={(e) => s.setPref("tuner", e.target.checked)} />
              {t.tuner}
            </label>
            <button className="ghost" onClick={refreshDevices} disabled={busy}>
              {t.refresh}
            </button>
          </section>

          <TimingPanel />

          {(s.prefs.tuner || s.mode === "recording") && (
            <Tuner
              active={s.mode === "recording" ? rec !== null : s.monitor !== null}
              hz={s.mode === "recording" ? (rec?.pitch_hz ?? null) : (s.monitor?.pitch_hz ?? null)}
              confidence={s.mode === "recording" ? (rec?.pitch_confidence ?? 0) : (s.monitor?.pitch_confidence ?? 0)}
              a4={project.a4_hz}
              scale={project.scale}
              profile={project.instrument}
            />
          )}
          <InstrumentPanel />
          <KeyboardPiano scale={project.scale} />

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
            selection={s.loaded ? s.selection : null}
            recordingSeconds={s.mode === "recording" && rec ? rec.seconds_captured : null}
            onSeek={(sec) => {
              s.setCursor(sec);
              if (s.mode === "playing") void onPlay(sec);
            }}
            onSelect={(sel) => s.setSelection(sel && barSeconds && s.prefs.snapBar ? snapToBars(sel, barSeconds, s.loaded?.duration_seconds ?? 0) : sel)}
            emptyText={t.noTakes}
            label={loadedTake ? takeName(loadedTake, t.take) : t.noTakes}
          />

          {s.loaded && <MidiPanel onRerendered={reloadRendered} />}
          {s.loaded && s.analysis && (
            <>
              <div className="transport">
                <label className="inline">
                  <input type="radio" name="notesView" checked={s.prefs.notesView === "roll"} onChange={() => s.setPref("notesView", "roll")} />
                  {t.viewRoll}
                </label>
                <label className="inline">
                  <input type="radio" name="notesView" checked={s.prefs.notesView === "falling"} onChange={() => s.setPref("notesView", "falling")} />
                  {t.viewFalling}
                </label>
                {s.prefs.notesView === "falling" && (
                  <label className="inline">
                    <input type="checkbox" checked={s.prefs.keyNames} onChange={(e) => s.setPref("keyNames", e.target.checked)} />
                    {t.noteNamesOnKeys}
                  </label>
                )}
                <span className="spacer" />
                <label className="inline">
                  {t.voice}
                  <select value={s.prefs.voice} onChange={(e) => s.setPref("voice", e.target.value as typeof s.prefs.voice)}>
                    {(["flute", "sine", "triangle"] as const).map((v) => (
                      <option key={v} value={v}>
                        {t.voices[v]}
                      </option>
                    ))}
                  </select>
                </label>
                {s.notesPlayer ? (
                  <button onClick={onStopNotes}>
                    ■ {t.stopNotes} <span className="mono-num">{formatTime(s.notesPlayer.position_seconds)}</span>
                  </button>
                ) : (
                  <button onClick={onPlayNotes} disabled={busy || s.analysis.notes.length === 0}>
                    ♪ {t.playNotes}
                  </button>
                )}
              </div>
              {s.prefs.notesView === "roll" ? (
                <>
                  <PianoRoll
                    notes={s.analysis.notes}
                    contour={s.analysis.contour}
                    duration={s.loaded.duration_seconds}
                    cursor={s.mode === "idle" && !s.notesPlayer ? s.cursor : null}
                    playhead={
                      s.notesPlayer
                        ? s.notesPlayer.position_seconds
                        : s.mode === "playing" && s.player
                          ? s.player.position_seconds
                          : null
                    }
                    selected={s.selectedNote}
                    scale={project.scale}
                    noteNames={t.noteNames}
                    label={t.pianoRoll}
                    onSelect={(i) => s.setSelectedNote(i)}
                    onSeek={(sec) => {
                      s.setCursor(sec);
                      if (s.mode === "playing") void onPlay(sec);
                    }}
                    onKey={onPianoKey}
                    onDrag={onDragNote}
                    onAdd={onAddNote}
                  />
                  <div className="muted small">
                    {t.pianoRollHint} {t.addNoteHint}
                  </div>
                </>
              ) : (
                <FallingNotes
                  notes={s.analysis.notes}
                  position={
                    s.notesPlayer
                      ? s.notesPlayer.position_seconds
                      : s.mode === "playing" && s.player
                        ? s.player.position_seconds
                        : s.cursor
                  }
                  lookahead={3}
                  scale={project.scale}
                  noteNames={t.noteNames}
                  showNames={s.prefs.keyNames}
                  label={t.viewFalling}
                  onSeek={(sec) => {
                    s.setCursor(sec);
                    if (s.mode === "playing") void onPlay(sec);
                  }}
                  target={trainerTarget}
                />
              )}
              <TrainerPanel
                notes={s.analysis.notes}
                hz={s.monitor?.pitch_hz ?? null}
                confidence={s.monitor?.pitch_confidence ?? 0}
                a4={project.a4_hz}
                notesPosition={s.notesPlayer ? s.notesPlayer.position_seconds : null}
                onPlayNotes={onPlayNotes}
                onStopNotes={onStopNotes}
                onPosition={(sec) => {
                  s.setPref("notesView", "falling");
                  s.setCursor(sec);
                }}
                onTarget={(index, wrong) => setTrainerTarget(index === null ? null : { index, wrong })}
              />
            </>
          )}
          {s.loaded && <AnalysisPanel withPending={withPending} selectTake={selectTake} />}

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
            {barSeconds && (
              <label className="inline">
                <input type="checkbox" checked={s.prefs.snapBar} onChange={(e) => s.setPref("snapBar", e.target.checked)} />
                {t.snapToBar}
              </label>
            )}
            <span className="mono-num time">
              {s.mode === "recording" && rec
                ? formatTime(rec.seconds_captured)
                : s.mode === "playing" && s.player
                  ? `${formatTime(s.player.position_seconds)} / ${formatTime(s.player.duration_seconds)}`
                  : s.loaded
                    ? `${formatTime(s.cursor)} / ${formatTime(s.loaded.duration_seconds)}`
                    : "0:00.0"}
            </span>
            {s.pending && (
              <span className="pending" role="status" aria-live="polite">
                {s.pending}
              </span>
            )}
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

          <section className="transport selection-row">
            <span className="muted small">
              {t.selection}:{" "}
              <span className="mono-num">
                {s.selection ? `${formatTime(s.selection[0])} – ${formatTime(s.selection[1])} (${formatTime(s.selection[1] - s.selection[0])})` : "—"}
              </span>
            </span>
            <button className="ghost small" onClick={() => selectEdge("start")} disabled={busy || !s.loaded}>
              ⇤ {t.selStartHere}
            </button>
            <button className="ghost small" onClick={() => selectEdge("end")} disabled={busy || !s.loaded || s.cursor <= 0}>
              {t.selEndHere} ⇥
            </button>
            <button className="ghost small" onClick={() => s.setSelection(null)} disabled={busy || !s.selection}>
              {t.clearSelection}
            </button>
            <button onClick={cutToSelection} disabled={busy || !s.selection}>
              ✂ {t.cutToSelection}
            </button>
            <button onClick={saveSelection} disabled={busy || !s.selection}>
              {t.saveSelectionAsTake}
            </button>
            {hasTrim && (
              <button className="ghost small" onClick={clearTrim} disabled={busy}>
                {t.clearTrim}
              </button>
            )}
          </section>

          <section className="transport">
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
            <span className="muted small">{s.loaded && s.mode === "idle" ? `${t.selectHint} ${t.trimHint}` : t.headphonesHint}</span>
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
