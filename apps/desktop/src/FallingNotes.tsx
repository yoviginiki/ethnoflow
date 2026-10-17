import { useEffect, useRef } from "react";
import type { Note, Scale } from "./api";
import { centsFromScale } from "./format";

interface Props {
  notes: Note[];
  /** Current time in seconds (playhead or cursor). */
  position: number;
  /** Seconds of music visible above the keyboard. */
  lookahead: number;
  scale: Scale | null;
  noteNames: readonly string[];
  showNames: boolean;
  label: string;
  onSeek?: (seconds: number) => void;
  /** Trainer: index of the note being waited for, drawn lit (red when wrong). */
  target?: { index: number; wrong: boolean } | null;
}

const KEYBOARD_PX = 56;
const BLACK = new Set([1, 3, 6, 8, 10]);

/**
 * Synthesia/midiano-style view: notes fall towards a piano keyboard; a note reaches
 * the keys exactly at its start time. Keys light up while their note sounds.
 */
export function FallingNotes({ notes, position, lookahead, scale, noteNames, showNames, label, onSeek, target }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);

  // Key range: whole octaves around the notes (at least two octaves).
  const midis = notes.map((n) => n.midi);
  const lo = Math.floor(((midis.length ? Math.min(...midis) : 60) - 2) / 12) * 12;
  const hi = Math.ceil(((midis.length ? Math.max(...midis) : 72) + 2) / 12) * 12 - 1;
  const whites: number[] = [];
  for (let m = lo; m <= hi; m++) if (!BLACK.has(((m % 12) + 12) % 12)) whites.push(m);

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
      const col = (v: string, d: string) => style.getPropertyValue(v).trim() || d;
      const colAxis = col("--axis", "#333");
      const colWave = col("--wave", "#7fd6c2");
      const colHead = col("--playhead", "#ffb454");
      const colText = col("--muted", "#888");
      const colPanel = col("--panel", "#1a1d22");
      const colOut = col("--rec", "#ff5c5c");

      const whiteW = w / whites.length;
      const keyTop = h - KEYBOARD_PX;
      const xOfMidi = (m: number) => {
        // Black keys sit between the two whites around them.
        const pc = ((m % 12) + 12) % 12;
        if (!BLACK.has(pc)) return whites.indexOf(m) * whiteW;
        const leftWhite = whites.indexOf(m - 1);
        return (leftWhite + 1) * whiteW - whiteW * 0.3;
      };
      const widthOfMidi = (m: number) => (BLACK.has(((m % 12) + 12) % 12) ? whiteW * 0.6 : whiteW);
      const yOfTime = (t: number) => keyTop - ((t - position) / lookahead) * keyTop;

      // Falling notes (only the visible window).
      const sounding = new Set<number>();
      notes.forEach((n, i) => {
        if (n.end < position - 0.05 || n.start > position + lookahead) return;
        if (n.start <= position && n.end > position) sounding.add(n.midi);
        const isTarget = target != null && target.index === i;
        const yTop = Math.max(0, yOfTime(n.end));
        const yBot = Math.min(keyTop, yOfTime(n.start));
        if (yBot <= yTop) return;
        const x = xOfMidi(n.midi);
        const wid = widthOfMidi(n.midi) - 2;
        const out = scale ? Math.abs(centsFromScale(scale, n.midi)) > 1 : false;
        ctx.fillStyle = isTarget ? (target.wrong ? colOut : colHead) : out ? colOut : colWave;
        ctx.globalAlpha = n.start <= position || isTarget ? 1 : 0.75;
        ctx.beginPath();
        ctx.roundRect(x + 1, yTop, wid, yBot - yTop, 3);
        ctx.fill();
        ctx.globalAlpha = 1;
        if (isTarget) sounding.add(n.midi);
      });
      // Hit line.
      ctx.strokeStyle = colHead;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(0, keyTop);
      ctx.lineTo(w, keyTop);
      ctx.stroke();

      // Keyboard: whites then blacks.
      ctx.font = "10px system-ui, sans-serif";
      ctx.textAlign = "center";
      whites.forEach((m, i) => {
        const x = i * whiteW;
        ctx.fillStyle = sounding.has(m) ? colWave : "#f3f1ec";
        ctx.fillRect(x, keyTop, whiteW - 1, KEYBOARD_PX);
        ctx.strokeStyle = colAxis;
        ctx.strokeRect(x + 0.5, keyTop + 0.5, whiteW - 1, KEYBOARD_PX - 1);
        const pc = ((m % 12) + 12) % 12;
        if (showNames || pc === 0) {
          ctx.fillStyle = "#333";
          const name = pc === 0 ? `${noteNames[pc]}${Math.floor(m / 12) - 1}` : noteNames[pc];
          ctx.fillText(name, x + whiteW / 2, h - 6);
        }
      });
      for (let m = lo; m <= hi; m++) {
        const pc = ((m % 12) + 12) % 12;
        if (!BLACK.has(pc)) continue;
        const x = xOfMidi(m);
        const wid = widthOfMidi(m);
        ctx.fillStyle = sounding.has(m) ? colWave : colPanel;
        ctx.fillRect(x, keyTop, wid, KEYBOARD_PX * 0.62);
        ctx.strokeStyle = colAxis;
        ctx.strokeRect(x + 0.5, keyTop + 0.5, wid - 1, KEYBOARD_PX * 0.62);
      }
      ctx.textAlign = "left";
      ctx.fillStyle = colText;
      ctx.fillText(`${position.toFixed(1)} s`, 6, 14);
    };
    draw();
    const ro = new ResizeObserver(draw);
    ro.observe(wrap);
    return () => ro.disconnect();
  }, [notes, position, lookahead, scale, noteNames, showNames, lo, hi, whites, target]);

  const onClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!onSeek) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const keyTop = rect.height - KEYBOARD_PX;
    if (y >= keyTop) return;
    onSeek(Math.max(0, position + ((keyTop - y) / keyTop) * lookahead));
  };

  return (
    <div ref={wrapRef} className="falling-wrap">
      <canvas ref={canvasRef} className="falling" onClick={onClick} role="img" aria-label={label} />
    </div>
  );
}
