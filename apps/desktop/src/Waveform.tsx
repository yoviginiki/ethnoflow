import { useEffect, useRef } from "react";
import type { WaveformPeaks } from "./api";

interface Props {
  peaks: WaveformPeaks | null;
  /** Playhead position in seconds, or null when not playing. */
  playhead: number | null;
  /** Live recording: seconds captured so far (draws a growing time bar). */
  recordingSeconds: number | null;
  onSeek?: (seconds: number) => void;
  emptyText: string;
}

/**
 * Canvas waveform drawn from reduced min/max peaks. Re-draws on resize and when
 * props change; the playhead is drawn in the same pass to keep it cheap.
 */
export function Waveform({ peaks, playhead, recordingSeconds, onSeek, emptyText }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);

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

      // centre line
      ctx.strokeStyle = colAxis;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, h / 2 + 0.5);
      ctx.lineTo(w, h / 2 + 0.5);
      ctx.stroke();

      if (recordingSeconds !== null) {
        // Live recording: show elapsed time as a growing bar; peaks arrive after stop.
        ctx.fillStyle = colRec;
        ctx.globalAlpha = 0.25;
        const frac = Math.min(1, recordingSeconds / 60); // one minute across the width
        ctx.fillRect(0, h * 0.4, w * frac, h * 0.2);
        ctx.globalAlpha = 1;
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

      // time ticks every second
      ctx.strokeStyle = colAxis;
      ctx.fillStyle = colText;
      ctx.font = "10px system-ui, sans-serif";
      const dur = peaks.duration_seconds;
      if (dur > 0) {
        const step = dur > 30 ? 5 : 1;
        for (let t = 0; t <= dur; t += step) {
          const x = (t / dur) * w;
          ctx.beginPath();
          ctx.moveTo(x + 0.5, h - 10);
          ctx.lineTo(x + 0.5, h);
          ctx.stroke();
          ctx.fillText(`${t}s`, x + 2, h - 2);
        }
      }

      if (playhead !== null && dur > 0) {
        const x = Math.min(w, (playhead / dur) * w);
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
  }, [peaks, playhead, recordingSeconds, emptyText]);

  const handleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!peaks || !onSeek || peaks.duration_seconds <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const frac = (e.clientX - rect.left) / rect.width;
    onSeek(Math.max(0, Math.min(1, frac)) * peaks.duration_seconds);
  };

  return (
    <div ref={wrapRef} className="waveform-wrap">
      <canvas
        ref={canvasRef}
        className="waveform"
        onClick={handleClick}
        role="img"
        aria-label="waveform"
        tabIndex={0}
      />
    </div>
  );
}
