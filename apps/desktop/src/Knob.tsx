import { useRef } from "react";

interface Props {
  value: number;
  min: number;
  max: number;
  step?: number;
  /** Value restored on double-click. */
  reset?: number;
  onChange: (v: number) => void;
  label: string;
  /** Text under the knob (e.g. "−12 dB"). */
  text?: string;
  size?: number;
  disabled?: boolean;
}

/**
 * A rotary knob drawn in SVG: drag up/down (or scroll, or arrow keys) to change,
 * double-click to reset. Used where a slider would be too small or where the
 * platform's vertical slider is unreliable (channel strips, part levels).
 */
export function Knob({ value, min, max, step = 1, reset, onChange, label, text, size = 44, disabled = false }: Props) {
  const drag = useRef<{ y: number; v: number } | null>(null);
  const range = max - min;
  const frac = range > 0 ? Math.min(1, Math.max(0, (value - min) / range)) : 0;
  const clamp = (v: number) => Math.min(max, Math.max(min, Math.round(v / step) * step));
  // 270° sweep from 7 o'clock to 5 o'clock.
  const start = -225;
  const angle = start + frac * 270;
  const r = size / 2 - 4;
  const c = size / 2;
  const toXY = (deg: number) => {
    const a = (deg * Math.PI) / 180;
    return [c + r * Math.cos(a), c + r * Math.sin(a)] as const;
  };
  const [sx, sy] = toXY(start);
  const [ex, ey] = toXY(angle);
  const [tx, ty] = toXY(start + 270);
  const large = frac * 270 > 180 ? 1 : 0;
  const arc = (from: readonly [number, number], to: readonly [number, number], big: number) => `M ${from[0]} ${from[1]} A ${r} ${r} 0 ${big} 1 ${to[0]} ${to[1]}`;
  const dot = toXY(angle);

  return (
    <div className={`knob ${disabled ? "disabled" : ""}`} style={{ width: size }}>
      <svg
        width={size}
        height={size}
        role="slider"
        tabIndex={disabled ? -1 : 0}
        aria-label={label}
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={value}
        aria-valuetext={text ?? String(value)}
        onPointerDown={(e) => {
          if (disabled) return;
          (e.target as Element).setPointerCapture?.(e.pointerId);
          drag.current = { y: e.clientY, v: value };
        }}
        onPointerMove={(e) => {
          if (!drag.current) return;
          // 150 px of travel = the whole range; Shift = fine.
          const px = e.shiftKey ? 600 : 150;
          onChange(clamp(drag.current.v + ((drag.current.y - e.clientY) / px) * range));
        }}
        onPointerUp={() => (drag.current = null)}
        onPointerCancel={() => (drag.current = null)}
        onDoubleClick={() => reset !== undefined && !disabled && onChange(reset)}
        onWheel={(e) => {
          if (disabled) return;
          e.preventDefault();
          onChange(clamp(value + (e.deltaY < 0 ? step : -step) * (e.shiftKey ? 1 : 2)));
        }}
        onKeyDown={(e) => {
          if (disabled) return;
          if (e.key === "ArrowUp" || e.key === "ArrowRight") onChange(clamp(value + step));
          else if (e.key === "ArrowDown" || e.key === "ArrowLeft") onChange(clamp(value - step));
          else if (e.key === "Home") onChange(min);
          else if (e.key === "End") onChange(max);
          else return;
          e.preventDefault();
        }}
      >
        <path d={arc([sx, sy], [tx, ty], 1)} className="knob-track" />
        {frac > 0.001 && <path d={arc([sx, sy], [ex, ey], large)} className="knob-arc" />}
        <circle cx={c} cy={c} r={r - 7} className="knob-body" />
        <circle cx={dot[0] * 0.78 + c * 0.22} cy={dot[1] * 0.78 + c * 0.22} r={2.5} className="knob-dot" />
      </svg>
      {text !== undefined && <span className="mono-num small knob-text">{text}</span>}
    </div>
  );
}
