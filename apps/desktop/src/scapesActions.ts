// Start / stop a Soundscapes journey: shared by the Soundscapes tab and the assistant
// bridge (`scapes_build`, `scapes_stop`), so the terminal goes through the owner's path.
import { api } from "./api";
import { messages } from "./i18n";
import { buildJourney, type PhaseRole, type World } from "./scapes";
import { recordChange } from "./rackHistory";
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
  if (!st.scapes) throw new Error("no Soundscapes rack loaded yet");
  const t = messages[st.prefs.lang];
  const names: Record<PhaseRole, string> = { intro: t.phaseIntro, development: t.phaseDevelopment, climax: t.phaseClimax, outro: t.phaseOutro };
  const rack = buildJourney(world, { minutes: opts.minutes, seed: opts.seed, loop: opts.loop, names, base: st.scapes, layerName: (k) => t[k] });
  const v = await api.setScapes(rack);
  recordChange("scapes", st.scapes, v.ambient, t.hJourney(t[world.title]), t.hStart);
  useStore.getState().setScapes(v.ambient);
  if (opts.play) {
    useStore.getState().setLayer("scapes", true);
    // The program clock exists once the rack runs; wait for its first status.
    const t0 = performance.now();
    while (useStore.getState().scapesStatus === null && performance.now() - t0 < 5000) await new Promise((r) => setTimeout(r, 50));
    const status = await api.scapesProgramSet(true, 0);
    useStore.getState().setScapesStatus(status);
  }
  return v.ambient;
}

/** Run / pause the journey's program and/or jump to a position (seconds). */
export async function seekJourney(playing: boolean | null, position_s: number | null) {
  const status = await api.scapesProgramSet(playing, position_s);
  useStore.getState().setScapesStatus(status);
  return status;
}

/** Stop the program and the Soundscapes layer (the rack and its scenes stay for editing). */
export async function stopJourney() {
  const st = useStore.getState();
  if (st.scapesStatus) {
    const status = await api.scapesProgramSet(false, 0).catch(() => null);
    if (status) useStore.getState().setScapesStatus(status);
  }
  if (st.layers.scapes) useStore.getState().setLayer("scapes", false);
}
