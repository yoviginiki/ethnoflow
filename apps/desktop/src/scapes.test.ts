import { describe, expect, it } from "vitest";
import type { Ambient } from "./api";
import { DEFAULT_MACROS } from "./ambientPresets";
import { buildJourney, journeyWorld, phaseAt, PHASE_ROLES, WORLDS, worldById, type PhaseRole } from "./scapes";

const base: Ambient = { schema_version: 3, layers: [], macros: DEFAULT_MACROS, preset: null, tags: [], automation: [], program: { scenes: [], loop_region: null } };
const names: Record<PhaseRole, string> = { intro: "Intro", development: "Development", climax: "Climax", outro: "Outro" };
const build = (id: string, minutes = 8, seed = 7, loop = false) => buildJourney(worldById(id)!, { minutes, seed, loop, names, base, layerName: (k) => k });

describe("soundscapes journeys", () => {
  it("every world builds four contiguous scenes that fill the length, with at most 8 layers", () => {
    for (const w of WORLDS) {
      const r = build(w.id, 8);
      expect(r.layers.length).toBeGreaterThan(0);
      expect(r.layers.length).toBeLessThanOrEqual(8);
      expect(r.program.scenes.map((s) => s.name)).toEqual(PHASE_ROLES.map((p) => names[p]));
      let at = 0;
      for (const sc of r.program.scenes) {
        expect(sc.start_s).toBe(at);
        expect(sc.duration_s).toBeGreaterThan(0);
        expect(sc.transition_s).toBeLessThan(sc.duration_s);
        at += sc.duration_s;
      }
      expect(at).toBeCloseTo(8 * 60, -1);
      expect(journeyWorld(r)).toBe(w.id);
    }
  });

  it("the arc is real: events are silent in the intro and loud at the climax, macros rise then fall", () => {
    const r = build("ancient-forest");
    const [intro, , climax, outro] = r.program.scenes;
    const events = r.layers.filter((l) => l.source.kind === "pluck" || l.source.kind === "bells").map((l) => l.id);
    for (const id of events) {
      expect(intro.snapshot.layers.find((l) => l.id === id)!.gain_db).toBe(-60);
      expect(climax.snapshot.layers.find((l) => l.id === id)!.gain_db).toBeGreaterThan(-30);
      expect(outro.snapshot.layers.find((l) => l.id === id)!.gain_db).toBe(-60);
    }
    expect(climax.snapshot.macros.intensity).toBeGreaterThan(intro.snapshot.macros.intensity);
    expect(outro.snapshot.macros.brightness).toBeLessThan(climax.snapshot.macros.brightness);
    expect(r.automation).toHaveLength(3);
    for (const lane of r.automation) {
      expect(lane.target.layer).toBeNull();
      expect(lane.mode.mode).toBe("walk");
    }
  });

  it("the same seed gives the same layer seeds and pans; loop sets the whole region", () => {
    const a = build("deep-space", 5, 42, true);
    const b = build("deep-space", 5, 42, true);
    expect(a.layers.map((l) => [l.seed, l.pan])).toEqual(b.layers.map((l) => [l.seed, l.pan]));
    expect(a.program.loop_region).toEqual([0, 300]);
    const c = build("deep-space", 5, 43, false);
    expect(c.layers.map((l) => l.seed)).not.toEqual(a.layers.map((l) => l.seed));
    expect(c.program.loop_region).toBeNull();
  });

  it("phaseAt follows the timeline and clamps at the end", () => {
    const r = build("rhodope-night", 10);
    expect(phaseAt(r, 0)?.role).toBe("intro");
    expect(phaseAt(r, 60 * 10 * 0.5)?.role).toBe("development");
    expect(phaseAt(r, 60 * 10 * 0.7)?.role).toBe("climax");
    const last = phaseAt(r, 10_000)!;
    expect(last.role).toBe("outro");
    expect(last.remaining_s).toBe(0);
    expect(phaseAt(base, 10)).toBeNull();
  });
});
