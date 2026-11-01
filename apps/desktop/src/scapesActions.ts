// Start / stop a Soundscapes journey: shared by the Soundscapes tab and the assistant
// bridge (`scapes_build`, `scapes_stop`), so the terminal goes through the owner's path.
import { api } from "./api";
import { messages } from "./i18n";
import { buildJourney, type PhaseRole, type World } from "./scapes";
import { useStore } from "./store";

export interface StartJourneyOptions {
  minutes: number;
  seed: number;
  loop: boolean;
  /** Start the sound (switch the atmosphere layer on and run the program). */
  play: boolean;
}

/** Build the journey into the project's rack; with `play`, start the atmosphere and the program from 0. */
export async function startJourney(world: World, opts: StartJourneyOptions) {
  const st = useStore.getState();
  if (!st.project) throw new Error("no project");
  if (!st.ambient) throw new Error("no rack loaded yet");
  const t = messages[st.prefs.lang];
  const names: Record<PhaseRole, string> = { intro: t.phaseIntro, development: t.phaseDevelopment, climax: t.phaseClimax, outro: t.phaseOutro };
  const rack = buildJourney(world, { minutes: opts.minutes, seed: opts.seed, loop: opts.loop, names, base: st.ambient, layerName: (k) => t[k] });
  const v = await api.setAmbient(rack);
  useStore.getState().setAmbient(v.ambient);
  if (opts.play) {
    useStore.getState().setLayer("ambient", true);
    // The program clock exists once the rack runs; wait for its first status.
    const t0 = performance.now();
    while (useStore.getState().ambientStatus === null && performance.now() - t0 < 5000) await new Promise((r) => setTimeout(r, 50));
    const status = await api.ambientProgramSet(true, 0);
    useStore.getState().setAmbientStatus(status);
  }
  return v.ambient;
}

/** Stop the program and the atmosphere layer (the rack and its scenes stay for editing). */
export async function stopJourney() {
  const st = useStore.getState();
  if (st.ambientStatus) {
    const status = await api.ambientProgramSet(false, 0).catch(() => null);
    if (status) useStore.getState().setAmbientStatus(status);
  }
  if (st.layers.ambient) useStore.getState().setLayer("ambient", false);
}
