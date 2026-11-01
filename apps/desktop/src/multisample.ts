// Multisample instruments (home-studio item 3, third slice): a folder of samples whose
// names carry a note ("Piano_C4.wav", "cello A#2", "flute-Bb3", "060.wav") becomes one
// Sampler layer with a zone per sample; the engine picks the nearest zone for every pitch
// and reads it at the small speed ratio that remains. Pure helpers, tested.
import type { SampleEntry } from "./api";

const NAMES: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11, H: 11 };

/** The MIDI note a file name carries, or null: "C4", "A#3", "Bb2", "C-1", "F#-1", or a bare MIDI number 12..120. C4 = 60. */
export function noteFromName(name: string): number | null {
  const base = name.replace(/\.[a-z0-9]+$/i, "");
  // Note name with octave, delimited by non-letters (so "cello" does not read as E).
  const m = base.match(/(?:^|[^A-Za-z])([A-Ha-h])([#b♯♭]?)(-?\d)(?![\d])/);
  if (m) {
    const letter = m[1].toUpperCase();
    const acc = m[2] === "#" || m[2] === "♯" ? 1 : m[2] === "b" || m[2] === "♭" ? -1 : 0;
    const octave = Number(m[3]);
    const midi = (octave + 1) * 12 + NAMES[letter] + acc;
    if (midi >= 0 && midi <= 127) return midi;
  }
  const n = base.match(/(?:^|[^\d])(\d{2,3})(?![\d])/);
  if (n) {
    const v = Number(n[1]);
    if (v >= 12 && v <= 120) return v;
  }
  return null;
}

export interface ZonePlan {
  entry: SampleEntry;
  root_midi: number;
}

/** The samples of one folder that carry a note, one per note (the first wins), sorted by pitch. */
export function planZones(entries: SampleEntry[], folder: string): ZonePlan[] {
  const seen = new Set<number>();
  const out: ZonePlan[] = [];
  const inFolder = entries.filter((e) => e.folder === folder || e.path.startsWith(folder.replace(/[\\/]+$/, "") + "\\") || e.path.startsWith(folder.replace(/[\\/]+$/, "") + "/"));
  // Shorter names first: "Piano_C4.wav" wins over "Piano_C4_alt.wav" for the same note.
  for (const e of inFolder.sort((a, b) => a.name.length - b.name.length || a.name.localeCompare(b.name))) {
    const midi = noteFromName(e.name);
    if (midi === null || seen.has(midi)) continue;
    seen.add(midi);
    out.push({ entry: e, root_midi: midi });
  }
  return out.sort((a, b) => a.root_midi - b.root_midi);
}

/** The register a set of zones covers, widened by a fifth at both ends (clamped to MIDI). */
export function zonesRegister(zones: { root_midi: number }[]): [number, number] {
  if (zones.length === 0) return [48, 72];
  const lo = Math.min(...zones.map((z) => z.root_midi));
  const hi = Math.max(...zones.map((z) => z.root_midi));
  return [Math.max(0, lo - 7), Math.min(127, hi + 7)];
}
