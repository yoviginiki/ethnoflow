import { useEffect } from "react";
import { api, describeError, type DroneSettings, type InstrumentProfile, type PadNote, type PadSettings, type Scale } from "./api";
import { DEFAULT_DRONE, DEFAULT_PAD, useStore, type DronePrefs, type PadPrefs, type Voicing } from "./store";

/** Suggested root: the project's scale tonic, else the lowest calibrated note. Never a
 *  built-in key — if nothing is known the user picks one. */
export function suggestedRoot(scale: Scale | null, profile: InstrumentProfile | null): number | null {
  return scale ? 48 + scale.tonic_pc : (profile?.notes[0]?.midi ?? null);
}

export function droneRoot(prefs: DronePrefs, scale: Scale | null, profile: InstrumentProfile | null): number | null {
  return prefs.rootMidi ?? suggestedRoot(scale, profile);
}

export function padRoot(pad: PadPrefs, drone: DronePrefs, scale: Scale | null, profile: InstrumentProfile | null): number | null {
  return pad.rootMidi ?? droneRoot(drone, scale, profile);
}

/** Chord notes for a voicing: mode-neutral intervals, or degrees of the chosen scale. */
export function voicingNotes(voicing: Voicing, root: number, scale: Scale | null): PadNote[] | null {
  const plain = (semis: number[]) => semis.map((s) => ({ midi: root + s, cents: 0 }));
  switch (voicing) {
    case "fifth":
      return plain([0, 7, 12]);
    case "sus2":
      return plain([0, 2, 7]);
    case "sus4":
      return plain([0, 5, 7]);
    case "fifth9":
      return plain([0, 7, 14]);
    case "scale_triad":
    case "scale_7th": {
      if (!scale || scale.degrees_cents.length < 7) return null;
      const idx = voicing === "scale_triad" ? [0, 2, 4] : [0, 2, 4, 6];
      return idx.map((i) => {
        const c = scale.degrees_cents[i];
        const semis = Math.round(c / 100);
        return { midi: root + semis, cents: c - semis * 100 };
      });
    }
  }
}

export function droneSettings(p: DronePrefs, rootMidi: number): DroneSettings {
  return {
    root_midi: rootMidi,
    root_cents: p.rootCents,
    fifth: p.fifth,
    octave: p.octave,
    sub: p.sub,
    warmth: p.warmth,
    movement: p.movement,
    breath: p.breath,
    detune_cents: p.detuneCents,
    cutoff_hz: p.cutoffHz,
    level_db: p.levelDb,
    intonation: p.intonation,
  };
}

export function padSettings(p: PadPrefs, notes: PadNote[]): PadSettings {
  return { notes, emotion: p.emotion, x: p.x, y: p.y, level_db: p.levelDb, lfo_hz: p.lfoHz, seed: p.seed, intonation: p.intonation };
}

/**
 * Runs the drone and pad layers from store state, independent of which screen is
 * open: `layers.drone/pad` start and stop them, preference changes are pushed through
 * `set_*` (no restart), and the status (meters, applied intonation) is polled while on.
 * Mounted once in `App`.
 */
export function useBackgroundLayers() {
  const droneOn = useStore((s) => s.layers.drone);
  const padOn = useStore((s) => s.layers.pad);
  const output = useStore((s) => s.prefs.selectedOutput);
  const dronePrefs = useStore((s) => s.prefs.drone ?? DEFAULT_DRONE);
  const padPrefs = useStore((s) => s.prefs.pad ?? DEFAULT_PAD);
  const project = useStore((s) => s.project);
  const scale = project?.scale ?? null;
  const profile = project?.instrument ?? null;
  const profileKey = profile ? `${profile.id}:${profile.version}` : "";
  const a4 = project?.a4_hz ?? null;
  const padChord = useStore((s) => s.padChord);
  const droneOverride = useStore((s) => s.droneRootOverride);
  // An armed harmony proposal (F3.4) overrides the root and the chord while it follows.
  const dRoot = droneOverride ?? droneRoot(dronePrefs, scale, profile);
  const pRoot = padRoot(padPrefs, dronePrefs, scale, profile);
  const padNotes = padChord ?? (pRoot === null ? null : voicingNotes(padPrefs.voicing, pRoot, scale));
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  // Closing the project stops everything (the backend does too).
  useEffect(() => {
    if (!project) {
      const st = useStore.getState();
      if (st.layers.drone) st.setLayer("drone", false);
      if (st.layers.pad) useStore.getState().setLayer("pad", false);
    }
  }, [project]);

  // Drone lifecycle: start/stop on the switch or an output-device change.
  useEffect(() => {
    if (!droneOn || dRoot === null || !project) return;
    let cancelled = false;
    api
      .startDrone(output, droneSettings(dronePrefs, dRoot))
      .then((st) => !cancelled && useStore.getState().setDroneStatus(st))
      .catch((e) => {
        fail(e);
        useStore.getState().setLayer("drone", false);
      });
    return () => {
      cancelled = true;
      useStore.getState().setDroneStatus(null);
      void api.stopDrone().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [droneOn, output, !!project]);

  // Drone parameters: pushed live.
  useEffect(() => {
    if (!droneOn || dRoot === null || !useStore.getState().droneStatus) return;
    api.setDrone(droneSettings(dronePrefs, dRoot)).then((st) => useStore.getState().setDroneStatus(st)).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [droneOn, dronePrefs, dRoot, profileKey, a4]);

  // Pad lifecycle.
  useEffect(() => {
    if (!padOn || !padNotes || !project) return;
    let cancelled = false;
    api
      .startPad(output, padSettings(padPrefs, padNotes))
      .then((st) => !cancelled && useStore.getState().setPadStatus(st))
      .catch((e) => {
        fail(e);
        useStore.getState().setLayer("pad", false);
      });
    return () => {
      cancelled = true;
      useStore.getState().setPadStatus(null);
      void api.stopPad().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [padOn, output, !!project]);

  const padNotesKey = padNotes ? padNotes.map((n) => `${n.midi}:${n.cents.toFixed(1)}`).join(",") : "";
  useEffect(() => {
    if (!padOn || !padNotes || !useStore.getState().padStatus) return;
    api.setPad(padSettings(padPrefs, padNotes)).then((st) => useStore.getState().setPadStatus(st)).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [padOn, padPrefs, padNotesKey, profileKey, a4]);

  // Master level of the background, whenever it changes and once per start.
  const masterDb = useStore((s) => s.prefs.masterDb ?? 0);
  const droneStarted = useStore((s) => s.droneStatus !== null);
  const padStarted = useStore((s) => s.padStatus !== null);
  useEffect(() => {
    if (!droneStarted && !padStarted) return;
    api.setGraphMaster(masterDb).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [masterDb, droneStarted, padStarted]);

  // Meters while anything plays.
  useEffect(() => {
    if (!droneOn && !padOn) return;
    const id = window.setInterval(() => {
      const st = useStore.getState();
      if (st.layers.drone && st.droneStatus) {
        api
          .droneStatus()
          .then((d) => d && useStore.getState().setDroneStatus(d))
          .catch(() => undefined);
      }
      if (st.layers.pad && st.padStatus) {
        api
          .padStatus()
          .then((p) => p && useStore.getState().setPadStatus(p))
          .catch(() => undefined);
      }
    }, 200);
    return () => window.clearInterval(id);
  }, [droneOn, padOn]);
}
