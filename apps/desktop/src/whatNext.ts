// "What next" (home-studio item 8, 2026-11-01): one sentence on every screen that names
// the next sensible step of the home-studio path and the screen it lives on. Pure: it
// reads a small snapshot of the state, so it is unit-tested and the bridge can ask for it.
import type { StringMessageKey } from "./i18n";
import type { LayerName, Screen } from "./store";

export interface NextSnapshot {
  hasProject: boolean;
  takes: number;
  hasScale: boolean;
  /** A take is loaded in Play & record. */
  loaded: boolean;
  layersOn: Record<LayerName, boolean>;
  /** The arrangement has at least one clip (null = unknown / not loaded yet). */
  clips: number | null;
  exportedMs: number | null;
  screen: Screen;
}

export interface NextStep {
  key: StringMessageKey;
  screen: Screen;
}

/** The next step of the path: project → scale → record → background → studio → export. */
export function whatNext(s: NextSnapshot): NextStep {
  if (!s.hasProject) return { key: "nextCreateProject", screen: "home" };
  if (s.takes === 0) return { key: "nextRecord", screen: "play" };
  if (!s.hasScale) return { key: "nextScale", screen: "start" };
  const bg = s.layersOn.drone || s.layersOn.pad || s.layersOn.ambient || s.layersOn.rhythm || s.layersOn.scapes;
  if (!bg) return { key: "nextBackground", screen: "start" };
  if (s.clips === 0) return { key: "nextArrange", screen: "studio" };
  if (s.exportedMs === null) return { key: "nextExport", screen: "studio" };
  return { key: "nextPlayAlong", screen: s.screen === "play" ? "library" : "play" };
}
