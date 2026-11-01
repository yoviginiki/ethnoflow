// The five areas of the app (UX audit 2026-10-31, §5.1 and §6 stages 3–5): Play /
// Background / Song / Learn / Library. Home shows one card per area; each area is one
// screen with tabs (AreaScreen.tsx) and every existing screen keeps its `Screen` id as
// its tab, so presets (`go`), the assistant bridge and the smoke test keep working. The
// breadcrumb in the top bar reads "Home › <area> › <screen>" from the same table.
import type { StringMessageKey } from "./i18n";
import type { Screen } from "./store";

export type AreaId = "play" | "background" | "song" | "learn" | "library";
/** Any screen but Home (Home is not inside an area). */
export type AreaScreenId = Exclude<Screen, "home">;

export interface HomeArea {
  id: AreaId;
  /** Area name (a verb in the owner's words: play, sound behind, arrange, learn, keep). */
  title: StringMessageKey;
  /** One line: what you do here. */
  desc: StringMessageKey;
  /** The screen the card opens. */
  main: AreaScreenId;
  /** The area's other tabs, in order after `main`. */
  also: AreaScreenId[];
}

export const AREAS: HomeArea[] = [
  { id: "play", title: "hPlay", desc: "hPlayDesc", main: "play", also: ["tuner"] },
  { id: "background", title: "hBackground", desc: "hBackgroundDesc", main: "compose", also: ["ambient", "rhythm", "instrument", "live"] },
  { id: "song", title: "hSong", desc: "hSongDesc", main: "studio", also: ["songmap", "create", "start"] },
  { id: "learn", title: "hLearn", desc: "hLearnDesc", main: "academy", also: ["references"] },
  { id: "library", title: "hLibrary", desc: "hLibraryDesc", main: "library", also: ["samples"] },
];

export const AREA_IDS: AreaId[] = AREAS.map((a) => a.id);

/** Screen ids kept only as aliases: "soundscapes" = Atmosphere in the modules view (store.setScreen maps it). */
export const ALIAS_SCREENS: Record<string, AreaScreenId> = { soundscapes: "ambient" };

/** The area a screen belongs to; Home and Help belong to none. */
export function areaOf(screen: Screen): HomeArea | null {
  if (screen === "home") return null;
  if (screen in ALIAS_SCREENS) screen = ALIAS_SCREENS[screen];
  return AREAS.find((a) => a.main === screen || a.also.includes(screen)) ?? null;
}

/** The tabs of an area: its main screen first, then the others. */
export function areaTabs(area: HomeArea): AreaScreenId[] {
  return [area.main, ...area.also];
}

/** Title of each screen: tab label, Home's links and the top-bar breadcrumb. */
export const SCREEN_TITLE: Record<AreaScreenId, StringMessageKey> = {
  play: "areaPlay",
  tuner: "areaTuner",
  compose: "bgTabDronePad",
  ambient: "areaAmbient",
  soundscapes: "areaSoundscapes",
  rhythm: "areaRhythm",
  instrument: "areaForge",
  live: "bgTabLive",
  studio: "areaStudio",
  songmap: "areaSongMap",
  create: "areaCreate",
  start: "areaStart",
  academy: "areaLearn",
  references: "areaReferences",
  library: "areaLibrary",
  samples: "areaSamples",
  help: "areaHelp",
};

/** Screens that work without an open project (the others show the project picker). */
export const NO_PROJECT_SCREENS: Screen[] = ["home", "help", "tuner", "library", "samples", "references", "academy", "create"];

/** The main screen of an area id, or null for an unknown id (the bridge validates with it). */
export function areaMainScreen(id: string): Screen | null {
  return AREAS.find((a) => a.id === id)?.main ?? null;
}
