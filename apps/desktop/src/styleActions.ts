// Apply a style (starter + progression + Soundscapes world) in one go: the session (drone,
// pad, rhythm, harmony, progression, atmosphere rack), then the journey of the style's
// world into the Soundscapes rack, then the layer switches. Shared by "Start from a
// melody", "Create a song" and the bridge, so every path does the same thing.
import { applySession } from "./session";
import { worldById } from "./scapes";
import { startJourney } from "./scapesActions";
import type { Starter } from "./starters";
import { useStore } from "./store";

export async function applyStyle(st: Starter, opts: { play: boolean; seed: number }): Promise<void> {
  // Layers come last: the session must not switch the Soundscapes layer on before its rack exists.
  await applySession({ ...st.session, layers: { ...st.session.layers, scapes: false } }, opts.play);
  const world = worldById(st.world);
  if (world) await startJourney(world, { minutes: 8, seed: opts.seed, loop: true, play: false });
  if (opts.play && st.session.layers.scapes) useStore.getState().setLayer("scapes", true);
}
