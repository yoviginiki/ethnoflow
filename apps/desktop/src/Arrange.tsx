import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as RPointerEvent, type WheelEvent as RWheelEvent } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, describeError, IMPORT_EXTENSIONS, type Arrangement, type ArrangeStatus, type Clip, type RecordingStatus, type TakeView, type Track, type WaveformPeaks, SECTION_NAMES } from "./api";
import { BackgroundTransport } from "./BackgroundTransport";
import { droneRoot, droneSettings, padRoot, padSettings, voicingNotes } from "./layers";
import { ConfirmDialog } from "./ConfirmDialog";
import { formatBytes, formatDb, formatTime, isTypingTarget, linearToDb, meterFraction, takeName } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

const RULER_H = 26;
const LANE_H = 76;
const MIN_PPS = 3;
const MAX_PPS = 600;
const SNAP_PX = 8;
const EDGE_PX = 7;
const MIN_CLIP_S = 0.05;
const POLL_MS = 50;
const PEAK_BINS = 2000;
const MAX_TRACKS = 8;

function uid(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID().slice(0, 8) : Math.random().toString(36).slice(2, 10);
}

function clone(a: Arrangement): Arrangement {
  return JSON.parse(JSON.stringify(a)) as Arrangement;
}

function trackColor(i: number): string {
  const hues = [170, 35, 265, 200, 95, 330, 20, 220];
  return `hsl(${hues[i % hues.length]} 55% 55%)`;
}

type Drag =
  | { kind: "none" }
  | { kind: "ruler"; anchor: number; moved: boolean }
  | { kind: "move"; track: number; clip: string; grab: number; origin: Arrangement; moved: boolean; startX: number }
  | { kind: "trimL" | "trimR"; track: number; clip: string; origin: Arrangement; moved: boolean };

/**
 * "My studio" (F5.1): tracks of clips over the project's takes on one timeline.
 * The clips, the drone and the pad play on the same audio graph, so the background
 * and the recordings sound together; "Record" adds a new take at the cursor while
 * everything plays. Edits go to the engine as a whole arrangement (undo/redo here).
 */
