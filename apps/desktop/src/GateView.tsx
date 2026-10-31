import { useEffect, useRef } from "react";
import type { Note } from "./api";
import { gateVerdict, glide, pitchRange, timeWindow, xOfTime, yOfMidi, type GateVerdict } from "./gate";
import { hzToMidi } from "./format";

interface Props {
  notes: Note[];
  /** The trainer's position (seconds): the ball stands here, in front of the next gate. */
  position: number;
  /** Trainer Wait mode: the note being waited for, red when the last reading was wrong. */
  target: { index: number; wrong: boolean } | null;
  hz: number | null;
  confidence: number;
  a4: number;
  /** Half-height of the opening, in cents (the trainer's level). */
  tolCents: number;
  noteNames: readonly string[];
  label: string;
  running: boolean;
  texts: { pass: string; high: string; low: string; silent: string; idle: string; done: string };
  onSeek?: (seconds: number) => void;
}

const BLACK = new Set([1, 3, 6, 8, 10]);
const LEFT_AXIS = 44;
const BALL_R = 9;
const TRAIL_S = 2.5;
const MIN_CONFIDENCE = 0.6;

/**
 * The "gate" game: time left→right, pitch bottom→top. Every expected note is a wall at
 * its start with an opening at its pitch (the tolerance band); the ball is the live
 * pitch. In the trainer's Wait mode the song advances only when the ball passes.
 */
