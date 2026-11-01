import { useCallback, useEffect, useRef, useState, type WheelEvent as RWheelEvent } from "react";
import { api, describeError, type EditOp, type EditStatus, type EqBand, type PlayerStatus, type WaveformPeaks } from "./api";
import { formatDb, formatTime, isTypingTarget, takeName } from "./format";
import { messages, type StringMessageKey } from "./i18n";
import { useStore } from "./store";
import { latestTake, stopPlaying } from "./takeActions";
import { Waveform } from "./Waveform";

const BINS = 1600;
const MIN_VIEW_S = 0.02;

/** Operation keys from the engine → their labels. */
const OP_LABEL: Record<string, StringMessageKey> = {
  open: "opOpen",
  cut: "opCut",
  copy: "opCopy",
  delete: "opDelete",
  paste: "opPaste",
  silence: "opSilence",
  trim: "opTrim",
  insert_silence: "opInsertSilence",
  gain: "opGain",
  normalize: "opNormalize",
  fade_in: "opFadeIn",
  fade_out: "opFadeOut",
  reverse: "opReverse",
  eq: "opEq",
  compress: "opCompress",
  limit: "opLimit",
  noise_profile: "opNoiseProfile",
  denoise: "opDenoise",
};

type FxKind = "eq" | "compress" | "limit" | "denoise";
const DEFAULT_DENOISE = { reduction_db: 12, sensitivity: 1.5 };
const DEFAULT_BANDS: EqBand[] = [
  { kind: "low_shelf", hz: 200, gain_db: 0, q: 1 },
  { kind: "peak", hz: 1000, gain_db: 0, q: 1 },
  { kind: "high_shelf", hz: 4000, gain_db: 0, q: 1 },
];
const DEFAULT_COMP = { threshold_db: -24, ratio: 3, attack_ms: 10, release_ms: 100, makeup_db: 0 };
const DEFAULT_LIMIT = { ceiling_db: -1 };

/**
 * Audio editor (home-studio item 5 = AUDIO_EDITOR_PLAN E1, 2026-11-01): one take as a
 * working copy in the engine, a selection on the waveform, clipboard operations, zoom,
 * and a history of versions to walk back and forth. Nothing touches the original file:
 * "Save as a new take" writes the current version next to it.
 */
