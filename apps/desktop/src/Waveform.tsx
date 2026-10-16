import { useEffect, useRef } from "react";
import type { WaveformPeaks } from "./api";

interface Props {
  peaks: WaveformPeaks | null;
  /** Playhead position in seconds, or null when not playing. */
  playhead: number | null;
  /** Edit cursor in seconds (idle only), or null. */
  cursor: number | null;
  /** Selected region [start, end) in seconds, or null. */
  selection: [number, number] | null;
  /** Accessible name, e.g. the take's name. */
  label: string;
  /** Live recording: seconds captured so far. */
  recordingSeconds: number | null;
  onSeek?: (seconds: number) => void;
  onSelect?: (selection: [number, number] | null) => void;
  emptyText: string;
}

/** Pointer movement below this (px) is a click, above it a drag selection. */
const DRAG_THRESHOLD_PX = 4;

/**
 * Canvas waveform drawn from reduced min/max peaks. Click = cursor, drag = selection,
 * arrows move the cursor (Shift+arrows extend the selection).
 */
export function Waveform({ peaks, playhead, cursor, selection, label, recordingSeconds, onSeek, onSelect, emptyText }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ startX: number; startSec: number; moved: boolean } | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;

    const draw = () => {
      const dpr = window.devicePixelRatio || 1;
      const w = wrap.clientWidth;
      const h = wrap.clientHeight;
      if (canvas.width !== Math.floor(w * dpr) || canvas.height !== Math.floor(h * dpr)) {
        canvas.width = Math.floor(w * dpr);
        canvas.height = Math.floor(h * dpr);
      }
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);

      const style = getComputedStyle(canvas);
      const colWave = style.getPropertyValue("--wave").trim() || "#7fd6c2";
      const colAxis = style.getPropertyValue("--axis").trim() || "#333";
      const colHead = style.getPropertyValue("--playhead").trim() || "#ffb454";
      const colRec = style.getPropertyValue("--rec").trim() || "#ff5c5c";
      const colText = style.getPropertyValue("--muted").trim() || "#888";
      const colSel = style.getPropertyValue("--selection").trim() || "rgba(255, 180, 84, 0.22)";

      // centre line
      ctx.strokeStyle = colAxis;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, h / 2 + 0.5);
      ctx.lineTo(w, h / 2 + 0.5);
      ctx.stroke();

      if (recordingSeconds !== null) {
        // Live recording: the waveform grows from the left. The view spans at least one
        // minute and widens as the take gets longer.
        const span = Math.max(60, recordingSeconds);
        if (peaks && peaks.min.length > 0) {
          const used = Math.min(1, peaks.duration_seconds / span) * w;
          const n = peaks.min.length;
          const mid = h / 2;
          const amp = (h / 2) * 0.92;
          ctx.fillStyle = colRec;
          ctx.beginPath();
          for (let i = 0; i < n; i++) {
            const x0 = (i / n) * used;
            const x1 = ((i + 1) / n) * used;
            const top = mid - peaks.max[i] * amp;
            const bot = mid - peaks.min[i] * amp;
            ctx.rect(x0, top, Math.max(1, x1 - x0), Math.max(1, bot - top));
          }
          ctx.fill();
        }
        ctx.fillStyle = colText;
        ctx.font = "12px system-ui, sans-serif";
        ctx.fillText("● REC", 8, 16);
        return;
      }

      if (!peaks || peaks.min.length === 0) {
        ctx.fillStyle = colText;
        ctx.font = "13px system-ui, sans-serif";
        ctx.textAlign = "center";
        ctx.fillText(emptyText, w / 2, h / 2 - 8);
        ctx.textAlign = "left";
        return;
      }

      const dur = peaks.duration_seconds;
      const toX = (sec: number) => (dur > 0 ? Math.min(w, Math.max(0, (sec / dur) * w)) : 0);

      if (selection && dur > 0) {
        const x0 = toX(selection[0]);
        const x1 = toX(selection[1]);
        ctx.fillStyle = colSel;
        ctx.fillRect(x0, 0, Math.max(1, x1 - x0), h);
        ctx.strokeStyle = colHead;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(x0 + 0.5, 0);
        ctx.lineTo(x0 + 0.5, h);
        ctx.moveTo(x1 - 0.5, 0);
        ctx.lineTo(x1 - 0.5, h);
        ctx.stroke();
      }

      const n = peaks.min.length;
      const mid = h / 2;
      const amp = (h / 2) * 0.92;
      ctx.fillStyle = colWave;
      ctx.beginPath();
      for (let i = 0; i < n; i++) {
        const x0 = (i / n) * w;
        const x1 = ((i + 1) / n) * w;
        const top = mid - peaks.max[i] * amp;
        const bot = mid - peaks.min[i] * amp;
        ctx.rect(x0, top, Math.max(1, x1 - x0), Math.max(1, bot - top));
      }
      ctx.fill();

      // time ticks
      ctx.strokeStyle = colAxis;
      ctx.fillStyle = colText;
      ctx.font = "10px system-ui, sans-serif";
      if (dur > 0) {
        const step = dur > 600 ? 60 : dur > 120 ? 10 : dur > 30 ? 5 : 1;
        for (let t = 0; t <= dur; t += step) {
          const x = toX(t);
          ctx.beginPath();
          ctx.moveTo(x + 0.5, h - 10);
          ctx.lineTo(x + 0.5, h);
          ctx.stroke();
          ctx.fillText(`${t}s`, x + 2, h - 2);
        }
      }

      if (cursor !== null && dur > 0) {
        const x = toX(cursor);
        ctx.strokeStyle = colHead;
        ctx.lineWidth = 1;
        ctx.setLineDash([4, 3]);
        ctx.beginPath();
        ctx.moveTo(x + 0.5, 0);
        ctx.lineTo(x + 0.5, h);
        ctx.stroke();
        ctx.setLineDash([]);
      }

      if (playhead !== null && dur > 0) {
        const x = toX(playhead);
        ctx.strokeStyle = colHead;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
      }
    };

    draw();
    const ro = new ResizeObserver(draw);
    ro.observe(wrap);
    return () => ro.disconnect();
  }, [peaks, playhead, cursor, selection, recordingSeconds, emptyText]);

  const secondsAt = (clientX: number, el: HTMLCanvasElement) => {
    if (!peaks || peaks.duration_seconds <= 0) return null;
    const rect = el.getBoundingClientRect();
    const frac = (clientX - rect.left) / rect.width;
    return Math.max(0, Math.min(1, frac)) * peaks.duration_seconds;
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (e.button !== 0) return;
    const sec = secondsAt(e.clientX, e.currentTarget);
    if (sec === null) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { startX: e.clientX, startSec: sec, moved: false };
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    if (!d) return;
    if (!d.moved && Math.abs(e.clientX - d.startX) < DRAG_THRESHOLD_PX) return;
    d.moved = true;
    const sec = secondsAt(e.clientX, e.currentTarget);
    if (sec === null) return;
    onSelect?.(sec > d.startSec ? [d.startSec, sec] : [sec, d.startSec]);
  };

  const onPointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const d = drag.current;
    drag.current = null;
    if (!d) return;
    if (!d.moved) {
      onSeek?.(d.startSec);
      return;
    }
    const sec = secondsAt(e.clientX, e.currentTarget) ?? d.startSec;
    const sel: [number, number] = sec > d.startSec ? [d.startSec, sec] : [sec, d.startSec];
    if (sel[1] - sel[0] < 0.01) onSelect?.(null);
    else {
      onSelect?.(sel);
      onSeek?.(sel[0]);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    if (!peaks || peaks.duration_seconds <= 0) return;
    const dur = peaks.duration_seconds;
    const step = e.ctrlKey ? 0.1 : e.altKey ? 5 : 1;
    const at = cursor ?? playhead ?? 0;
    let next: number | null = null;
    if (e.key === "ArrowLeft") next = Math.max(0, at - step);
    else if (e.key === "ArrowRight") next = Math.min(dur, at + step);
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = dur;
    else if (e.key === "Escape" && selection) {
      e.preventDefault();
      onSelect?.(null);
      return;
    }
    if (next === null) return;
    e.preventDefault();
    if (e.shiftKey && onSelect) {
      // Extend the selection from its anchor (the end not being moved).
      const anchor = selection ? (Math.abs(at - selection[0]) < 1e-6 ? selection[1] : selection[0]) : at;
      onSelect(next > anchor ? [anchor, next] : [next, anchor]);
    }
    onSeek?.(next);
  };

  return (
    <div ref={wrapRef} className="waveform-wrap">
      <canvas
        ref={canvasRef}
        className="waveform"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => (drag.current = null)}
        onKeyDown={onKeyDown}
        role="img"
        aria-label={label}
        tabIndex={0}
      />
    </div>
  );
}
