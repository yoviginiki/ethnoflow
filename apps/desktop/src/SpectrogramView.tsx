import { useEffect, useRef } from "react";
import type { Spectrogram } from "./api";

/**
 * The editor's spectrogram (A11): a dB grid painted as a heat map, low frequencies at the
 * bottom, time left to right over the same span as the waveform above it.
 */
export function SpectrogramView({ grid, label }: { grid: Spectrogram | null; label: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const w = canvas.clientWidth || 800;
    const h = canvas.clientHeight || 160;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.floor(w * dpr);
    canvas.height = Math.floor(h * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    if (!grid || grid.cols === 0 || grid.rows === 0) return;
    const cw = w / grid.cols;
    const rh = h / grid.rows;
    for (let r = 0; r < grid.rows; r++) {
      for (let c = 0; c < grid.cols; c++) {
        const db = grid.db[r * grid.cols + c];
        // −90 dB … 0 dB → dark blue … yellow.
        const v = Math.max(0, Math.min(1, (db + 90) / 90));
        const hue = 240 - v * 200;
        const light = 10 + v * 55;
        ctx.fillStyle = `hsl(${hue}, 85%, ${light}%)`;
        ctx.fillRect(c * cw, h - (r + 1) * rh, Math.ceil(cw), Math.ceil(rh));
      }
    }
    ctx.fillStyle = "rgba(255,255,255,0.75)";
    ctx.font = "10px system-ui, sans-serif";
    const marks = [100, 1000, 10000].filter((hz) => hz > grid.min_hz && hz < grid.max_hz);
    for (const hz of marks) {
      const frac = Math.log(hz / grid.min_hz) / Math.log(grid.max_hz / grid.min_hz);
      const y = h - frac * h;
      ctx.fillRect(0, y, 6, 1);
      ctx.fillText(hz >= 1000 ? `${hz / 1000}k` : String(hz), 8, y + 3);
    }
  }, [grid]);
  return <canvas ref={ref} className="spectrogram" role="img" aria-label={label} />;
}
