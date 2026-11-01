// Keyboard everywhere (KEYBOARD_EVERYWHERE_PLAN K1/K2, 2026-11-01): one place that knows
// where a pressed key goes — the project's live-synth voice, or the instrument being
// auditioned in the designer — plus the app-wide MIDI input (opened once, polled for the
// trainer's lights) and the computer-keyboard note map. The keyboard never changes the
// project: it only sounds.
import { useEffect, useRef } from "react";
import { api, describeError, type PadNote, type Scale } from "./api";
import { voicingNotes } from "./layers";
import { centsFromScale } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

/** "module:<rack>:<layer>" = one strike of that event layer (K3). */
export type KeyboardSource = "voice" | "instrument" | "pad" | "drone" | `module:${"ambient" | "scapes"}:${number}`;

/** The event layers a key can strike: every bells / pluck / resonator / sampler layer of both racks. */
export function moduleSources(): { id: KeyboardSource; rack: "ambient" | "scapes"; layer: number; name: string; kind: string }[] {
  const st = useStore.getState();
  const out: { id: KeyboardSource; rack: "ambient" | "scapes"; layer: number; name: string; kind: string }[] = [];
  for (const rack of ["ambient", "scapes"] as const) {
    const a = rack === "ambient" ? st.ambient : st.scapes;
    a?.layers.forEach((l, i) => {
      if (["bells", "pluck", "resonator", "sampler"].includes(l.source.kind)) out.push({ id: `module:${rack}:${i}`, rack, layer: i, name: l.name, kind: l.source.kind });
    });
  }
  return out;
}

/** What the pad / drone showed before the keyboard took them over (restored on release). */
let before: { padChord: PadNote[] | null; droneRoot: number | null; padOn: boolean; droneOn: boolean } | null = null;
let moduleWarned = 0;

function restoreFollow(): void {
  if (!before) return;
  const st = useStore.getState();
  st.setFollow(before.padChord, before.droneRoot);
  if (!before.padOn && st.layers.pad) st.setLayer("pad", false);
  if (!before.droneOn && st.layers.drone) st.setLayer("drone", false);
  before = null;
}

/** Two rows of the computer keyboard = two octaves (lower row from C). */
export const LOWER: Record<string, number> = { z: 0, s: 1, x: 2, d: 3, c: 4, v: 5, g: 6, b: 7, h: 8, n: 9, j: 10, m: 11, ",": 12, l: 13, ".": 14 };
export const UPPER: Record<string, number> = { q: 12, "2": 13, w: 14, "3": 15, e: 16, r: 17, "5": 18, t: 19, "6": 20, y: 21, "7": 22, u: 23, i: 24, "9": 25, o: 26, "0": 27, p: 28 };
export const BLACK = new Set([1, 3, 6, 8, 10]);
export const OCTAVE_MIN = 24;
export const OCTAVE_MAX = 96;
const MIDI_POLL_MS = 50;

/** Key code (semitones above the base) for a computer key, or null. */
export function codeForKey(key: string): number | null {
  const k = key.toLowerCase();
  return LOWER[k] ?? UPPER[k] ?? null;
}

/** The MIDI note a key code plays: base + code, snapped to the scale when locked. */
export function midiForCode(base: number, code: number, scale: Scale | null, lock: boolean): number {
  let midi = base + code;
  if (lock && scale) {
    const off = centsFromScale(scale, midi);
    midi = Math.round(midi - off / 100);
  }
  return Math.max(0, Math.min(127, midi));
}

const held = new Set<number>();
let instrumentWarned = 0;

/** Send one note where the keyboard points. Sound. */
export async function keyboardNote(midi: number, on: boolean, velocity?: number): Promise<void> {
  const st = useStore.getState();
  const vel = velocity ?? st.prefs.keyboardVelocity;
  if (on) held.add(midi);
  else held.delete(midi);
  if (st.prefs.keyboardSource === "instrument") {
    const ok = await api.instrumentNote(midi, on, vel);
    if (!ok && on) {
      // No audition running: start one silently with the designer's voice, then retry.
      const voice = st.designerVoice;
      if (voice) {
        await api.instrumentAuditionStart(voice, st.prefs.selectedOutput, null, -6, true);
        await api.setKeyboardTarget("instrument");
        await api.instrumentNote(midi, on, vel);
      } else if (Date.now() - instrumentWarned > 3000) {
        instrumentWarned = Date.now();
        st.setMessage(messages[st.prefs.lang].kbNoInstrument);
      }
    }
    return;
  }
  const src = st.prefs.keyboardSource;
  if (src === "pad" || src === "drone") {
    // K3: the key becomes the pad's root (its voicing) or the drone's root while held;
    // the previous chord / root and the layer switches come back on release.
    if (on) {
      if (!before) before = { padChord: st.padChord, droneRoot: st.droneRootOverride, padOn: st.layers.pad, droneOn: st.layers.drone };
      if (src === "pad") {
        // A scale voicing needs 7 degrees; on a shorter scale the key still gets a chord (a fifth).
        const chord = voicingNotes(st.prefs.pad.voicing, midi, st.project?.scale ?? null) ?? voicingNotes("fifth", midi, null);
        st.setFollow(chord, st.droneRootOverride);
        if (!st.layers.pad) st.setLayer("pad", true);
      } else {
        st.setFollow(st.padChord, midi);
        if (!st.layers.drone) st.setLayer("drone", true);
      }
    } else if (held.size === 0) {
      restoreFollow();
    }
    return;
  }
  if (src.startsWith("module:")) {
    const [, rack, idx] = src.split(":");
    const ok = await api.rackNote(rack as "ambient" | "scapes", Number(idx), midi, on, vel);
    if (!ok && on && Date.now() - moduleWarned > 3000) {
      moduleWarned = Date.now();
      st.setMessage(messages[st.prefs.lang].kbModuleOff);
    }
    return;
  }
  if (!st.liveSynthOn) return;
  await api.synthNote(midi, on, on ? vel : null);
}

