import type { LayerKind, LayerSource, Macros } from "./api";
import type { StringMessageKey } from "./i18n";

/** One continuous parameter of a layer source, as the UI shows it (slider or knob). */
export interface SliderSpec {
  key: string;
  label: StringMessageKey;
  min: number;
  max: number;
  step: number;
  /** How the value reads next to the control. */
  fmt: (v: number) => string;
}
export const pct = (v: number) => `${Math.round(v * 100)}%`;
export const REGISTER = Array.from({ length: 73 }, (_, i) => 36 + i); // C2 … C8

/** The parameters of every source kind, shared by Atmosphere (sliders) and Soundscapes (knobs). */
export const SOURCE_SLIDERS: Record<LayerKind, SliderSpec[]> = {
  noise: [{ key: "color", label: "colorLabel", min: 0, max: 1, step: 0.01, fmt: pct }],
  wind: [
    { key: "gust", label: "gustLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "speed", label: "speedLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "center_hz", label: "centerLabel", min: 80, max: 6000, step: 10, fmt: (v) => `${Math.round(v)} Hz` },
    { key: "resonance", label: "resonanceLabel", min: 0, max: 1, step: 0.01, fmt: pct },
  ],
  texture: [
    { key: "brightness", label: "brightnessLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "motion", label: "motionLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "resonance", label: "resonanceLabel", min: 0, max: 1, step: 0.01, fmt: pct },
  ],
  bells: [
    { key: "density", label: "densityLabel", min: 0, max: 60, step: 0.5, fmt: (v) => v.toFixed(1) },
    { key: "decay_s", label: "decayLabel", min: 0.2, max: 30, step: 0.1, fmt: (v) => `${v.toFixed(1)} s` },
    { key: "brightness", label: "brightnessLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "velocity_spread", label: "spreadLabel", min: 0, max: 1, step: 0.01, fmt: pct },
  ],
  pluck: [
    { key: "density", label: "densityLabel", min: 0, max: 120, step: 0.5, fmt: (v) => v.toFixed(1) },
    { key: "damping", label: "dampingLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "brightness", label: "brightnessLabel", min: 0, max: 1, step: 0.01, fmt: pct },
    { key: "velocity_spread", label: "spreadLabel", min: 0, max: 1, step: 0.01, fmt: pct },
  ],
  loop: [
    { key: "start", label: "loopStart", min: 0, max: 600, step: 0.01, fmt: (v) => `${v.toFixed(2)} s` },
    { key: "end", label: "loopEnd", min: 0, max: 600, step: 0.01, fmt: (v) => (v === 0 ? "—" : `${v.toFixed(2)} s`) },
    { key: "crossfade", label: "crossfadeLabel", min: 0, max: 5, step: 0.01, fmt: (v) => `${v.toFixed(2)} s` },
    { key: "rate", label: "rateLabel", min: 0.25, max: 4, step: 0.01, fmt: (v) => `×${v.toFixed(2)}` },
  ],
};

export const MACRO_SLIDERS: { key: keyof Macros; label: StringMessageKey }[] = [
  { key: "intensity", label: "macroIntensity" },
  { key: "brightness", label: "macroBrightness" },
  { key: "motion", label: "macroMotion" },
  { key: "space", label: "macroSpace" },
  { key: "space_size", label: "macroSpaceSize" },
  { key: "tonal_gravity", label: "macroGravity" },
  { key: "width", label: "macroWidth" },
];

export function hasTonal(src: LayerSource): src is Extract<LayerSource, { tonal: boolean }> {
  return "tonal" in src;
}