export function Arrange() {
  const lang = useStore((s) => s.prefs.lang);
  const t = messages[lang];
  const project = useStore((s) => s.project)!;
  const prefs = useStore((s) => s.prefs);
  const mode = useStore((s) => s.mode);
  const setMode = useStore((s) => s.setMode);
  const error = useStore((s) => s.error);
  const message = useStore((s) => s.message);
  const setError = useStore((s) => s.setError);
  const setMessage = useStore((s) => s.setMessage);
  const pending = useStore((s) => s.pending);

  const [arr, setArrState] = useState<Arrangement | null>(null);
  const arrRef = useRef<Arrangement | null>(null);
  const setArr = useCallback((a: Arrangement | null) => {
    arrRef.current = a;
    setArrState(a);
  }, []);
  const [undo, setUndo] = useState<Arrangement[]>([]);
  const [redo, setRedo] = useState<Arrangement[]>([]);
  const [status, setStatus] = useState<ArrangeStatus | null>(null);
  const statusRef = useRef<ArrangeStatus | null>(null);
  statusRef.current = status;
  const [cursor, setCursorState] = useState(0);
  const cursorRef = useRef(0);
  const setCursor = useCallback((c: number) => {
    cursorRef.current = Math.max(0, c);
    setCursorState(Math.max(0, c));
  }, []);
  const [pps, setPps] = useState(40);
  const [scroll, setScroll] = useState(0);
  const viewRef = useRef({ pps: 40, scroll: 0 });
  viewRef.current = { pps, scroll };
  const [snap, setSnap] = useState(true);
  const [selected, setSelected] = useState<{ track: string; clip: string } | null>(null);
  const [selectedTrack, setSelectedTrack] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [removeTrack, setRemoveTrack] = useState<Track | null>(null);
  const [rec, setRec] = useState<RecordingStatus | null>(null);
  const recStart = useRef<{ at: number; track: string | null } | null>(null);
  const peaks = useRef(new Map<string, WaveformPeaks>());
  const [peaksVersion, setPeaksVersion] = useState(0);
  const drag = useRef<Drag>({ kind: "none" });
  const preview = useRef<Arrangement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [tick, bump] = useState(0);
  const redraw = useCallback(() => bump((n) => n + 1), []);

  const playing = status?.playing ?? false;
  const recording = mode === "recording";
  const busy = pending !== null;
  const timing = project.timing;
  const beatSeconds = timing.bpm && timing.unit ? (60 / timing.bpm) * (4 / timing.unit) : null;
  const barSeconds = beatSeconds && timing.numerator ? beatSeconds * timing.numerator : null;
  const takes = useMemo(() => project.takes.filter((tk) => tk.state !== "recording"), [project.takes]);
  const takeById = useMemo(() => new Map(takes.map((tk) => [tk.id, tk])), [takes]);

  const fail = useCallback((e: unknown) => useStore.getState().setError(describeError(e)), []);

  // ---------- load / save ----------

  const reload = useCallback(async () => {
    try {
      const v = await api.arrangementGet();
      setArr(v.arrangement);
    } catch (e) {
      fail(e);
    }
  }, [fail, setArr]);

  useEffect(() => {
    void reload();
    return () => {
      void api.arrangementRelease().catch(() => undefined);
    };
  }, [reload]);

  /** Apply an edit: local state at once, the engine (and disk) right after. */
  const commit = useCallback(
    (next: Arrangement, { history = true }: { history?: boolean } = {}) => {
      const prev = arrRef.current;
      if (history && prev) {
        setUndo((u) => [...u.slice(-99), prev]);
        setRedo([]);
      }
      setArr(next);
      api
        .setArrangement(next)
        .then((v) => {
          // Only adopt the engine's copy if nothing newer was sent meanwhile.
          if (arrRef.current === next) setArr(v.arrangement);
        })
        .catch((e) => {
          fail(e);
          void reload();
        });
    },
    [fail, reload, setArr],
  );

  const doUndo = useCallback(() => {
    const cur = arrRef.current;
    setUndo((u) => {
      if (u.length === 0 || !cur) return u;
      const prev = u[u.length - 1];
      setRedo((r) => [...r.slice(-99), cur]);
      commit(prev, { history: false });
      return u.slice(0, -1);
    });
  }, [commit]);

  const doRedo = useCallback(() => {
    const cur = arrRef.current;
    setRedo((r) => {
      if (r.length === 0 || !cur) return r;
      const next = r[r.length - 1];
      setUndo((u) => [...u.slice(-99), cur]);
      commit(next, { history: false });
      return r.slice(0, -1);
    });
  }, [commit]);

  // Peaks of every take a clip refers to (once per take).
  useEffect(() => {
    if (!arr) return;
    const ids = new Set(arr.tracks.flatMap((tr) => tr.clips.map((c) => c.take_id)));
    for (const id of ids) {
      if (peaks.current.has(id)) continue;
      peaks.current.set(id, { sample_rate: 0, frames: 0, duration_seconds: 0, frames_per_bin: 0, min: [], max: [] });
      api
        .takePeaks(id, PEAK_BINS)
        .then((p) => {
          peaks.current.set(id, p);
          setPeaksVersion((v) => v + 1);
        })
        .catch(() => peaks.current.delete(id));
    }
  }, [arr]);

  /** Playable length of a take (trim applied), seconds. */
  const takeLength = useCallback(
    (id: string): number => {
      const p = peaks.current.get(id);
      if (p && p.duration_seconds > 0) return p.duration_seconds;
      const tk = takeById.get(id);
      if (!tk || tk.duration_seconds == null) return 0;
      return (tk.trim_out_seconds ?? tk.duration_seconds) - tk.trim_in_seconds;
    },
    [takeById],
  );

  // ---------- transport ----------

  useEffect(() => {
    if (!playing && !recording) return;
    const id = window.setInterval(() => {
      api
        .arrangementStatus()
        .then((st) => {
          if (!st) return;
          setStatus(st);
          if (st.finished && !useStore.getState().mode.startsWith("rec")) {
            setCursor(st.position_seconds);
          }
        })
        .catch(() => undefined);
      if (useStore.getState().mode === "recording") {
        api
          .recordingStatus()
          .then((r) => setRec(r))
          .catch(() => undefined);
      }
    }, POLL_MS);
    return () => window.clearInterval(id);
  }, [playing, recording, setCursor]);

  // Keep the playhead in view while it runs.
  const headSeconds = playing && status ? status.position_seconds : cursor;
  useEffect(() => {
    if (!playing) return;
    const w = wrapRef.current?.clientWidth ?? 0;
    const { pps: p, scroll: sc } = viewRef.current;
    const x = (headSeconds - sc) * p;
    if (x > w - 40 || x < 0) setScroll(Math.max(0, headSeconds - 40 / p));
  }, [headSeconds, playing]);

  const play = useCallback(
    async (from?: number) => {
      try {
        setError(null);
        const st = await api.arrangementPlay(prefs.selectedOutput, from ?? cursorRef.current);
        setStatus(st);
      } catch (e) {
        fail(e);
      }
    },
    [fail, prefs.selectedOutput, setError],
  );

  const pause = useCallback(async () => {
    try {
      const st = await api.arrangementPause();
      setStatus(st);
      if (st) setCursor(st.position_seconds);
    } catch (e) {
      fail(e);
    }
  }, [fail, setCursor]);

  const seek = useCallback(
    async (sec: number) => {
      const s = Math.max(0, sec);
      setCursor(s);
      if (statusRef.current) {
        try {
          const st = await api.arrangementSeek(s);
          setStatus(st);
        } catch (e) {
          fail(e);
        }
      }
    },
    [fail, setCursor],
  );

  const stop = useCallback(async () => {
    await pause();
    await seek(arrRef.current?.loop_region?.[0] ?? 0);
  }, [pause, seek]);

  // ---------- recording onto the tracks ----------

  const onRecord = useCallback(async () => {
    try {
      setError(null);
      setMessage(null);
      const at = statusRef.current?.playing ? statusRef.current.position_seconds : cursorRef.current;
      await api.arrangementPlay(prefs.selectedOutput, at);
      await api.startRecording({ input: prefs.selectedInput, output: prefs.selectedOutput, sample_rate: prefs.sampleRate, click: false, count_in_bars: 0 });
      recStart.current = { at, track: selectedTrack };
      setRec(null);
      setMode("recording");
    } catch (e) {
      fail(e);
      void api.arrangementPause().then((st) => setStatus(st)).catch(() => undefined);
    }
  }, [fail, prefs.selectedInput, prefs.selectedOutput, prefs.sampleRate, selectedTrack, setError, setMessage, setMode]);

  const onStopRecord = useCallback(async () => {
    const st = useStore.getState();
    st.setPending(t.finishing);
    try {
      const r = await api.stopRecording();
      await api.arrangementPause().then((s) => setStatus(s));
      setMode("idle");
      setRec(null);
      st.setProject(await api.projectState());
      const start = recStart.current;
      recStart.current = null;
      const cur = arrRef.current;
      if (start && cur) {
        const next = clone(cur);
        let track = next.tracks.find((tr) => tr.id === start.track);
        if (!track) {
          if (next.tracks.length >= MAX_TRACKS) throw new Error(t.maxTracks);
          track = { id: uid(), name: `${t.trackDefault} ${next.tracks.length + 1}`, gain_db: 0, pan: 0, mute: false, solo: false, clips: [] };
          next.tracks.push(track);
          setSelectedTrack(track.id);
        }
        const clip: Clip = { id: uid(), take_id: r.take.id, start: start.at, offset: 0, length: Math.max(MIN_CLIP_S, r.take.duration_seconds ?? 0), gain_db: 0, fade_in: 0, fade_out: 0, mute: false };
        track.clips.push(clip);
        commit(next);
        setSelected({ track: track.id, clip: clip.id });
        setCursor(start.at);
      }
      if (r.frames_lost > 0) st.setMessage(`${t.recordedOnto} · ${r.frames_lost} ${t.framesLost}`);
      else st.setMessage(t.recordedOnto);
    } catch (e) {
      fail(e);
      setMode("idle");
    } finally {
      useStore.getState().setPending(null);
    }
  }, [commit, fail, setCursor, setMode, t]);

  // ---------- editing ----------

  const addTrack = useCallback(() => {
    const cur = arrRef.current;
    if (!cur || cur.tracks.length >= MAX_TRACKS) return;
    const next = clone(cur);
    const tr: Track = { id: uid(), name: `${t.trackDefault} ${next.tracks.length + 1}`, gain_db: 0, pan: 0, mute: false, solo: false, clips: [] };
    next.tracks.push(tr);
    commit(next);
    setSelectedTrack(tr.id);
  }, [commit, t.trackDefault]);

  const patchTrack = useCallback(
    (id: string, f: (tr: Track) => void, history = true) => {
      const cur = arrRef.current;
      if (!cur) return;
      const next = clone(cur);
      const tr = next.tracks.find((x) => x.id === id);
      if (!tr) return;
      f(tr);
      commit(next, { history });
    },
    [commit],
  );

  const addTake = useCallback(
    (take: TakeView) => {
      const cur = arrRef.current;
      if (!cur) return;
      const next = clone(cur);
      let tr = next.tracks.find((x) => x.id === selectedTrack);
      if (!tr) {
        if (next.tracks.length >= MAX_TRACKS) {
          setError(t.maxTracks);
          return;
        }
        tr = { id: uid(), name: `${t.trackDefault} ${next.tracks.length + 1}`, gain_db: 0, pan: 0, mute: false, solo: false, clips: [] };
        next.tracks.push(tr);
        setSelectedTrack(tr.id);
      }
      const len = takeLength(take.id);
      if (len <= 0) {
        setError(t.takeNoLength);
        return;
      }
      const clip: Clip = { id: uid(), take_id: take.id, start: cursorRef.current, offset: 0, length: len, gain_db: 0, fade_in: 0, fade_out: 0, mute: false };
      tr.clips.push(clip);
      commit(next);
      setSelected({ track: tr.id, clip: clip.id });
    },
    [commit, selectedTrack, setError, t, takeLength],
  );

  const selectedClip = useMemo(() => {
    if (!arr || !selected) return null;
    const tr = arr.tracks.find((x) => x.id === selected.track);
    const c = tr?.clips.find((x) => x.id === selected.clip);
    return tr && c ? { track: tr, clip: c } : null;
  }, [arr, selected]);

  const patchClip = useCallback(
    (f: (c: Clip, tr: Track) => void) => {
      const cur = arrRef.current;
      if (!cur || !selected) return;
      const next = clone(cur);
      const tr = next.tracks.find((x) => x.id === selected.track);
      const c = tr?.clips.find((x) => x.id === selected.clip);
      if (!tr || !c) return;
      f(c, tr);
      commit(next);
    },
    [commit, selected],
  );

  const deleteClip = useCallback(() => {
    patchClip((c, tr) => {
      tr.clips = tr.clips.filter((x) => x.id !== c.id);
    });
    setSelected(null);
  }, [patchClip]);

  const splitClip = useCallback(() => {
    const at = playing && statusRef.current ? statusRef.current.position_seconds : cursorRef.current;
    patchClip((c, tr) => {
      if (at <= c.start + MIN_CLIP_S || at >= c.start + c.length - MIN_CLIP_S) return;
      const cut = at - c.start;
      const right: Clip = { ...c, id: uid(), start: at, offset: c.offset + cut, length: c.length - cut, fade_in: 0 };
      c.length = cut;
      c.fade_out = 0;
      tr.clips.push(right);
    });
  }, [patchClip, playing]);

  const duplicateClip = useCallback(() => {
    let newId: string | null = null;
    patchClip((c, tr) => {
      newId = uid();
      tr.clips.push({ ...c, id: newId, start: c.start + c.length });
    });
    if (newId && selected) setSelected({ track: selected.track, clip: newId });
  }, [patchClip, selected]);

  // ---------- geometry ----------

  const toX = useCallback((sec: number) => (sec - viewRef.current.scroll) * viewRef.current.pps, []);
  const toSec = useCallback((x: number) => x / viewRef.current.pps + viewRef.current.scroll, []);

  /** Snap a time to the grid, clip edges, the loop and the cursor when enabled. */
  const snapSec = useCallback(
    (sec: number, ignoreClip: string | null): number => {
      if (!snap) return Math.max(0, sec);
      const cur = preview.current ?? arrRef.current;
      const p = viewRef.current.pps;
      const cands: number[] = [0, cursorRef.current];
      if (cur) {
        for (const tr of cur.tracks) for (const c of tr.clips) if (c.id !== ignoreClip) cands.push(c.start, c.start + c.length);
        if (cur.loop_region) cands.push(cur.loop_region[0], cur.loop_region[1]);
      }
      if (beatSeconds) {
        const grid = p * beatSeconds >= 12 ? beatSeconds : (barSeconds ?? beatSeconds);
        cands.push(Math.round(sec / grid) * grid);
      }
      let best = sec;
      let bestD = SNAP_PX / p;
      for (const c of cands) {
        const d = Math.abs(c - sec);
        if (d < bestD) {
          bestD = d;
          best = c;
        }
      }
      return Math.max(0, best);
    },
    [snap, beatSeconds, barSeconds],
  );

  const hit = useCallback(
    (x: number, y: number) => {
      const cur = preview.current ?? arrRef.current;
      if (!cur) return { lane: -1, sec: toSec(x), clip: null as { track: number; clip: Clip; edge: "L" | "R" | null } | null };
      const sec = toSec(x);
      if (y < RULER_H) return { lane: -1, sec, clip: null };
      const lane = Math.floor((y - RULER_H) / LANE_H);
      if (lane >= cur.tracks.length) return { lane, sec, clip: null };
      const tr = cur.tracks[lane];
      // Topmost = last drawn = last in the list.
      for (let i = tr.clips.length - 1; i >= 0; i--) {
        const c = tr.clips[i];
        const x0 = toX(c.start);
        const x1 = toX(c.start + c.length);
        if (x >= x0 - EDGE_PX && x <= x1 + EDGE_PX) {
          const edge = x < x0 + EDGE_PX ? "L" : x > x1 - EDGE_PX ? "R" : null;
          if (edge || (x >= x0 && x <= x1)) return { lane, sec, clip: { track: lane, clip: c, edge } };
        }
      }
      return { lane, sec, clip: null };
    },
    [toSec, toX],
  );

  // ---------- pointer ----------

  const onPointerDown = (e: RPointerEvent<HTMLCanvasElement>) => {
    if (!arr) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    e.currentTarget.setPointerCapture(e.pointerId);
    e.currentTarget.focus();
    const h = hit(x, y);
    if (y < RULER_H) {
      drag.current = { kind: "ruler", anchor: h.sec, moved: false };
      return;
    }
    if (h.clip) {
      const tr = arr.tracks[h.clip.track];
      setSelected({ track: tr.id, clip: h.clip.clip.id });
      setSelectedTrack(tr.id);
      const origin = clone(arr);
      if (h.clip.edge === "L") drag.current = { kind: "trimL", track: h.clip.track, clip: h.clip.clip.id, origin, moved: false };
      else if (h.clip.edge === "R") drag.current = { kind: "trimR", track: h.clip.track, clip: h.clip.clip.id, origin, moved: false };
      else drag.current = { kind: "move", track: h.clip.track, clip: h.clip.clip.id, grab: h.sec - h.clip.clip.start, origin, moved: false, startX: x };
      return;
    }
    if (h.lane >= 0 && h.lane < arr.tracks.length) setSelectedTrack(arr.tracks[h.lane].id);
    setSelected(null);
    void seek(snapSec(h.sec, null));
  };

  const onPointerMove = (e: RPointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    if (d.kind === "none") {
      const h = hit(x, y);
      e.currentTarget.style.cursor = y < RULER_H ? "col-resize" : h.clip?.edge ? "ew-resize" : h.clip ? "grab" : "crosshair";
      return;
    }
    const cur = arrRef.current;
    if (!cur) return;
    if (d.kind === "ruler") {
      const sec = snapSec(toSec(x), null);
      if (Math.abs(sec - d.anchor) * viewRef.current.pps > 3) {
        d.moved = true;
        const next = clone(cur);
        next.loop_region = [Math.min(sec, d.anchor), Math.max(sec, d.anchor)];
        if (next.loop_region[1] - next.loop_region[0] < MIN_CLIP_S) next.loop_region = null;
        preview.current = next;
        redraw();
      }
      return;
    }
    const next = clone(d.origin);
    const srcTrack = next.tracks[d.track];
    const idx = srcTrack.clips.findIndex((c) => c.id === d.clip);
    if (idx < 0) return;
    const c = srcTrack.clips[idx];
    if (d.kind === "move") {
      if (!d.moved && Math.abs(x - d.startX) < 3) return;
      d.moved = true;
      c.start = snapSec(toSec(x) - d.grab, c.id);
      const lane = Math.min(next.tracks.length - 1, Math.max(0, Math.floor((y - RULER_H) / LANE_H)));
      if (lane !== d.track) {
        srcTrack.clips.splice(idx, 1);
        next.tracks[lane].clips.push(c);
      }
    } else if (d.kind === "trimL") {
      d.moved = true;
      const minStart = c.start - c.offset;
      const maxStart = c.start + c.length - MIN_CLIP_S;
      const s = Math.min(maxStart, Math.max(minStart, snapSec(toSec(x), c.id)));
      const delta = s - c.start;
      c.start = s;
      c.offset += delta;
      c.length -= delta;
    } else {
      d.moved = true;
      const avail = takeLength(c.take_id);
      const maxLen = avail > 0 ? Math.max(MIN_CLIP_S, avail - c.offset) : Number.POSITIVE_INFINITY;
      const end = Math.min(c.start + maxLen, Math.max(c.start + MIN_CLIP_S, snapSec(toSec(x), c.id)));
      c.length = end - c.start;
    }
    preview.current = next;
    redraw();
  };

  const onPointerUp = (e: RPointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    drag.current = { kind: "none" };
    e.currentTarget.releasePointerCapture(e.pointerId);
    const p = preview.current;
    preview.current = null;
    if (d.kind === "ruler") {
      if (d.moved && p) commit(p);
      else void seek(snapSec(d.anchor, null));
      return;
    }
    if (d.kind !== "none" && d.moved && p) {
      commit(p);
      if (d.kind === "move") {
        const tr = p.tracks.find((x) => x.clips.some((c) => c.id === d.clip));
        if (tr) {
          setSelected({ track: tr.id, clip: d.clip });
          setSelectedTrack(tr.id);
        }
      }
    } else redraw();
  };

  const onWheel = (e: RWheelEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    if (e.ctrlKey) {
      const { pps: p, scroll: sc } = viewRef.current;
      const factor = Math.exp(-e.deltaY * 0.0015);
      const np = Math.min(MAX_PPS, Math.max(MIN_PPS, p * factor));
      const secAt = x / p + sc;
      setPps(np);
      setScroll(Math.max(0, secAt - x / np));
    } else {
      const delta = (e.deltaX || e.deltaY) / viewRef.current.pps;
      setScroll((s) => Math.max(0, s + delta));
    }
  };

  // ---------- keyboard ----------

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTypingTarget(e.target) || removeTrack) return;
      const ctrl = e.ctrlKey || e.metaKey;
      if (e.code === "Space") {
        e.preventDefault();
        if (recording) void onStopRecord();
        else if (playing) void pause();
        else void play();
      } else if (e.key === "Home") {
        e.preventDefault();
        void seek(0);
      } else if (e.key === "Escape") {
        if (recording) void onStopRecord();
        else void stop();
      } else if ((e.key === "Delete" || e.key === "Backspace") && selected) {
        e.preventDefault();
        deleteClip();
      } else if ((e.key === "s" || e.key === "S") && !ctrl && selected) {
        splitClip();
      } else if ((e.key === "d" || e.key === "D") && ctrl && selected) {
        e.preventDefault();
        duplicateClip();
      } else if ((e.key === "z" || e.key === "Z") && ctrl) {
        e.preventDefault();
        if (e.shiftKey) doRedo();
        else doUndo();
      } else if ((e.key === "y" || e.key === "Y") && ctrl) {
        e.preventDefault();
        doRedo();
      } else if ((e.key === "r" || e.key === "R") && !ctrl) {
        if (recording) void onStopRecord();
        else if (!busy) void onRecord();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, deleteClip, doRedo, doUndo, duplicateClip, onRecord, onStopRecord, pause, play, playing, recording, removeTrack, seek, selected, splitClip, stop]);

  // ---------- drawing ----------

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;
    const a = preview.current ?? arr;
    const dpr = window.devicePixelRatio || 1;
    const w = wrap.clientWidth;
    const h = RULER_H + LANE_H * ((a?.tracks.length ?? 0) + 1);
    if (canvas.width !== Math.floor(w * dpr) || canvas.height !== Math.floor(h * dpr)) {
      canvas.width = Math.floor(w * dpr);
      canvas.height = Math.floor(h * dpr);
      canvas.style.height = `${h}px`;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const style = getComputedStyle(canvas);
    const col = (name: string, fb: string) => style.getPropertyValue(name).trim() || fb;
    const colPanel = col("--panel", "#1a1d22");
    const colAxis = col("--axis", "#2b3038");
    const colText = col("--text", "#e8e6e1");
    const colMuted = col("--muted", "#888");
    const colHead = col("--playhead", "#ffb454");
    const colSel = col("--selection", "rgba(255,180,84,0.22)");
    const colAccent = col("--accent", "#7fd6c2");
    const colRec = col("--rec", "#ff5c5c");
    ctx.fillStyle = colPanel;
    ctx.fillRect(0, 0, w, h);

    // Ruler: seconds, and bars when the project has tempo and meter.
    const stepCands = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300];
    const step = stepCands.find((s) => s * pps >= 70) ?? 600;
    ctx.font = "11px system-ui, sans-serif";
    ctx.textBaseline = "top";
    ctx.strokeStyle = colAxis;
    ctx.fillStyle = colMuted;
    const first = Math.floor(scroll / step) * step;
    for (let s = first; s < scroll + w / pps; s += step) {
      const x = Math.round(toX(s)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x, RULER_H - 8);
      ctx.lineTo(x, h);
      ctx.stroke();
      ctx.fillText(formatTime(s), x + 3, 3);
    }
    // Section markers (F4.3): accent lines with their names.
    for (const m of project.markers) {
      const x = Math.round(toX(m.at_seconds)) + 0.5;
      if (x < -40 || x > w + 40) continue;
      ctx.strokeStyle = colAccent;
      ctx.fillStyle = colAccent;
      ctx.globalAlpha = 0.9;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, h);
      ctx.stroke();
      ctx.fillText(m.name, x + 4, RULER_H - 23 < 3 ? 3 : 12);
      ctx.globalAlpha = 1;
    }
    if (barSeconds && barSeconds * pps >= 14) {
      ctx.strokeStyle = colMuted;
      ctx.fillStyle = colMuted;
      const b0 = Math.floor(scroll / barSeconds);
      for (let b = b0; b * barSeconds < scroll + w / pps; b++) {
        const x = Math.round(toX(b * barSeconds)) + 0.5;
        ctx.globalAlpha = 0.5;
        ctx.beginPath();
        ctx.moveTo(x, RULER_H - 4);
        ctx.lineTo(x, h);
        ctx.stroke();
        ctx.globalAlpha = 1;
        if (barSeconds * pps >= 40) ctx.fillText(String(b + 1), x + 3, RULER_H - 13);
        if (beatSeconds && beatSeconds * pps >= 10) {
          ctx.globalAlpha = 0.2;
          for (let k = 1; k < (timing.numerator ?? 1); k++) {
            const bx = Math.round(toX(b * barSeconds + k * beatSeconds)) + 0.5;
            ctx.beginPath();
            ctx.moveTo(bx, RULER_H);
            ctx.lineTo(bx, h);
            ctx.stroke();
          }
          ctx.globalAlpha = 1;
        }
      }
    }
    // Loop region.
    if (a?.loop_region) {
      const x0 = toX(a.loop_region[0]);
      const x1 = toX(a.loop_region[1]);
      ctx.fillStyle = colSel;
      ctx.fillRect(x0, 0, Math.max(1, x1 - x0), h);
      ctx.fillStyle = colHead;
      ctx.fillRect(x0, 0, 2, RULER_H);
      ctx.fillRect(x1 - 2, 0, 2, RULER_H);
    }
    ctx.strokeStyle = colAxis;
    ctx.beginPath();
    ctx.moveTo(0, RULER_H + 0.5);
    ctx.lineTo(w, RULER_H + 0.5);
    ctx.stroke();

    // Lanes and clips.
    if (a) {
      a.tracks.forEach((tr, ti) => {
        const y0 = RULER_H + ti * LANE_H;
        if (tr.id === selectedTrack) {
          ctx.fillStyle = "rgba(127, 214, 194, 0.05)";
          ctx.fillRect(0, y0, w, LANE_H);
        }
        ctx.strokeStyle = colAxis;
        ctx.beginPath();
        ctx.moveTo(0, y0 + LANE_H + 0.5);
        ctx.lineTo(w, y0 + LANE_H + 0.5);
        ctx.stroke();
        const anySolo = a.tracks.some((x) => x.solo);
        const audible = !tr.mute && (!anySolo || tr.solo);
        for (const c of tr.clips) {
          const x0 = toX(c.start);
          const x1 = toX(c.start + c.length);
          if (x1 < 0 || x0 > w) continue;
          const cw = Math.max(2, x1 - x0);
          const isSel = selected?.clip === c.id;
          ctx.globalAlpha = audible && !c.mute ? 1 : 0.35;
          ctx.fillStyle = trackColor(ti);
          ctx.globalAlpha *= 0.28;
          ctx.fillRect(x0, y0 + 3, cw, LANE_H - 6);
          ctx.globalAlpha = audible && !c.mute ? 1 : 0.35;
          // Waveform of the window into the take.
          const p = peaks.current.get(c.take_id);
          if (p && p.min.length > 0 && p.duration_seconds > 0) {
            const n = p.min.length;
            const b0 = (c.offset / p.duration_seconds) * n;
            const b1 = ((c.offset + c.length) / p.duration_seconds) * n;
            const mid = y0 + LANE_H / 2;
            const amp = (LANE_H / 2 - 6) * Math.min(2, Math.pow(10, c.gain_db / 20));
            ctx.fillStyle = trackColor(ti);
            ctx.beginPath();
            const px0 = Math.max(0, x0);
            const px1 = Math.min(w, x1);
            for (let x = px0; x < px1; x += 1) {
              const f0 = b0 + ((x - x0) / cw) * (b1 - b0);
              const f1 = b0 + ((x + 1 - x0) / cw) * (b1 - b0);
              let mn = 1;
              let mx = -1;
              for (let i = Math.floor(f0); i <= Math.min(n - 1, Math.floor(f1)); i++) {
                if (i < 0) continue;
                mn = Math.min(mn, p.min[i]);
                mx = Math.max(mx, p.max[i]);
              }
              if (mx < mn) continue;
              // Fades scale what is drawn, like what is heard.
              const tIn = (x - x0) / pps;
              const tOut = (x1 - x) / pps;
              let g = 1;
              if (c.fade_in > 0 && tIn < c.fade_in) g *= tIn / c.fade_in;
              if (c.fade_out > 0 && tOut < c.fade_out) g *= tOut / c.fade_out;
              const top = mid - Math.min(1, mx * g) * amp;
              const bot = mid - Math.max(-1, mn * g) * amp;
              ctx.rect(x, top, 1, Math.max(1, bot - top));
            }
            ctx.fill();
          }
          // Fade handles.
          ctx.strokeStyle = colText;
          ctx.globalAlpha = 0.6;
          if (c.fade_in > 0) {
            ctx.beginPath();
            ctx.moveTo(x0, y0 + LANE_H - 3);
            ctx.lineTo(x0 + c.fade_in * pps, y0 + 3);
            ctx.stroke();
          }
          if (c.fade_out > 0) {
            ctx.beginPath();
            ctx.moveTo(x1, y0 + LANE_H - 3);
            ctx.lineTo(x1 - c.fade_out * pps, y0 + 3);
            ctx.stroke();
          }
          ctx.globalAlpha = 1;
          ctx.strokeStyle = isSel ? colAccent : trackColor(ti);
          ctx.lineWidth = isSel ? 2 : 1;
          ctx.strokeRect(x0 + 0.5, y0 + 3.5, cw - 1, LANE_H - 7);
          ctx.lineWidth = 1;
          const tk = takeById.get(c.take_id);
          ctx.fillStyle = colText;
          ctx.save();
          ctx.beginPath();
          ctx.rect(x0, y0, cw, LANE_H);
          ctx.clip();
          ctx.fillText(`${tk ? takeName(tk, t.take) : c.take_id}${c.mute ? " · " + t.muteClip : ""}`, Math.max(x0, 0) + 5, y0 + 6);
          ctx.restore();
        }
      });
      // Empty lane at the bottom: where a new track would go.
      const yAdd = RULER_H + a.tracks.length * LANE_H;
      ctx.fillStyle = colMuted;
      ctx.font = "12px system-ui, sans-serif";
      ctx.fillText(a.tracks.length === 0 ? t.noTracksHint : t.addTrackHint, 8, yAdd + 10);
    }

    // Recording in progress: a growing red block on the target lane.
    if (recording && recStart.current && a) {
      const ti = Math.max(
        0,
        a.tracks.findIndex((x) => x.id === recStart.current?.track),
      );
      const lane = recStart.current.track && ti >= 0 ? ti : a.tracks.length;
      const y0 = RULER_H + lane * LANE_H;
      const x0 = toX(recStart.current.at);
      const x1 = toX(recStart.current.at + (rec?.seconds_captured ?? 0));
      ctx.fillStyle = colRec;
      ctx.globalAlpha = 0.35;
      ctx.fillRect(x0, y0 + 3, Math.max(2, x1 - x0), LANE_H - 6);
      ctx.globalAlpha = 1;
      ctx.fillStyle = colRec;
      ctx.fillText("● REC", x0 + 5, y0 + 6);
    }

    // Playhead / cursor.
    const hx = Math.round(toX(headSeconds)) + 0.5;
    ctx.strokeStyle = playing || recording ? colHead : colAccent;
    ctx.lineWidth = playing || recording ? 2 : 1;
    ctx.beginPath();
    ctx.moveTo(hx, 0);
    ctx.lineTo(hx, h);
    ctx.stroke();
    ctx.lineWidth = 1;
    // Redraw on resize.
  }, [arr, pps, scroll, selected, selectedTrack, headSeconds, playing, recording, rec, peaksVersion, barSeconds, beatSeconds, timing.numerator, takeById, t, toX, tick, project.markers]);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const ro = new ResizeObserver(() => redraw());
    ro.observe(wrap);
    return () => ro.disconnect();
  }, [redraw]);

  // ---------- export ----------

  const onExportMix = async () => {
    const st = useStore.getState();
    try {
      st.setError(null);
      const path = await save({ title: t.exportMix, defaultPath: `${project.name}-song.wav`, filters: [{ name: "WAV", extensions: ["wav"] }] });
      if (!path) return;
      st.setPending(t.exporting);
      // The background goes in exactly as it plays: the switched-on layers with their settings.
      const scale = st.project?.scale ?? null;
      const profile = st.project?.instrument ?? null;
      const dp = st.prefs.drone;
      const pp = st.prefs.pad;
      const dRoot = st.droneRootOverride ?? droneRoot(dp, scale, profile);
      const pRoot = padRoot(pp, dp, scale, profile);
      const padNotes = st.padChord ?? (pRoot === null ? null : voicingNotes(pp.voicing, pRoot, scale));
      const include = {
        clips: true,
        drone: st.layers.drone && dRoot !== null ? droneSettings(dp, dRoot) : null,
        pad: st.layers.pad && padNotes ? padSettings(pp, padNotes) : null,
        ambient: st.layers.ambient,
        rhythm: st.layers.rhythm ? { ...st.prefs.rhythm, chord: st.padChord, root_midi: dRoot, intonation: dp.intonation } : null,
      };
      const r = await api.exportSong(path, include, prefs.exportFormat, null, null, true);
      st.setMessage(`${t.mixExported}: ${r.path} (${formatBytes(r.bytes)}) · ${r.stems.length} ${t.stemsWord}`);
    } catch (e) {
      fail(e);
    } finally {
      useStore.getState().setPending(null);
    }
  };

  const duration = useMemo(() => (arr ? arr.tracks.reduce((m, tr) => tr.clips.reduce((mm, c) => Math.max(mm, c.start + c.length), m), 0) : 0), [arr]);
  const loopOn = !!arr?.loop_region;
  const toggleLoop = () => {
    if (!arr) return;
    const next = clone(arr);
    if (next.loop_region) next.loop_region = null;
    else if (selectedClip) next.loop_region = [selectedClip.clip.start, selectedClip.clip.start + selectedClip.clip.length];
    else if (duration > 0) next.loop_region = [0, duration];
    else return;
    commit(next);
  };

  const recDb = rec ? linearToDb(rec.peak) : -120;

  return (
    <main className="arrange" aria-busy={busy}>
      <section className="transport">
        <h2 className="arrange-title">{t.arrangeTitle}</h2>
        <label className="inline small" title={t.sectionsHint}>
          {t.sectionsLabel}
          <select
            value=""
            onChange={(e) => {
              const name = e.target.value;
              if (!name) return;
              const at = Math.round(headSeconds * 100) / 100;
              void api
                .setMarkers([...project.markers.filter((m) => Math.abs(m.at_seconds - at) > 0.01), { at_seconds: at, name }])
                .then((pv) => useStore.getState().setProject(pv))
                .catch(fail);
            }}
          >
            <option value="">+ {t.sectionAtCursor}</option>
            {SECTION_NAMES.map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
          {project.markers.length > 0 && (
            <button className="ghost small" title={t.sectionRemoveNearest} onClick={() => {
              const nearest = project.markers.reduce((best, m) => (Math.abs(m.at_seconds - headSeconds) < Math.abs(best.at_seconds - headSeconds) ? m : best), project.markers[0]);
              void api.setMarkers(project.markers.filter((m) => m !== nearest)).then((pv) => useStore.getState().setProject(pv)).catch(fail);
            }}>
              ✕
            </button>
          )}
        </label>
        <button className="ghost" onClick={() => useStore.getState().setScreen("songmap")} title={t.songMapOpenHint}>
          🗺 {t.songMapShort}
        </button>
        <button className="ghost" onClick={() => void seek(0)} title={t.toStart} aria-label={t.toStart}>
          ⏮
        </button>
        {playing ? (
          <button onClick={() => void pause()} aria-keyshortcuts="Space">
            ⏸ {t.pause}
          </button>
        ) : (
          <button onClick={() => void play()} disabled={!arr || recording} aria-keyshortcuts="Space">
            ▶ {t.play}
          </button>
        )}
        <button className="ghost" onClick={() => void stop()} disabled={!status} aria-keyshortcuts="Escape">
          ■ {t.stop}
        </button>
        {recording ? (
          <button className="danger" onClick={() => void onStopRecord()} aria-keyshortcuts="R Escape">
            ■ {t.stop}
          </button>
        ) : (
          <button className="rec" onClick={() => void onRecord()} disabled={busy || !arr} aria-keyshortcuts="R" title={t.recordOnto}>
            ● {t.record}
          </button>
        )}
        <label className="inline">
          <input type="checkbox" checked={loopOn} onChange={toggleLoop} disabled={!arr} />
          {t.loopRegion}
        </label>
        <label className="inline">
          <input type="checkbox" checked={snap} onChange={(e) => setSnap(e.target.checked)} />
          {t.snap}
        </label>
        <span className="mono-num time">
          {formatTime(recording && rec ? recStart.current!.at + rec.seconds_captured : headSeconds)} / {formatTime(duration)}
        </span>
        <label className="inline" title={t.zoom}>
          🔍
          <input type="range" min={Math.log(MIN_PPS)} max={Math.log(MAX_PPS)} step={0.01} value={Math.log(pps)} onChange={(e) => setPps(Math.exp(Number(e.target.value)))} style={{ width: 90 }} />
        </label>
        <button className="ghost small" onClick={doUndo} disabled={undo.length === 0} title={t.undo} aria-label={t.undo}>
          ↶
        </button>
        <button className="ghost small" onClick={doRedo} disabled={redo.length === 0} title={t.redo} aria-label={t.redo}>
          ↷
        </button>
        <span className="spacer" />
        <BackgroundTransport compact />
        {pending && (
          <span className="pending" role="status" aria-live="polite">
            {pending}
          </span>
        )}
        <span className={`status ${recording ? "recording" : playing ? "playing" : "idle"}`}>{recording ? t.recording : playing ? t.playing : t.idle}</span>
      </section>

      <div className="arrange-body">
        <div className="track-heads">
          <div className="ruler-spacer muted small">{project.name}</div>
          {arr?.tracks.map((tr, ti) => (
            <div key={tr.id} className={`track-head ${tr.id === selectedTrack ? "selected" : ""}`} style={{ borderLeftColor: trackColor(ti) }} onClick={() => setSelectedTrack(tr.id)}>
              {renaming === tr.id ? (
                <form
                  className="rename"
                  onSubmit={(e) => {
                    e.preventDefault();
                    patchTrack(tr.id, (x) => {
                      x.name = draft.trim() || x.name;
                    });
                    setRenaming(null);
                  }}
                >
                  <input value={draft} onChange={(e) => setDraft(e.target.value)} autoFocus onBlur={() => setRenaming(null)} aria-label={t.trackName} />
                </form>
              ) : (
                <button
                  className="ghost track-name"
                  onDoubleClick={() => {
                    setRenaming(tr.id);
                    setDraft(tr.name);
                  }}
                  title={t.renameHint}
                >
                  {tr.name}
                </button>
              )}
              <div className="track-ctl">
                <button className={`small ${tr.mute ? "on" : "ghost"}`} onClick={() => patchTrack(tr.id, (x) => void (x.mute = !x.mute))} title={t.mute} aria-pressed={tr.mute}>
                  {t.muteAbbr}
                </button>
                <button className={`small ${tr.solo ? "on" : "ghost"}`} onClick={() => patchTrack(tr.id, (x) => void (x.solo = !x.solo))} title={t.solo} aria-pressed={tr.solo}>
                  {t.soloAbbr}
                </button>
                <input
                  type="range"
                  min={-40}
                  max={6}
                  step={0.5}
                  value={tr.gain_db}
                  title={`${t.trackGain}: ${formatDb(tr.gain_db)}`}
                  aria-label={t.trackGain}
                  onChange={(e) => patchTrack(tr.id, (x) => void (x.gain_db = Number(e.target.value)), false)}
                />
                <span className="mono-num small">{formatDb(tr.gain_db)}</span>
                <button className="ghost small" onClick={() => setRemoveTrack(tr)} title={t.removeTrack} aria-label={t.removeTrack}>
                  ✕
                </button>
              </div>
            </div>
          ))}
          <div className="track-head add">
            <button className="ghost" onClick={addTrack} disabled={!arr || arr.tracks.length >= MAX_TRACKS}>
              + {t.addTrack}
            </button>
          </div>
        </div>
        <div className="timeline-wrap" ref={wrapRef}>
          <canvas
            ref={canvasRef}
            className="timeline"
            tabIndex={0}
            role="application"
            aria-label={t.arrangeTitle}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerUp}
            onWheel={onWheel}
          />
        </div>
      </div>

      <div className="arrange-bottom">
        <section className="panel takes-palette">
          <div className="panel-head">
            <h3>{t.takesPalette}</h3>
            <button
              className="ghost small"
              disabled={!arr || recording || busy}
              title={t.importAudioHint}
              onClick={() =>
                void (async () => {
                  const path = await open({ title: t.importAudio, multiple: false, directory: false, filters: [{ name: t.audioFiles, extensions: IMPORT_EXTENSIONS }] });
                  if (!path || Array.isArray(path)) return;
                  const st = useStore.getState();
                  try {
                    st.setError(null);
                    st.setPending(t.importing);
                    const pv = await api.importAudio(path);
                    st.setProject(pv);
                    const created = pv.takes.reduce((best, tk) => (tk.number > best.number ? tk : best), pv.takes[0]);
                    if (created) addTake(created);
                    st.setMessage(`${t.importedOk}: ${path}`);
                  } catch (e) {
                    fail(e);
                  } finally {
                    useStore.getState().setPending(null);
                  }
                })()
              }
            >
              ⤓ {t.importAudio}
            </button>
          </div>
          {takes.length === 0 && <div className="muted small">{t.noTakes}</div>}
          <ul>
            {takes.map((tk) => (
              <li key={tk.id}>
                <span className="take-name">{takeName(tk, t.take)}</span>
                <span className="muted small mono-num">{formatTime(takeLength(tk.id) || (tk.duration_seconds ?? 0))}</span>
                <button className="small" onClick={() => addTake(tk)} disabled={!arr || recording} title={t.addAtCursor}>
                  + {t.addAtCursor}
                </button>
              </li>
            ))}
          </ul>
          <div className="muted small">{t.paletteHint}</div>
        </section>

        <section className="panel clip-inspector">
          <h3>{t.clipTitle}</h3>
          {selectedClip ? (
            <>
              <div className="analysis-row">
                <b>{takeById.get(selectedClip.clip.take_id) ? takeName(takeById.get(selectedClip.clip.take_id)!, t.take) : selectedClip.clip.take_id}</b>
                <span className="muted small">
                  {selectedClip.track.name} · {t.startLabel} {formatTime(selectedClip.clip.start)} · {t.lengthLabel} {formatTime(selectedClip.clip.length)} · {t.offsetLabel} {formatTime(selectedClip.clip.offset)}
                </span>
              </div>
              <div className="drone-sliders">
                <label className="drone-slider">
                  <span>{t.clipGain}</span>
                  <input type="range" min={-40} max={12} step={0.5} value={selectedClip.clip.gain_db} onChange={(e) => patchClip((c) => void (c.gain_db = Number(e.target.value)))} />
                  <span className="mono-num">{formatDb(selectedClip.clip.gain_db)}</span>
                </label>
                <label className="drone-slider">
                  <span>{t.fadeIn}</span>
                  <input type="range" min={0} max={Math.min(5, selectedClip.clip.length)} step={0.01} value={selectedClip.clip.fade_in} onChange={(e) => patchClip((c) => void (c.fade_in = Number(e.target.value)))} />
                  <span className="mono-num">{selectedClip.clip.fade_in.toFixed(2)} s</span>
                </label>
                <label className="drone-slider">
                  <span>{t.fadeOut}</span>
                  <input type="range" min={0} max={Math.min(5, selectedClip.clip.length)} step={0.01} value={selectedClip.clip.fade_out} onChange={(e) => patchClip((c) => void (c.fade_out = Number(e.target.value)))} />
                  <span className="mono-num">{selectedClip.clip.fade_out.toFixed(2)} s</span>
                </label>
              </div>
              <div className="analysis-row">
                <label className="inline">
                  <input type="checkbox" checked={selectedClip.clip.mute} onChange={(e) => patchClip((c) => void (c.mute = e.target.checked))} />
                  {t.muteClip}
                </label>
                <button className="small" onClick={splitClip}>
                  {t.split}
                </button>
                <button className="small" onClick={duplicateClip}>
                  {t.duplicate}
                </button>
                <button className="small danger" onClick={deleteClip}>
                  {t.deleteClip}
                </button>
              </div>
            </>
          ) : (
            <div className="muted small">{t.noClipSelected}</div>
          )}
        </section>

        <section className="panel arrange-side">
          <h3>{t.mixTitle}</h3>
          <div className="meter-row">
            <span className="muted small">{recording ? t.level : t.mixTitle}</span>
            <div className="meter" role="meter" aria-valuemin={-60} aria-valuemax={0} aria-valuenow={Math.round(recording ? recDb : linearToDb(status?.master_peak ?? 0))}>
              <div className={`meter-fill ${(recording ? recDb : linearToDb(status?.master_peak ?? 0)) > -3 ? "hot" : ""}`} style={{ width: `${meterFraction(recording ? recDb : linearToDb(status?.master_peak ?? 0)) * 100}%` }} />
            </div>
            <span className="mono-num">{formatDb(recording ? recDb : linearToDb(status?.master_peak ?? 0))}</span>
            {rec && rec.clipped_samples > 0 && <span className="clip">{t.clip}</span>}
          </div>
          <button onClick={() => void onExportMix()} disabled={busy || duration <= 0 || recording}>
            {t.exportMix}
          </button>
          <div className="muted small">{t.exportMixHint}</div>
          <div className="muted small">{t.recLatencyNote}</div>
          <div className="muted small">{t.arrangeShortcuts}</div>
        </section>
      </div>

      {error && (
        <div className="banner error" role="alert">
          {t.errorPrefix}: {error}
        </div>
      )}
      {!error && message && (
        <div className="banner ok" role="status">
          {message}
        </div>
      )}

      {removeTrack && (
        <ConfirmDialog
          title={t.removeTrackQ}
          body={`${removeTrack.name}: ${removeTrack.clips.length} ${t.clipsCount}. ${t.removeTrackExplain}`}
          confirmLabel={t.removeTrack}
          cancelLabel={t.cancel}
          onConfirm={() => {
            const cur = arrRef.current;
            if (cur) {
              const next = clone(cur);
              next.tracks = next.tracks.filter((x) => x.id !== removeTrack.id);
              commit(next);
              if (selectedTrack === removeTrack.id) setSelectedTrack(null);
              if (selected?.track === removeTrack.id) setSelected(null);
            }
            setRemoveTrack(null);
          }}
          onCancel={() => setRemoveTrack(null)}
        />
      )}
    </main>
  );
}