/** Release everything the keyboard holds (both targets). */
export async function keyboardAllOff(): Promise<void> {
  held.clear();
  restoreFollow();
  await Promise.all([api.synthAllOff().catch(() => undefined), api.instrumentNotesOff().catch(() => undefined)]);
}

export function heldNotes(): number[] {
  return [...held];
}

/**
 * App-wide: the MIDI port from the prefs stays open on every screen; its events light the
 * keys and feed the trainer. The sound goes straight from the engine's forwarding thread
 * to the chosen target (no round trip through the UI).
 */
export function useMidiInput(): void {
  const port = useStore((s) => s.prefs.midiInput);
  useEffect(() => {
    void api.listMidiPorts().then((p) => useStore.getState().setMidiPorts(p.map((x) => x.name))).catch(() => undefined);
  }, []);
  useEffect(() => {
    if (!port) {
      void api.closeMidiInput().catch(() => undefined);
      useStore.getState().setMidiState([], []);
      return;
    }
    let cancelled = false;
    const down = new Set<number>();
    api.openMidiInput(port).catch((e) => {
      useStore.getState().setError(describeError(e));
      useStore.getState().setPref("midiInput", null);
    });
    const tick = async () => {
      if (cancelled) return;
      try {
        const r = await api.pollMidiEvents();
        if (r) {
          for (const ev of r.events) {
            if (ev.kind === "note_on") down.add(ev.note);
            else if (ev.kind === "note_off") down.delete(ev.note);
          }
          if (r.events.length) useStore.getState().setMidiState([...down], r.events);
        }
      } catch {
        return;
      }
      setTimeout(tick, MIDI_POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
      void api.closeMidiInput().catch(() => undefined);
    };
  }, [port]);
}

/**
 * App-wide: the keyboard's target follows the source pref (and the Instruments screen
 * picks "the edited instrument" by itself); the live synth runs while the keyboard is on
 * and points at the project's voice.
 */
export function useKeyboardRouting(): void {
  const on = useStore((s) => s.prefs.keyboardOn);
  const source = useStore((s) => s.prefs.keyboardSource);
  const screen = useStore((s) => s.screen);
  const output = useStore((s) => s.prefs.selectedOutput);
  const voice = useStore((s) => s.prefs.voice);
  const lastScreen = useRef(screen);

  // Entering Instruments: play the instrument under your fingers; leaving: the voice.
  useEffect(() => {
    const st = useStore.getState();
    if (screen === "instrument" && lastScreen.current !== "instrument") st.setPref("keyboardSource", "instrument");
    else if (screen !== "instrument" && lastScreen.current === "instrument" && st.prefs.keyboardSource === "instrument") st.setPref("keyboardSource", "voice");
    lastScreen.current = screen;
  }, [screen]);

  useEffect(() => {
    void keyboardAllOff();
    // The engine target matters for MIDI: the instrument, or the live synth (voice); the
    // pad / drone / module sources are driven from here, so MIDI goes to the synth path
    // only when the voice is chosen.
    void api.setKeyboardTarget(source === "instrument" ? "instrument" : "synth").catch(() => undefined);
  }, [source]);

  useEffect(() => {
    const st = useStore.getState();
    if (on && source === "voice") {
      api
        .startLiveSynth(output, voice)
        .then(() => useStore.getState().setLiveSynthOn(true))
        .catch((e) => {
          useStore.getState().setError(describeError(e));
          useStore.getState().setPref("keyboardOn", false);
        });
      return () => {
        void api.stopLiveSynth().catch(() => undefined);
        useStore.getState().setLiveSynthOn(false);
      };
    }
    st.setLiveSynthOn(false);
    return undefined;
  }, [on, source, output, voice]);
}
