// The five areas of the app (UX audit 2026-10-31, §5.1 and §6 stage 3): Play / Background /
// Song / Learn / Library. Home shows one card per area; every existing screen keeps its
// `Screen` id and belongs to exactly one area, so presets (`go`), the assistant bridge and
// the smoke test keep working while stages 4 and 5 merge the screens into tabs. The
// breadcrumb in the top bar reads "Home › <area> › <screen>" from the same table.
import type { StringMessageKey } from "./i18n";
import type { Screen } from "./store";

export type AreaId = "play" | "background" | "song" | "learn" | "library";

export interface HomeArea {
  id: AreaId;
  /** Area name (a verb in the owner's words: play, sound behind, arrange, learn, keep). */
  title: StringMessageKey;
  /** One line: what you do here. */
  desc: StringMessageKey;
  /** The screen the card opens. */
  main: Screen;
  /** Other screens that belong to the area until they become its tabs (stages 4–5). */
  also: Screen[];
}

export const AREAS: HomeArea[] = [
  { id: "play", title: "hPlay", desc: "hPlayDesc", main: "play", also: ["tuner"] },
  { id: "background", title: "hBackground", desc: "hBackgroundDesc", main: "compose", also: ["ambient", "soundscapes", "rhythm", "instrument"] },
  { id: "song", title: "hSong", desc: "hSongDesc", main: "studio", also: ["songmap", "create", "start"] },
  { id: "learn", title: "hLearn", desc: "hLearnDesc", main: "academy", also: ["references"] },
  { id: "library", title: "hLibrary", desc: "hLibraryDesc", main: "library", also: ["samples"] },
];

export const AREA_IDS: AreaId[] = AREAS.map((a) => a.id);

/** The area a screen belongs to; Home and Help belong to none. */
export function areaOf(screen: Screen): HomeArea | null {
  return AREAS.find((a) => a.main === screen || a.also.includes(screen)) ?? null;
}

/** The main screen of an area id, or null for an unknown id (the bridge validates with it). */
export function areaMainScreen(id: string): Screen | null {
  return AREAS.find((a) => a.id === id)?.main ?? null;
}
