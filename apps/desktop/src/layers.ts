import { useEffect } from "react";
import { api, describeError, type DroneSettings, type InstrumentProfile, type PadNote, type PadSettings, type Scale } from "./api";
import { DEFAULT_DRONE, DEFAULT_PAD, DEFAULT_RHYTHM, useStore, type DronePrefs, type PadPrefs, type Voicing } from "./store";

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
/** Fade the whole background (rack, drone, pad) out over `seconds` and switch it off. */
export async function fadeOutBackground(seconds: number): Promise<void> {
  const st = useStore.getState();
  if (!st.layers.drone && !st.layers.pad && !st.layers.ambient && !st.layers.rhythm) return;
  st.setLastLayers({ ...st.layers });
  try {
    await api.backgroundFadeOut(seconds);
  } catch (e) {
    st.setError(describeError(e));
    return;
  }
  window.setTimeout(() => {
    const s = useStore.getState();
    s.setLayer("drone", false);
    s.setLayer("pad", false);
    s.setLayer("ambient", false);
    s.setLayer("rhythm", false);
  }, seconds * 1000 + 150);
}

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

  const ambientOn = useStore((s) => s.layers.ambient);
  const rhythmOn = useStore((s) => s.layers.rhythm);
  const rhythmPrefs = useStore((s) => s.prefs.rhythm ?? DEFAULT_RHYTHM);
  const timing = project?.timing ?? null;
  const timingKey = timing ? `${timing.bpm}:${timing.numerator}:${timing.unit}:${timing.grouping.join(",")}` : "";
  const rhythmSettings = () => ({ ...rhythmPrefs, chord: padChord, root_midi: dRoot, intonation: dronePrefs.intonation });

  // Closing the project stops everything (the backend does too).
  useEffect(() => {
    if (!project) {
      const st = useStore.getState();
      if (st.layers.drone) st.setLayer("drone", false);
      if (st.layers.pad) useStore.getState().setLayer("pad", false);
      if (st.layers.ambient) useStore.getState().setLayer("ambient", false);
      if (st.layers.rhythm) useStore.getState().setLayer("rhythm", false);
      if (st.metronomeOn) useStore.getState().setMetronomeOn(false);
      useStore.getState().setAmbient(null);
      useStore.getState().setSleepAt(null);
    }
  }, [project]);

  // The rack is loaded with the project so "Play background" knows whether it has layers.
  const projectRoot = project?.root ?? null;
  useEffect(() => {
    if (!projectRoot) return;
    api
      .ambientGet()
      .then((v) => useStore.getState().setAmbient(v.ambient))
      .catch(() => undefined);
  }, [projectRoot]);

  // Ambient rack lifecycle (F1A.1): start/stop on its switch or an output change.
  useEffect(() => {
    if (!ambientOn || !project) return;
    let cancelled = false;
    api
      .ambientPlay(output)
      .then((st) => !cancelled && useStore.getState().setAmbientStatus(st))
      .catch((e) => {
        fail(e);
        useStore.getState().setLayer("ambient", false);
      });
    return () => {
      cancelled = true;
      useStore.getState().setAmbientStatus(null);
      void api.ambientStop().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ambientOn, output, !!project]);

  // Rhythm section lifecycle (Phase 4): start/stop on its switch or an output change.
  useEffect(() => {
    if (!rhythmOn || !project) return;
    let cancelled = false;
    api
      .startRhythm(output, rhythmSettings())
      .then((st) => !cancelled && useStore.getState().setRhythmStatus(st))
      .catch((e) => {
        fail(e);
        useStore.getState().setLayer("rhythm", false);
      });
    return () => {
      cancelled = true;
      useStore.getState().setRhythmStatus(null);
      void api.stopRhythm().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rhythmOn, output, !!project]);

  // Rhythm parameters, the followed chord and the project's mode: pushed live.
  const scaleKey = scale ? `${scale.tonic_pc}:${scale.degrees_cents.join(",")}` : "";
  useEffect(() => {
    if (!rhythmOn || !useStore.getState().rhythmStatus) return;
    // A slider sends many values per second; the engine swaps patterns in place,
    // but 120 ms of quiet keeps it to the values the user actually settles on.
    const id = window.setTimeout(() => {
      api.setRhythm(rhythmSettings()).then((st) => useStore.getState().setRhythmStatus(st)).catch(fail);
    }, 120);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rhythmOn, rhythmPrefs, padChord, dRoot, scaleKey, profileKey, a4, timingKey, dronePrefs.intonation]);

  // Standalone metronome: follows its switch, the output device and the timing.
  const metronomeOn = useStore((s) => s.metronomeOn);
  const mode = useStore((s) => s.mode);
  useEffect(() => {
    if (!metronomeOn || !project) return;
    if (mode === "recording") {
      // The recording brings its own click (count-in, click while recording).
      useStore.getState().setMetronomeOn(false);
      return;
    }
    let cancelled = false;
    api.startMetronome(output).catch((e) => {
      if (!cancelled) {
        fail(e);
        useStore.getState().setMetronomeOn(false);
      }
    });
    return () => {
      cancelled = true;
      void api.stopMetronome().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [metronomeOn, output, timingKey, mode === "recording", !!project]);

  // Sleep timer (F1A.2): when it is due, fade everything out.
  const sleepAt = useStore((s) => s.sleepAt);
  useEffect(() => {
    if (sleepAt === null) return;
    const id = window.setInterval(() => {
      if (Date.now() >= sleepAt) {
        useStore.getState().setSleepAt(null);
        void fadeOutBackground(20);
      }
    }, 1000);
    return () => window.clearInterval(id);
  }, [sleepAt]);

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
  const ambientStarted = useStore((s) => s.ambientStatus !== null);
  const rhythmStarted = useStore((s) => s.rhythmStatus !== null);
  useEffect(() => {
    if (!droneStarted && !padStarted && !ambientStarted && !rhythmStarted) return;
    // The master slider's floor (−40) is silence.
    api.setGraphMaster(masterDb <= -39.5 ? -120 : masterDb).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [masterDb, droneStarted, padStarted, ambientStarted, rhythmStarted]);

  // Meters while anything plays.
  useEffect(() => {
    if (!droneOn && !padOn && !rhythmOn) return;
    const id = window.setInterval(() => {
      const st = useStore.getState();
      if (st.layers.rhythm && st.rhythmStatus) {
        api
          .rhythmStatus()
          .then((r) => useStore.getState().setRhythmStatus(r))
          .catch(() => undefined);
      }
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
  }, [droneOn, padOn, rhythmOn]);
}
