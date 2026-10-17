import { useEffect, useRef, useState } from "react";
import type { ContourView, Note, NoteClass, Scale } from "./api";
import { centsFromScale, classColor } from "./format";

interface Props {
  notes: Note[];
  contour: ContourView[];
  duration: number;
  cursor: number | null;
  playhead: number | null;
  selected: number | null;
  scale: Scale | null;
  noteNames: readonly string[];
  label: string;
  onSelect: (index: number | null) => void;
  onSeek: (seconds: number) => void;
  onKey: (e: React.KeyboardEvent<HTMLCanvasElement>) => void;
  /** A note was dragged: new start/end in seconds (pitch unchanged). */
  onDrag?: (index: number, start: number, end: number) => void;
  /** F2.11 note classes, index-aligned with `notes` (colour); optional. */
  classes?: (NoteClass | undefined)[];
  /** Alt+click on empty space: add a note at this time and pitch. */
  onAdd?: (start: number, midi: number) => void;
}

const LANE_PX = 10;
const MARGIN_PX = 2;
const GUTTER_PX = 34;
const EDGE_PX = 6;
const DRAG_THRESHOLD_PX = 3;
const MIN_NOTE_S = 0.03;
const NEW_NOTE_S = 0.25;

/**
 * Canvas piano roll aligned to the waveform's time axis: contour as a thin line,
 * notes as bars (shifted by their cents), lanes tinted when they belong to the
 * chosen scale. Editing is driven by the parent through `onKey`.
 */
