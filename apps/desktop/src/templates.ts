// Project templates by style (home-studio item 8, 2026-11-01): a new project can start
// with one of the nine styles already set — drone, pad, rhythm, atmosphere, progression
// and Soundscapes world — through the same `applyStyle` the Start screen uses. Nothing
// is analysed and no take exists yet: the starter is built without a scale (it then
// plays the project's defaults) and with the project's timing.
import { api } from "./api";
import { messages, type StringMessageKey } from "./i18n";
import { proposeStarters, STARTER_IDS, type StarterId } from "./starters";
import { useStore } from "./store";
import { applyStyle } from "./styleActions";

export const STARTER_TITLE: Record<StarterId, StringMessageKey> = {
  quiet_ambient: "varQuietAmbient",
  dark_drone: "varDarkDrone",
  liquid_dnb: "varLiquidDnb",
  ambient_dnb: "varAmbientDnb",
  ethno_pulse: "varEthnoPulse",
  rnb_soul: "varRnbSoul",
  rock_band: "varRockBand",
  pop_air: "varPopAir",
  jazz_night: "varJazzNight",
};

export function isStarterId(x: unknown): x is StarterId {
  return typeof x === "string" && (STARTER_IDS as string[]).includes(x);
}

/** Wait until the project's atmosphere rack is loaded (the style needs it as a base). */
async function waitForRack(ms = 6000): Promise<void> {
  const t0 = Date.now();
  while (!useStore.getState().ambient) {
    if (Date.now() - t0 > ms) throw new Error("the atmosphere rack did not load");
    await new Promise((r) => setTimeout(r, 100));
  }
}

/** Apply a style template to the open project (after creation). Returns the starter id. */
export async function applyTemplate(style: StarterId, play = false): Promise<StarterId> {
  await waitForRack();
  const st = useStore.getState();
  const t = messages[st.prefs.lang];
  const p = st.project;
  if (!p || !st.ambient) throw new Error("no project");
  const names = { noise: t.kindNoise, wind: t.kindWind, texture: t.kindTexture, bells: t.kindBells, pluck: t.kindPluck, loop: t.kindLoop, cloud: t.kindCloud, resonator: t.kindResonator, choir: t.kindChoir, bloom: t.kindBloom, sampler: t.kindSampler };
  const starters = proposeStarters({ scale: p.scale, timing: p.timing, detectedBpm: null, register: null, baseRack: st.ambient, names });
  const starter = starters.find((s) => s.id === style);
  if (!starter) throw new Error(`no starter ${style}`);
  if (starter.timing && JSON.stringify(starter.timing) !== JSON.stringify(p.timing)) {
    useStore.getState().setProject(await api.setTiming(starter.timing));
  }
  await applyStyle(starter, { play, seed: 1 });
  return starter.id;
}