export function EditScreen() {
  const t = messages[useStore((s) => s.prefs.lang)];
  const project = useStore((s) => s.project);
  const editTakeId = useStore((s) => s.editTakeId);
  const setEditTake = useStore((s) => s.setEditTake);
  const selectedOutput = useStore((s) => s.prefs.selectedOutput);
  const [status, setStatus] = useState<EditStatus | null>(null);
  const [peaks, setPeaks] = useState<WaveformPeaks | null>(null);
  /** Visible range in seconds, or null for the whole take. */
  const [view, setView] = useState<[number, number] | null>(null);
  const [cursor, setCursor] = useState(0);
  const [selection, setSelection] = useState<[number, number] | null>(null);
  const [playing, setPlaying] = useState<{ from: number; st: PlayerStatus } | null>(null);
  const [busy, setBusy] = useState(false);
  const [pick, setPick] = useState("");
  // A7 effects with a preview: which form is open and its values.
  const [fx, setFx] = useState<FxKind | null>(null);
  const [bands, setBands] = useState<EqBand[]>(DEFAULT_BANDS);
  const [comp, setComp] = useState(DEFAULT_COMP);
  const [lim, setLim] = useState(DEFAULT_LIMIT);
  const [dn, setDn] = useState(DEFAULT_DENOISE);
  const statusRef = useRef(status);
  statusRef.current = status;
  const viewRef = useRef(view);
  viewRef.current = view;

  const fail = useCallback((e: unknown) => useStore.getState().setError(describeError(e)), []);

  const loadPeaks = useCallback(
    async (v: [number, number] | null) => {
      try {
        setPeaks(await api.editPeaks(BINS, v ? v[0] : null, v ? v[1] : null));
      } catch (e) {
        fail(e);
      }
    },
    [fail],
  );

  /** Take a fresh status: clamp the view, cursor and selection to the new length. */
  const absorb = useCallback(
    (st: EditStatus | null) => {
      setStatus(st);
      if (!st) {
        setPeaks(null);
        return;
      }
      const len = st.seconds;
      let v = viewRef.current;
      if (v && (v[1] > len + 1e-6 || v[1] - v[0] < MIN_VIEW_S)) {
        v = v[0] < len - MIN_VIEW_S ? [v[0], len] : null;
        setView(v);
      }
      setCursor((c) => Math.min(c, len));
      setSelection((sel) => (sel && sel[0] < len - 1e-6 ? [sel[0], Math.min(sel[1], len)] : null));
      void loadPeaks(v);
    },
    [loadPeaks],
  );

  const open = useCallback(
    async (id: string) => {
      setBusy(true);
      try {
        await stopPlaying().catch(() => undefined);
        setPlaying(null);
        setView(null);
        viewRef.current = null;
        setSelection(null);
        setCursor(0);
        absorb(await api.editOpen(id));
        setEditTake(id);
      } catch (e) {
        fail(e);
      } finally {
        setBusy(false);
      }
    },
    [absorb, fail, setEditTake],
  );

  // Mount: continue the engine's session, or open the take the studio pointed at.
  useEffect(() => {
    void (async () => {
      try {
        const st = await api.editStatus();
        if (editTakeId && st?.take_id !== editTakeId) {
          await open(editTakeId);
        } else {
          absorb(st);
        }
      } catch (e) {
        fail(e);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editTakeId]);

  // Playhead polling while the editor plays.
  useEffect(() => {
    if (!playing) return;
    const id = window.setInterval(async () => {
      try {
        const st = await api.playbackStatus();
        if (!st || st.finished) setPlaying(null);
        else setPlaying((p) => (p ? { ...p, st } : null));
      } catch {
        setPlaying(null);
      }
    }, 100);
    return () => window.clearInterval(id);
  }, [playing !== null]); // eslint-disable-line react-hooks/exhaustive-deps

  const run = useCallback(
    async (op: EditOp) => {
      if (!statusRef.current) return;
      setBusy(true);
      try {
        absorb(await api.editApply(op));
      } catch (e) {
        fail(e);
      } finally {
        setBusy(false);
      }
    },
    [absorb, fail],
  );

  const whole = (): [number, number] => [0, statusRef.current?.seconds ?? 0];
  const region = (): [number, number] => selection ?? whole();
  const needSel = () => {
    if (!selection) {
      useStore.getState().setMessage(t.editNeedSelection);
      return null;
    }
    return selection;
  };

  const doCut = () => {
    const r = needSel();
    if (r) void run({ op: "cut", a: r[0], b: r[1] }).then(() => setSelection(null));
  };
  const doCopy = () => {
    const r = needSel();
    if (r) void run({ op: "copy", a: r[0], b: r[1] });
  };
  const doDelete = () => {
    const r = needSel();
    if (r) void run({ op: "delete", a: r[0], b: r[1] }).then(() => setSelection(null));
  };
  const doPaste = () => void run({ op: "paste", at: cursor });
  const doSilence = () => {
    const r = region();
    void run({ op: "silence", a: r[0], b: r[1] });
  };
  const doTrim = () => {
    const r = needSel();
    if (r) void run({ op: "trim", a: r[0], b: r[1] }).then(() => setSelection(null));
  };
  const doInsertSilence = () => void run({ op: "insert_silence", at: cursor, seconds: 1 });
  const doGain = (db: number) => {
    const r = region();
    void run({ op: "gain", a: r[0], b: r[1], db });
  };
  const doNormalize = () => {
    const r = region();
    void run({ op: "normalize", a: r[0], b: r[1], target_dbfs: -1 });
  };
  const doFade = (which: "fade_in" | "fade_out") => {
    const r = region();
    void run({ op: which, a: r[0], b: r[1] });
  };
  const doReverse = () => {
    const r = region();
    void run({ op: "reverse", a: r[0], b: r[1] });
  };
  const doSnap = async () => {
    try {
      if (selection) {
        const a = await api.editSnap(selection[0]);
        const b = await api.editSnap(selection[1]);
        if (b > a) setSelection([a, b]);
      } else {
        setCursor(await api.editSnap(cursor));
      }
    } catch (e) {
      fail(e);
    }
  };
  const fxOp = (): EditOp | null => {
    const r = region();
    if (fx === "eq") return { op: "eq", a: r[0], b: r[1], bands };
    if (fx === "compress") return { op: "compress", a: r[0], b: r[1], ...comp };
    if (fx === "limit") return { op: "limit", a: r[0], b: r[1], ...lim };
    if (fx === "denoise") return { op: "denoise", a: r[0], b: r[1], ...dn };
    return null;
  };
  const previewFx = async () => {
    const op = fxOp();
    if (!op) return;
    try {
      await stopPlaying().catch(() => undefined);
      const st = await api.editPreview(op, 4, selectedOutput);
      setPlaying({ from: region()[0], st });
    } catch (e) {
      fail(e);
    }
  };
  const applyFx = () => {
    const op = fxOp();
    if (op) void run(op);
  };
  const setBand = (i: number, patch: Partial<EqBand>) => setBands((b) => b.map((x, k) => (k === i ? { ...x, ...patch } : x)));
  const doUndo = () => void api.editUndo().then(absorb).catch(fail);
  const doRedo = () => void api.editRedo().then(absorb).catch(fail);
  const doJump = (step: number) => void api.editJump(step).then(absorb).catch(fail);

  const play = async (from: number, to: number | null) => {
    try {
      await stopPlaying().catch(() => undefined);
      const st = await api.editPlay(from, to, selectedOutput);
      setPlaying({ from, st });
    } catch (e) {
      fail(e);
    }
  };
  const stop = async () => {
    await api.stopPlayback().catch(() => undefined);
    setPlaying(null);
  };

  const save = async () => {
    if (!status || !project) return;
    const parent = project.takes.find((x) => x.id === status.take_id);
    const ops = status.versions
      .slice(1, status.index + 1)
      .map((v) => t[OP_LABEL[v.op] ?? "opOpen"])
      .filter((x, i, arr) => arr.indexOf(x) === i);
    const label = `${parent ? takeName(parent, t.take) : t.take} · ${ops.length ? ops.join(", ") : t.opOpen}`;
    setBusy(true);
    try {
      const r = await api.editSave(label);
      useStore.getState().setProject(r.project);
      absorb(await api.editStatus());
      useStore.getState().setMessage(`${t.editSaved}: ${label}`);
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  // Zoom: Ctrl+wheel around the pointer; the view is [from, to] in absolute seconds.
  const onWheel = (e: RWheelEvent<HTMLDivElement>) => {
    if (!e.ctrlKey || !status) return;
    e.preventDefault();
    const len = status.seconds;
    const v = view ?? [0, len];
    const rect = e.currentTarget.getBoundingClientRect();
    const frac = Math.min(1, Math.max(0, (e.clientX - rect.left) / Math.max(1, rect.width)));
    const at = v[0] + frac * (v[1] - v[0]);
    const factor = e.deltaY < 0 ? 0.7 : 1 / 0.7;
    let span = Math.min(len, Math.max(MIN_VIEW_S, (v[1] - v[0]) * factor));
    let from = at - frac * span;
    if (from < 0) from = 0;
    if (from + span > len) from = Math.max(0, len - span);
    span = Math.min(span, len - from);
    const next: [number, number] | null = span >= len - 1e-6 ? null : [from, from + span];
    setView(next);
    viewRef.current = next;
    void loadPeaks(next);
  };
  const zoomTo = (next: [number, number] | null) => {
    setView(next);
    viewRef.current = next;
    void loadPeaks(next);
  };

  // Keyboard, as in My studio.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTypingTarget(e.target) || !statusRef.current) return;
      const k = e.key.toLowerCase();
      if (e.ctrlKey && k === "x") doCut();
      else if (e.ctrlKey && k === "c") doCopy();
      else if (e.ctrlKey && k === "v") doPaste();
      else if (e.ctrlKey && k === "z") doUndo();
      else if (e.ctrlKey && k === "y") doRedo();
      else if (e.key === "Delete") doDelete();
      else if (k === "z") void doSnap();
      else if (e.key === " ") {
        if (playing) void stop();
        else if (selection) void play(selection[0], selection[1]);
        else void play(cursor, null);
      } else if (e.key === "Escape") void stop();
      else if (e.key === "Home") setCursor(0);
      else if (e.key === "End") setCursor(statusRef.current.seconds);
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const takes = (project?.takes ?? []).filter((x) => x.source !== "midi_file" && x.state !== "recording");
  const parent = status && project ? project.takes.find((x) => x.id === status.take_id) : null;
  const from = view ? view[0] : 0;
  const rel = (abs: number) => abs - from;
  const playhead = playing ? rel(playing.from + playing.st.position_seconds) : null;
  const viewSel: [number, number] | null = selection ? [rel(selection[0]), rel(selection[1])] : null;
  const hasClip = (status?.clipboard_seconds ?? 0) > 0;

  return (
    <main className="editor">
      <section className="panel">
        <div className="panel-head">
          <h2>{t.editTitle}</h2>
          <label className="inline small">
            {t.editPick}
            <select
              value={pick || status?.take_id || ""}
              onChange={(e) => {
                setPick(e.target.value);
                if (e.target.value) void open(e.target.value);
              }}
              disabled={busy}
            >
              <option value="">{t.editPickHint}</option>
              {takes.map((x) => (
                <option key={x.id} value={x.id}>
                  {takeName(x, t.take)}
                </option>
              ))}
            </select>
          </label>
          {!status && takes.length > 0 && (
            <button className="small" onClick={() => void open(latestTake(takes)?.id ?? takes[0].id)} disabled={busy}>
              {t.editOpenLatest}
            </button>
          )}
        </div>
        <div className="muted small">{t.editHint}</div>
        {status && (
          <div className="muted small mono-num">
            {parent ? takeName(parent, t.take) : status.take_id} · {formatTime(status.seconds)} · {status.sample_rate} Hz · {status.channels === 1 ? t.mono : t.stereo} · {t.peak} {formatDb(status.peak_dbfs)}
            {status.dirty ? ` · ${t.editDirty}` : ""}
          </div>
        )}
      </section>

      <section className="panel waveform-panel editor-wave" onWheel={onWheel}>
        <Waveform
          peaks={peaks}
          playhead={playhead}
          cursor={playing ? null : rel(cursor)}
          selection={viewSel}
          label={t.editTitle}
          recordingSeconds={null}
          emptyText={t.editEmpty}
          onSeek={(sec) => setCursor(Math.max(0, Math.min(status?.seconds ?? 0, from + sec)))}
          onSelect={(sel) => setSelection(sel ? [from + sel[0], from + sel[1]] : null)}
        />
        <div className="analysis-row small">
          <span>
            {t.editCursor}: <span className="mono-num">{formatTime(cursor)}</span>
          </span>
          <span>
            {t.selection}: <span className="mono-num">{selection ? `${formatTime(selection[0])} – ${formatTime(selection[1])} (${formatTime(selection[1] - selection[0])})` : "—"}</span>
          </span>
          <span>
            {t.editView}: <span className="mono-num">{view ? `${formatTime(view[0])} – ${formatTime(view[1])}` : t.editZoomAll}</span>
          </span>
          <button className="ghost small" onClick={() => selection && zoomTo([selection[0], selection[1]])} disabled={!selection}>
            {t.editZoomSel}
          </button>
          <button className="ghost small" onClick={() => zoomTo(null)} disabled={!view}>
            {t.editZoomAll}
          </button>
          <button className="ghost small" onClick={() => setSelection(null)} disabled={!selection}>
            {t.clearSelection}
          </button>
        </div>
      </section>

      <section className="panel">
        <div className="analysis-row">
          {playing ? (
            <button onClick={() => void stop()}>{t.editStop}</button>
          ) : (
            <>
              <button onClick={() => selection && void play(selection[0], selection[1])} disabled={!status || !selection}>
                {t.editPlaySel}
              </button>
              <button onClick={() => void play(cursor, null)} disabled={!status}>
                {t.editPlayCursor}
              </button>
            </>
          )}
          <span className="fx-sep" />
          <button onClick={doCut} disabled={busy || !selection} title="Ctrl+X">
            {t.editCut}
          </button>
          <button onClick={doCopy} disabled={busy || !selection} title="Ctrl+C">
            {t.editCopy}
          </button>
          <button onClick={doPaste} disabled={busy || !hasClip} title="Ctrl+V">
            {t.editPaste}
            {hasClip ? ` (${formatTime(status?.clipboard_seconds ?? 0)})` : ""}
          </button>
          <button onClick={doDelete} disabled={busy || !selection} title="Delete">
            {t.editDelete}
          </button>
          <button onClick={doTrim} disabled={busy || !selection}>
            {t.editTrim}
          </button>
          <button onClick={doSilence} disabled={busy || !status}>
            {t.editSilence}
          </button>
          <button onClick={doInsertSilence} disabled={busy || !status}>
            {t.editInsertSilence}
          </button>
        </div>
        <div className="analysis-row">
          <button onClick={() => doGain(3)} disabled={busy || !status}>
            +3 dB
          </button>
          <button onClick={() => doGain(-3)} disabled={busy || !status}>
            −3 dB
          </button>
          <button onClick={doNormalize} disabled={busy || !status}>
            {t.editNormalize}
          </button>
          <button onClick={() => doFade("fade_in")} disabled={busy || !status}>
            {t.editFadeIn}
          </button>
          <button onClick={() => doFade("fade_out")} disabled={busy || !status}>
            {t.editFadeOut}
          </button>
          <button onClick={doReverse} disabled={busy || !status}>
            {t.editReverse}
          </button>
          <button onClick={() => void doSnap()} disabled={busy || !status} title="Z">
            {t.editSnap}
          </button>
          <span className="muted small">{selection ? t.editOnSelection : t.editOnWhole}</span>
        </div>
        <div className="analysis-row">
          <button className="ghost small" onClick={doUndo} disabled={!status?.can_undo} title="Ctrl+Z">
            ↶ {t.undo}
          </button>
          <button className="ghost small" onClick={doRedo} disabled={!status?.can_redo} title="Ctrl+Y">
            ↷ {t.redo}
          </button>
          <button className="primary" onClick={() => void save()} disabled={busy || !status || !status.dirty}>
            {t.editSave}
          </button>
        </div>
        <div className="muted small">{t.editShortcuts}</div>
      </section>

      {status && (
        <section className="panel" aria-label={t.editFx}>
          <div className="analysis-row">
            <h3>{t.editFx}</h3>
            {(["eq", "compress", "limit", "denoise"] as FxKind[]).map((k) => (
              <button key={k} className={`small ${fx === k ? "primary" : ""}`} onClick={() => setFx(fx === k ? null : k)}>
                {k === "eq" ? t.fxEq : k === "compress" ? t.fxCompress : k === "limit" ? t.fxLimit : t.fxDenoise}
              </button>
            ))}
            <span className="muted small">{t.editFxHint}</span>
          </div>
          {fx === "eq" && (
            <div className="fx-form">
              {bands.map((b, i) => (
                <div className="analysis-row" key={i}>
                  <span className="small" style={{ width: 90 }}>
                    {b.kind === "low_shelf" ? t.eqLow : b.kind === "peak" ? t.eqMid : t.eqHigh}
                  </span>
                  <label className="inline small">
                    Hz
                    <input type="number" min={20} max={20000} step={10} value={b.hz} onChange={(e) => setBand(i, { hz: Math.max(20, Math.min(20000, Number(e.target.value))) })} style={{ width: 80 }} />
                  </label>
                  <label className="inline small">
                    dB
                    <input type="range" min={-24} max={24} step={0.5} value={b.gain_db} onChange={(e) => setBand(i, { gain_db: Number(e.target.value) })} />
                    <span className="mono-num">{b.gain_db >= 0 ? "+" : ""}{b.gain_db.toFixed(1)}</span>
                  </label>
                  {b.kind === "peak" && (
                    <label className="inline small">
                      Q
                      <input type="number" min={0.1} max={10} step={0.1} value={b.q} onChange={(e) => setBand(i, { q: Math.max(0.1, Math.min(10, Number(e.target.value))) })} style={{ width: 60 }} />
                    </label>
                  )}
                </div>
              ))}
            </div>
          )}
          {fx === "compress" && (
            <div className="fx-form analysis-row">
              {(
                [
                  ["threshold_db", t.compThreshold, -60, 0, 1, "dB"],
                  ["ratio", t.compRatio, 1, 20, 0.5, ":1"],
                  ["attack_ms", t.compAttack, 0.1, 200, 0.1, "ms"],
                  ["release_ms", t.compRelease, 1, 2000, 1, "ms"],
                  ["makeup_db", t.compMakeup, 0, 24, 0.5, "dB"],
                ] as [keyof typeof DEFAULT_COMP, string, number, number, number, string][]
              ).map(([key, label, min, max, step, unit]) => (
                <label className="inline small" key={key}>
                  {label}
                  <input type="range" min={min} max={max} step={step} value={comp[key]} onChange={(e) => setComp({ ...comp, [key]: Number(e.target.value) })} />
                  <span className="mono-num">
                    {comp[key]} {unit}
                  </span>
                </label>
              ))}
            </div>
          )}
          {fx === "limit" && (
            <div className="fx-form analysis-row">
              <label className="inline small">
                {t.fxCeiling}
                <input type="range" min={-24} max={0} step={0.5} value={lim.ceiling_db} onChange={(e) => setLim({ ceiling_db: Number(e.target.value) })} />
                <span className="mono-num">{lim.ceiling_db.toFixed(1)} dBFS</span>
              </label>
            </div>
          )}
          {fx === "denoise" && (
            <div className="fx-form analysis-row">
              <button className="small" onClick={() => { const r = needSel(); if (r) void run({ op: "noise_profile", a: r[0], b: r[1] }); }} disabled={busy || !selection} title={t.fxProfileHint}>
                {t.fxProfile}
              </button>
              <span className={`badge ${status?.noise_profile ? "ok" : "warn"}`}>{status?.noise_profile ? t.fxProfileReady : t.fxProfileNone}</span>
              <label className="inline small">
                {t.fxReduction}
                <input type="range" min={0} max={40} step={1} value={dn.reduction_db} onChange={(e) => setDn({ ...dn, reduction_db: Number(e.target.value) })} />
                <span className="mono-num">{dn.reduction_db} dB</span>
              </label>
              <label className="inline small">
                {t.fxSensitivity}
                <input type="range" min={0.5} max={4} step={0.1} value={dn.sensitivity} onChange={(e) => setDn({ ...dn, sensitivity: Number(e.target.value) })} />
                <span className="mono-num">{dn.sensitivity.toFixed(1)}</span>
              </label>
            </div>
          )}
          {fx && (
            <div className="analysis-row">
              <button onClick={() => void previewFx()} disabled={busy || (fx === "denoise" && !status?.noise_profile)}>
                ▶ {t.fxPreview}
              </button>
              <button className="primary" onClick={applyFx} disabled={busy || (fx === "denoise" && !status?.noise_profile)}>
                ✓ {t.fxApply}
              </button>
              <span className="muted small">{selection ? t.editOnSelection : t.editOnWhole}</span>
            </div>
          )}
        </section>
      )}

      {status && (
        <section className="panel">
          <h3>{t.editHistory}</h3>
          <ol className="edit-history">
            {status.versions.map((v, i) => (
              <li key={v.step} className={i === status.index ? "on" : i > status.index ? "undone" : ""}>
                <button className="ghost small" onClick={() => doJump(v.step)} disabled={i === status.index}>
                  {v.step}. {t[OP_LABEL[v.op] ?? "opOpen"]} <span className="mono-num muted">({formatTime(v.seconds)})</span>
                </button>
              </li>
            ))}
          </ol>
        </section>
      )}
    </main>
  );
}