export function PianoRoll({ notes, contour, duration, cursor, playhead, selected, scale, noteNames, label, onSelect, onSeek, onKey, onDrag, onAdd, classes }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ index: number; mode: "move" | "resize"; startX: number; origStart: number; origEnd: number; moved: boolean } | null>(null);
  /** Live preview of the dragged note (start, end) until the pointer is released. */
  const [preview, setPreview] = useState<{ index: number; start: number; end: number } | null>(null);

  // Pitch range from the data, with two semitones of air.
  const midis = [...notes.map((n) => n.midi), ...contour.filter((c) => c.midi !== null).map((c) => Math.round(c.midi!))];
  const lo = (midis.length ? Math.min(...midis) : 57) - MARGIN_PX;
  const hi = (midis.length ? Math.max(...midis) : 81) + MARGIN_PX;
  const lanes = hi - lo + 1;
  const height = lanes * LANE_PX;

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;
    const draw = () => {
      const dpr = window.devicePixelRatio || 1;
      const w = wrap.clientWidth;
      const h = height;
      if (canvas.width !== Math.floor(w * dpr) || canvas.height !== Math.floor(h * dpr)) {
        canvas.width = Math.floor(w * dpr);
        canvas.height = Math.floor(h * dpr);
      }
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      const style = getComputedStyle(canvas);
      const col = (v: string, d: string) => style.getPropertyValue(v).trim() || d;
      const colAxis = col("--axis", "#333");
      const colWave = col("--wave", "#7fd6c2");
      const colHead = col("--playhead", "#ffb454");
      const colText = col("--muted", "#888");
      const colSel = col("--selection", "rgba(255,180,84,0.22)");
      const colOut = col("--rec", "#ff5c5c");

      const x0 = GUTTER_PX;
      const toX = (sec: number) => x0 + (duration > 0 ? (sec / duration) * (w - x0) : 0);
      const toY = (midi: number) => (hi - midi) * LANE_PX; // top of the lane

      // Lanes: tint scale degrees, label C's and the tonic.
      ctx.font = "9px system-ui, sans-serif";
      for (let m = lo; m <= hi; m++) {
        const y = toY(m);
        const inScale = scale ? Math.abs(centsFromScale(scale, m)) < 1 : false;
        const pc = ((m % 12) + 12) % 12;
        ctx.fillStyle = inScale ? colSel : "transparent";
        if (inScale) ctx.fillRect(x0, y, w - x0, LANE_PX);
        ctx.strokeStyle = colAxis;
        ctx.beginPath();
        ctx.moveTo(x0, y + LANE_PX + 0.5);
        ctx.lineTo(w, y + LANE_PX + 0.5);
        ctx.stroke();
        if (pc === 0 || (scale && pc === scale.tonic_pc)) {
          ctx.fillStyle = colText;
          ctx.fillText(`${noteNames[pc]}${Math.floor(m / 12) - 1}`, 2, y + LANE_PX - 1);
        }
      }

      // Contour: thin polyline with gaps where unvoiced or unsure.
      ctx.strokeStyle = colWave;
      ctx.lineWidth = 1;
      ctx.globalAlpha = 0.9;
      ctx.beginPath();
      let pen = false;
      for (const c of contour) {
        if (c.midi === null || c.confidence < 0.5) {
          pen = false;
          continue;
        }
        const x = toX(c.t);
        const y = toY(c.midi) + LANE_PX / 2;
        if (pen) ctx.lineTo(x, y);
        else ctx.moveTo(x, y);
        pen = true;
      }
      ctx.stroke();
      ctx.globalAlpha = 1;

      // Notes as bars, offset by their cents; out-of-scale notes outlined in red.
      notes.forEach((n, i) => {
        const pv = preview && preview.index === i ? preview : null;
        const x = toX(pv ? pv.start : n.start);
        const wid = Math.max(2, toX(pv ? pv.end : n.end) - x);
        const y = toY(n.midi + n.cents / 100) + 1;
        const out = scale ? Math.abs(centsFromScale(scale, n.midi)) > 1 : false;
        const cls = classes?.[i];
        ctx.fillStyle = i === selected ? colHead : cls ? classColor(cls, colWave) : colWave;
        ctx.globalAlpha = (0.35 + 0.65 * Math.min(1, n.confidence)) * (cls === "noise" ? 0.5 : 1);
        ctx.fillRect(x, y, wid, LANE_PX - 2);
        ctx.globalAlpha = 1;
        if (out || cls === "probable_error") {
          ctx.strokeStyle = colOut;
          ctx.lineWidth = 1;
          if (cls === "probable_error") ctx.setLineDash([3, 2]);
          ctx.strokeRect(x + 0.5, y + 0.5, wid - 1, LANE_PX - 3);
          ctx.setLineDash([]);
        }
      });

      if (cursor !== null) {
        ctx.strokeStyle = colHead;
        ctx.setLineDash([4, 3]);
        ctx.beginPath();
        ctx.moveTo(toX(cursor) + 0.5, 0);
        ctx.lineTo(toX(cursor) + 0.5, h);
        ctx.stroke();
        ctx.setLineDash([]);
      }
      if (playhead !== null) {
        ctx.strokeStyle = colHead;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(toX(playhead), 0);
        ctx.lineTo(toX(playhead), h);
        ctx.stroke();
      }
    };
    draw();
    const ro = new ResizeObserver(draw);
    ro.observe(wrap);
    return () => ro.disconnect();
  }, [notes, contour, duration, cursor, playhead, selected, scale, noteNames, height, hi, lo, preview, classes]);

  const locate = (e: React.PointerEvent<HTMLCanvasElement> | React.MouseEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const pxPerSec = duration > 0 ? (rect.width - GUTTER_PX) / duration : 0;
    const sec = pxPerSec > 0 ? (x - GUTTER_PX) / pxPerSec : 0;
    const midi = hi - Math.floor(y / LANE_PX);
    const hit = notes.findIndex((n) => sec >= n.start && sec <= n.end && Math.abs(n.midi + n.cents / 100 - midi) < 0.75);
    const nearEnd = hit >= 0 && pxPerSec > 0 && (notes[hit].end - sec) * pxPerSec <= EDGE_PX;
    return { x, sec, midi, hit, nearEnd, pxPerSec };
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0 || duration <= 0) return;
    const { x, sec, midi, hit, nearEnd } = locate(e);
    if (x < GUTTER_PX) return;
    if (hit < 0) {
      if (e.altKey && onAdd) {
        onAdd(Math.max(0, sec), midi);
        return;
      }
      onSelect(null);
      onSeek(Math.max(0, Math.min(duration, sec)));
      return;
    }
    onSelect(hit);
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { index: hit, mode: nearEnd ? "resize" : "move", startX: e.clientX, origStart: notes[hit].start, origEnd: notes[hit].end, moved: false };
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (!d) {
      const { hit, nearEnd } = locate(e);
      e.currentTarget.style.cursor = hit >= 0 ? (nearEnd ? "ew-resize" : "grab") : "crosshair";
      return;
    }
    const dx = e.clientX - d.startX;
    if (!d.moved && Math.abs(dx) < DRAG_THRESHOLD_PX) return;
    d.moved = true;
    const { pxPerSec } = locate(e);
    const dt = pxPerSec > 0 ? dx / pxPerSec : 0;
    if (d.mode === "move") {
      const len = d.origEnd - d.origStart;
      const start = Math.max(0, Math.min(duration - len, d.origStart + dt));
      setPreview({ index: d.index, start, end: start + len });
    } else {
      const end = Math.max(d.origStart + MIN_NOTE_S, Math.min(duration, d.origEnd + dt));
      setPreview({ index: d.index, start: d.origStart, end });
    }
  };

  const onPointerUp = () => {
    const d = drag.current;
    drag.current = null;
    if (d && d.moved && preview && onDrag) onDrag(preview.index, preview.start, preview.end);
    setPreview(null);
  };

  return (
    <div ref={wrapRef} className="pianoroll-wrap" style={{ height }}>
      <canvas
        ref={canvasRef}
        className="pianoroll"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => {
          drag.current = null;
          setPreview(null);
        }}
        onKeyDown={onKey}
        role="img"
        aria-label={label}
        tabIndex={0}
      />
    </div>
  );
}

export { NEW_NOTE_S };