export function GateView({ notes, position, target, hz, confidence, a4, tolCents, noteNames, label, running, texts, onSeek }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const shown = useRef(position); // animated position
  const live = useRef<number | null>(null); // smoothed pitch
  const trail = useRef<{ t: number; midi: number }[]>([]);
  const latest = useRef({ notes, position, target, hz, confidence, a4, tolCents, noteNames, running, texts });
  latest.current = { notes, position, target, hz, confidence, a4, tolCents, noteNames, running, texts };

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap) return;
    let raf = 0;
    let last = performance.now();
    const draw = () => {
      const now = performance.now();
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      const L = latest.current;
      shown.current = glide(shown.current, L.position, dt);
      const midiFrac = L.hz !== null && L.confidence >= MIN_CONFIDENCE ? hzToMidi(L.hz, L.a4) : null;
      live.current = midiFrac === null ? null : live.current === null ? midiFrac : live.current + (midiFrac - live.current) * Math.min(1, dt * 14);
      const ts = now / 1000;
      if (live.current !== null) trail.current.push({ t: ts, midi: live.current });
      trail.current = trail.current.filter((p) => ts - p.t <= TRAIL_S);

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
      const colOut = col("--rec", "#ff5c5c");
      const colFg = col("--text", "#e8e6e1");

      const range = pitchRange(L.notes);
      const plotH = h - 18;
      const plotW = w - LEFT_AXIS;
      const yOf = (m: number) => yOfMidi(m, range, plotH);
      const semi = plotH / (range[1] - range[0]);
      const win = timeWindow(shown.current);
      const xOf = (t: number) => LEFT_AXIS + xOfTime(t, win, plotW);

      // Pitch grid and names on the left.
      ctx.font = "10px system-ui, sans-serif";
      ctx.textAlign = "right";
      for (let m = Math.ceil(range[0]); m <= Math.floor(range[1]); m++) {
        const pc = ((m % 12) + 12) % 12;
        const y = yOf(m);
        ctx.strokeStyle = colAxis;
        ctx.globalAlpha = BLACK.has(pc) ? 0.35 : 0.7;
        ctx.beginPath();
        ctx.moveTo(LEFT_AXIS, y + 0.5);
        ctx.lineTo(w, y + 0.5);
        ctx.stroke();
        ctx.globalAlpha = 1;
        if (!BLACK.has(pc) && (semi >= 11 || pc === 0 || pc === 5)) {
          ctx.fillStyle = colText;
          ctx.fillText(`${L.noteNames[pc]}${Math.floor(m / 12) - 1}`, LEFT_AXIS - 4, y + 3.5);
        }
      }

      // Walls and corridors.
      const opening = (L.tolCents / 100) * semi; // half-height in px
      const groupStart = L.target ? L.notes[L.target.index]?.start : null;
      L.notes.forEach((n, i) => {
        if (n.end < win[0] || n.start > win[1]) return;
        const isTarget = L.target != null && (i === L.target.index || (groupStart !== null && Math.abs(n.start - groupStart) <= 0.03));
        const passed = L.running ? groupStart !== null && n.start < groupStart - 0.03 : n.end <= L.position;
        const y = yOf(n.midi);
        const x1 = xOf(n.start);
        const x2 = xOf(n.end);
        const colour = isTarget ? (L.target!.wrong ? colOut : colHead) : passed ? colText : colWave;
        // Corridor: the note's duration at its pitch.
        ctx.fillStyle = colour;
        ctx.globalAlpha = isTarget ? 0.45 : passed ? 0.2 : 0.3;
        ctx.beginPath();
        ctx.roundRect(Math.max(LEFT_AXIS, x1), y - opening, Math.max(4, x2 - x1), opening * 2, 4);
        ctx.fill();
        ctx.globalAlpha = 1;
        // Wall at the start with the opening.
        if (x1 >= LEFT_AXIS - 6 && x1 <= w) {
          ctx.fillStyle = colour;
          ctx.globalAlpha = isTarget ? 0.9 : passed ? 0.25 : 0.55;
          ctx.fillRect(x1 - 3, 0, 6, Math.max(0, y - opening));
          ctx.fillRect(x1 - 3, y + opening, 6, Math.max(0, plotH - (y + opening)));
          ctx.globalAlpha = 1;
          // Door frame.
          ctx.strokeStyle = colour;
          ctx.lineWidth = isTarget ? 2 : 1;
          ctx.strokeRect(x1 - 3.5, y - opening, 7, opening * 2);
          ctx.lineWidth = 1;
        }
        if (isTarget) {
          ctx.fillStyle = colFg;
          ctx.font = "bold 11px system-ui, sans-serif";
          ctx.textAlign = "left";
          const pc = ((n.midi % 12) + 12) % 12;
          ctx.fillText(`${L.noteNames[pc]}${Math.floor(n.midi / 12) - 1}`, x1 + 8, y - opening - 4);
          ctx.font = "10px system-ui, sans-serif";
        }
      });

      // Ball: the live pitch, standing at the current position; a trail behind it.
      const xBall = xOf(shown.current);
      const targetMidis = L.target && L.notes[L.target.index] ? L.notes.filter((n) => Math.abs(n.start - L.notes[L.target!.index].start) <= 0.03).map((n) => n.midi) : [];
      const v = L.running ? gateVerdict(live.current, targetMidis, L.tolCents) : { verdict: "silent" as GateVerdict, cents: null };
      const blocked = v.verdict === "high" || v.verdict === "low";
      const bx = blocked ? xBall - 12 : xBall;
      if (trail.current.length > 1) {
        ctx.strokeStyle = colHead;
        ctx.globalAlpha = 0.5;
        ctx.lineWidth = 2;
        ctx.beginPath();
        trail.current.forEach((p, i) => {
          const x = bx - (ts - p.t) * 60;
          const y = yOf(p.midi);
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        });
        ctx.stroke();
        ctx.globalAlpha = 1;
        ctx.lineWidth = 1;
      }
      if (live.current !== null) {
        const by = yOf(live.current);
        ctx.fillStyle = v.verdict === "pass" ? colWave : blocked ? colOut : colHead;
        ctx.beginPath();
        ctx.arc(bx, by, BALL_R, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = colFg;
        ctx.stroke();
        // Direction arrow when blocked.
        if (blocked) {
          ctx.fillStyle = colOut;
          ctx.font = "bold 16px system-ui, sans-serif";
          ctx.textAlign = "center";
          ctx.fillText(v.verdict === "low" ? "▲" : "▼", bx, v.verdict === "low" ? by - 14 : by + 26);
        }
      } else if (targetMidis.length) {
        // Resting: hollow ball at the target's height.
        ctx.strokeStyle = colText;
        ctx.setLineDash([3, 3]);
        ctx.beginPath();
        ctx.arc(xBall, yOf(targetMidis[0]), BALL_R, 0, Math.PI * 2);
        ctx.stroke();
        ctx.setLineDash([]);
      }

      // Status line.
      ctx.fillStyle = colFg;
      ctx.font = "12px system-ui, sans-serif";
      ctx.textAlign = "left";
      const msg = !L.running
        ? L.texts.idle
        : v.verdict === "pass"
          ? `✓ ${L.texts.pass}`
          : v.verdict === "high"
            ? `▼ ${L.texts.high} (+${v.cents!.toFixed(0)} c)`
            : v.verdict === "low"
              ? `▲ ${L.texts.low} (${v.cents!.toFixed(0)} c)`
              : L.texts.silent;
      ctx.fillText(msg, LEFT_AXIS + 6, h - 5);
      ctx.textAlign = "right";
      ctx.fillStyle = colText;
      ctx.fillText(`${shown.current.toFixed(1)} s`, w - 6, h - 5);

      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, []);

  const onClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!onSeek || running) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    if (x < LEFT_AXIS) return;
    const win = timeWindow(shown.current);
    onSeek(Math.max(0, win[0] + ((x - LEFT_AXIS) / (rect.width - LEFT_AXIS)) * (win[1] - win[0])));
  };

  return (
    <div ref={wrapRef} className="gate-wrap">
      <canvas ref={canvasRef} className="gate" onClick={onClick} role="img" aria-label={label} />
    </div>
  );
}
