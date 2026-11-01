import { useEffect, useRef } from "react";
import { api, describeError, type Ambient as AmbientRack, type AmbientLayer, type EditOp, type LayerKind, type LayerSource, type LibraryKind, type Scale, type Timing, type Level, type TrainerLogEntry, type FingeringOverride, type Locks, type WavFormat } from "./api";
import { startLive, stopLive } from "./LiveFollowPanel";
import { makeSong } from "./makeSong";
import type { StarterId } from "./starters";
import { defaultSource, makeLayer, MAX_AMBIENT_LAYERS } from "./ambientPresets";
import { applyLibraryPayload, applySession, collectSession, mergeSessionPrefs, pickSessionPrefs, rackForProject, SESSION_PREF_KEYS } from "./session";
import { AREA_IDS, AREAS, areaMainScreen, areaOf, areaTabs } from "./areas";
import { messages } from "./i18n";
import { DEFAULT_RHYTHM, useStore, type LayerName, type Screen } from "./store";
import { latestTake, loadTake, playLoaded, stopPlaying, continueLast } from "./takeActions";
import { heldNotes, keyboardAllOff, keyboardNote } from "./keyboard";
import { applyTemplate, isStarterId } from "./templates";
import { whatNext } from "./whatNext";
import { journeyWorld, phaseAt, WORLD_IDS, worldById } from "./scapes";
import { startJourney, stopJourney } from "./scapesActions";
import { RACKS, type RackKind } from "./rackAdapter";
import { chordNotes, DEFAULT_PROGRESSION, generateProgression, GENRES, genreSpec, stepAtBar, stepLabel, totalBars, type Genre, type ProgStep, type Progression } from "./progression";
import { jumpHistory, recordChange, redoHistory, undoHistory, useHistoryStore } from "./rackHistory";
import { readNowPlaying, stopAll } from "./nowPlaying";
import { hzToMidi as tunerHzToMidi } from "./format";
import { clampA4, readTuner, targetsFor, TUNER_INSTRUMENTS, type TunerInstrument } from "./tunerTargets";
import { droneRoot, droneSettings, padRoot, padSettings, voicingNotes } from "./layers";
import type { SongMap } from "./api";

/**
 * Assistant bridge: commands dropped into `<config>/assistant/inbox/*.json` by a
 * coding assistant in the terminal are applied here, replies go to `outbox/`, and a
 * state snapshot is kept in `state.json`. The shell only moves the files
 * (`assistant_cmds.rs`); the vocabulary lives in `runAssistantCommand`.
 * Documented in docs/ASSISTANT_BRIDGE.md — keep the two in step.
 */
export const BRIDGE_SCHEMA = 1;
const POLL_MS = 500;
const STATE_MS = 1500;
const LAYER_NAMES: LayerName[] = ["drone", "pad", "ambient", "rhythm", "scapes"];

export type AssistantCmd = Record<string, unknown> & { op?: string };

/** What the assistant can read: everything musical, nothing heavy (no audio). */
export function snapshot(): Record<string, unknown> {
  const s = useStore.getState();
  const p = s.project;
  return {
    schema: BRIDGE_SCHEMA,
    at: Date.now(),
    lang: s.prefs.lang,
    screen: s.screen,
    project: p
      ? { name: p.name, root: p.root, scale: p.scale, a4_hz: p.a4_hz, timing: p.timing, markers: p.markers, instrument: p.instrument ?? null, takes: p.takes }
      : null,
    loaded_take: s.loaded?.id ?? null,
    error: s.error ?? null,
    message: s.message ?? null,
    prefs: pickSessionPrefs(s.prefs),
    layers: s.layers,
    metronome: s.metronomeOn,
    ambient: s.ambient,
    scapes: s.scapes,
    scapes_status: s.scapesStatus ? { playing: s.scapesStatus.playing, program_position: s.scapesStatus.program_position, program_playing: s.scapesStatus.program_playing } : null,
    rhythm_status: s.rhythmStatus,
    ambient_status: s.ambientStatus ? { playing: s.ambientStatus.playing, master_peak: s.ambientStatus.master_peak, limiter_gain: s.ambientStatus.limiter_gain } : null,
  };
}

function currentPayload(kind: LibraryKind): unknown {
  const s = useStore.getState();
  switch (kind) {
    case "session":
      return collectSession();
    case "ambient":
      return s.ambient;
    case "drone":
    case "pad":
    case "rhythm":
    case "harmony":
    case "intent":
      return s.prefs[kind];
    default:
      return null;
  }
}

export async function runAssistantCommand(cmd: AssistantCmd): Promise<unknown> {
  const s = useStore.getState();
  const project = () => {
    if (!s.project) throw new Error("no project open");
    return s.project;
  };
  const t = messages[s.prefs.lang];
  const rack = () => {
    project();
    if (!s.ambient) throw new Error("no rack loaded yet");
    return s.ambient;
  };
  const setRack = async (next: AmbientRack) => {
    const v = await api.setAmbient(next);
    useStore.getState().setAmbient(v.ambient);
    return v.ambient;
  };
  switch (cmd.op) {
    case "ping":
      return { pong: true, schema: BRIDGE_SCHEMA };
    case "state":
      return snapshot();
    case "message":
      s.setMessage(String(cmd.text ?? ""));
      return {};
    case "screen":
      s.setScreen(String(cmd.screen) as Screen);
      return { screen: useStore.getState().screen };
    case "area": {
      // Home's five areas (UX audit stage 3): open an area's main screen, or list them.
      if (cmd.area === undefined) return { areas: AREA_IDS, tabs: Object.fromEntries(AREAS.map((a) => [a.id, areaTabs(a)])), current: s.screen === "home" ? null : (areaOf(s.screen)?.id ?? null) };
      const main = areaMainScreen(String(cmd.area));
      if (!main) throw new Error(`unknown area ${String(cmd.area)}; one of ${AREA_IDS.join(", ")}`);
      s.setScreen(main);
      return { area: cmd.area, screen: useStore.getState().screen };
    }
    case "sample_folders": {
      // Library › My samples: the scanned folders (prefs.sampleFolders). `add` / `remove`
      // arrays of absolute paths; without them: just the list. The screen rescans on open.
      const cur = useStore.getState().prefs.sampleFolders ?? [];
      const add = Array.isArray(cmd.add) ? cmd.add.map(String) : [];
      const remove = new Set(Array.isArray(cmd.remove) ? cmd.remove.map(String) : []);
      const next = [...cur.filter((f) => !remove.has(f)), ...add.filter((f) => !cur.includes(f))];
      if (add.length || remove.size) s.setPref("sampleFolders", next);
      return { folders: useStore.getState().prefs.sampleFolders ?? [] };
    }
    case "scapes": {
      // Soundscapes 2 (stage S1): the worlds and the journey in the rack now.
      const st = useStore.getState();
      const world = journeyWorld(st.scapes);
      const ph = world ? phaseAt(st.scapes, st.scapesStatus?.program_position ?? 0) : null;
      return { worlds: WORLD_IDS, current: world, phase: ph, on: st.layers.scapes, playing: !!st.scapesStatus?.program_playing && st.layers.scapes, position_s: st.scapesStatus?.program_position ?? 0, layers: st.scapes?.layers.length ?? 0 };
    }
    case "scapes_build": {
      // Build a world's journey into the rack (replaces it; `previous` comes back for a
      // restore). `play: true` starts the atmosphere and the program — sound!
      const w = worldById(String(cmd.world));
      if (!w) throw new Error(`unknown world ${String(cmd.world)}; one of ${WORLD_IDS.join(", ")}`);
      project();
      const previous = useStore.getState().scapes;
      const built = await startJourney(w, { minutes: typeof cmd.minutes === "number" ? cmd.minutes : 8, seed: typeof cmd.seed === "number" ? cmd.seed : 1 + Math.floor(Math.random() * 1_000_000), loop: cmd.loop === true, play: cmd.play === true });
      const end = built.program.scenes.reduce((m, sc) => Math.max(m, sc.start_s + sc.duration_s), 0);
      return { world: w.id, layers: built.layers.length, scenes: built.program.scenes.map((sc) => ({ name: sc.name, start_s: sc.start_s, duration_s: sc.duration_s, transition_s: sc.transition_s })), lanes: built.automation.length, end_s: end, loop: built.program.loop_region !== null, playing: cmd.play === true, previous };
    }
    case "scapes_stop": {
      await stopJourney();
      return { playing: false };
    }
    case "set_scapes": {
      // Replace the Soundscapes rack (same shape as set_ambient; used to restore after scapes_build).
      const p = project();
      const next = rackForProject(cmd.ambient as AmbientRack, new Set(p.takes.map((t) => t.id)));
      const prevRack = s.scapes;
      const v = await api.setScapes(next);
      recordChange("scapes", prevRack, v.ambient, t.hAssistant, t.hStart);
      useStore.getState().setScapes(v.ambient);
      return { layers: v.ambient.layers.length, scenes: v.ambient.program.scenes.length };
    }
    case "progression": {
      // The chord progression (session pref) and the chord of the current bar.
      const p = s.prefs.progression ?? DEFAULT_PROGRESSION;
      const scale = s.project?.scale ?? null;
      const bar = (s.transport?.playing ? s.transport.bar : s.layers.rhythm ? s.rhythmStatus?.bar : null) ?? null;
      const now = bar !== null ? stepAtBar(p, Math.max(0, bar - 1)) : null;
      return { genres: GENRES, progression: p, total_bars: totalBars(p), labels: p.steps.map((st) => stepLabel(scale, st)), now: now && scale ? { index: now.index, label: stepLabel(scale, now.step), notes: chordNotes(scale, now.step).map((n) => n.midi) } : null };
    }
    case "progression_set": {
      // Generate (`generate: true` with `genre`, `seed`, `bars`, `bars_per_chord`) or patch
      // (`steps`, `follow`); `drums: true` also sets the rhythm's drum style to the genre's.
      // Follow starts no sound by itself: it only acts while the song or the rhythm runs.
      const cur = s.prefs.progression ?? DEFAULT_PROGRESSION;
      let next: Progression = cur;
      if (cmd.generate === true) {
        const genre = (typeof cmd.genre === "string" && genreSpec(cmd.genre) ? cmd.genre : cur.genre) as Genre;
        next = { ...generateProgression(genre, typeof cmd.seed === "number" ? cmd.seed : cur.seed, typeof cmd.bars === "number" ? cmd.bars : cur.bars || 8, typeof cmd.bars_per_chord === "number" ? cmd.bars_per_chord : undefined), follow: cur.follow };
        if (cmd.drums === true) {
          const rp = s.prefs.rhythm ?? DEFAULT_RHYTHM;
          s.setPref("rhythm", { ...rp, drums: { ...rp.drums, style: genreSpec(genre)!.drumStyle } });
        }
      }
      if (Array.isArray(cmd.steps)) next = { ...next, edited: true, steps: (cmd.steps as ProgStep[]).map((st) => ({ degree: Math.max(1, Math.min(7, Number(st.degree) || 1)), quality: st.quality ?? "triad", bars: Math.max(1, Number(st.bars) || 1) })) };
      if (typeof cmd.follow === "boolean") next = { ...next, follow: cmd.follow };
      s.setPref("progression", next);
      const scale = s.project?.scale ?? null;
      return { progression: next, total_bars: totalBars(next), labels: next.steps.map((st) => stepLabel(scale, st)) };
    }
    case "chord_pitches": {
      // The chord the racks follow (MIDI list or null). Normally the progression / live follower sets it.
      const midi = Array.isArray(cmd.midi) ? (cmd.midi as number[]).map((m) => Math.max(0, Math.min(127, Math.round(Number(m))))) : null;
      await api.setChordPitches(midi);
      return { midi };
    }
    case "rack_pitches": {
      // What every layer of a rack would play now (Hz per layer; empty for non-tonal), also while stopped.
      const kind = (cmd.rack === "scapes" ? "scapes" : "ambient") as RackKind;
      const st = await RACKS[kind].status();
      const rk = RACKS[kind].read(useStore.getState());
      return { rack: kind, follow_chord: rk?.follow_chord ?? false, layers: (rk?.layers ?? []).map((l, i) => ({ name: l.name, kind: l.source.kind, hz: st.pitches_hz[i] ?? [] })) };
    }
    case "rack_history": {
      // The steps of a rack's edit history (`rack`: ambient | scapes); `undo` / `redo` (count)
      // or `jump` (step index) apply that state without recording a new step.
      const kind = (cmd.rack === "scapes" ? "scapes" : "ambient") as RackKind;
      const R = RACKS[kind];
      let h = useHistoryStore.getState().byRack[kind];
      let target: { history: typeof h; rack: AmbientRack } | null = null;
      for (let i = 0; i < (typeof cmd.undo === "number" ? cmd.undo : 0); i++) {
        const r = undoHistory(target?.history ?? h);
        if (!r) break;
        target = r;
      }
      for (let i = 0; i < (typeof cmd.redo === "number" ? cmd.redo : 0); i++) {
        const r = redoHistory(target?.history ?? h);
        if (!r) break;
        target = r;
      }
      if (typeof cmd.jump === "number") target = jumpHistory(target?.history ?? h, cmd.jump);
      if (target) {
        const v = await R.set(target.rack);
        R.store(v.ambient);
        useHistoryStore.getState().set(kind, target.history);
        h = target.history;
      }
      return { rack: kind, index: h.index, count: h.entries.length, labels: h.entries.map((e) => e.label), layers: R.read(useStore.getState())?.layers.length ?? 0 };
    }
    case "scapes_seek": {
      // Jump the journey to `position_s` and/or run / pause its program (`playing`).
      const st = await api.scapesProgramSet(typeof cmd.playing === "boolean" ? cmd.playing : null, typeof cmd.position_s === "number" ? cmd.position_s : null);
      useStore.getState().setScapesStatus(st);
      return { position_s: st.program_position, playing: st.program_playing, end_s: st.program_end };
    }
    case "continue": {
      // Home's "Continue": Play & record with the latest take loaded (none: ready to record).
      project();
      const id = await continueLast();
      const st = useStore.getState();
      return { screen: st.screen, loaded_id: st.loaded?.id ?? null, latest_id: id };
    }
    case "set_prefs": {
      const merged = mergeSessionPrefs(s.prefs, cmd.prefs);
      for (const k of SESSION_PREF_KEYS) if (merged[k] !== s.prefs[k]) s.setPref(k, merged[k]);
      return { prefs: pickSessionPrefs(useStore.getState().prefs) };
    }
    case "layers": {
      for (const n of LAYER_NAMES) if (typeof cmd[n] === "boolean" && s.layers[n] !== cmd[n]) s.setLayer(n, cmd[n] as boolean);
      if (typeof cmd.metronome === "boolean") s.setMetronomeOn(cmd.metronome);
      return { layers: useStore.getState().layers, metronome: useStore.getState().metronomeOn };
    }
    case "set_ambient": {
      const p = project();
      const prevRack = s.ambient;
      const next = await setRack(rackForProject(cmd.ambient as AmbientRack, new Set(p.takes.map((t) => t.id))));
      recordChange("ambient", prevRack, next, t.hAssistant, t.hStart);
      return { layers: next.layers.map((l) => ({ id: l.id, name: l.name, kind: l.source.kind })) };
    }
    case "set_macros": {
      const r = rack();
      const next = await setRack({ ...r, macros: { ...r.macros, ...((cmd.macros as object) ?? {}) } });
      return { macros: next.macros };
    }
    case "add_layer": {
      const p = project();
      const r = rack();
      if (r.layers.length >= MAX_AMBIENT_LAYERS) throw new Error(`rack full (${MAX_AMBIENT_LAYERS} layers)`);
      const kind = String(cmd.kind) as LayerKind;
      const takeId = String(cmd.take_id ?? "");
      if ((kind === "loop" || kind === "cloud") && !p.takes.some((t) => t.id === takeId)) throw new Error(`${kind} needs a take_id of this project`);
      const source = { ...defaultSource(kind, takeId), ...((cmd.source as object) ?? {}) } as LayerSource;
      const layer = makeLayer(String(cmd.name ?? kind), source, (cmd.patch ?? {}) as Partial<AmbientLayer>);
      await setRack({ ...r, layers: [...r.layers, layer] });
      return { id: layer.id, name: layer.name };
    }
    case "patch_layer": {
      const r = rack();
      const target = r.layers.find((l) => l.id === cmd.id || l.name === cmd.name);
      if (!target) throw new Error("layer not found (give id or name)");
      const patch = (cmd.patch ?? {}) as Partial<AmbientLayer> & { source?: Record<string, unknown> };
      const next = { ...target, ...patch, source: patch.source ? ({ ...target.source, ...patch.source } as LayerSource) : target.source } as AmbientLayer;
      await setRack({ ...r, layers: r.layers.map((l) => (l.id === target.id ? next : l)) });
      return { layer: next };
    }
    case "remove_layer": {
      const r = rack();
      const target = r.layers.find((l) => l.id === cmd.id || l.name === cmd.name);
      if (!target) throw new Error("layer not found (give id or name)");
      await setRack({ ...r, layers: r.layers.filter((l) => l.id !== target.id), automation: r.automation.filter((a) => a.target.layer !== target.id) });
      return { removed: target.id };
    }
    case "save_preset": {
      const kind = String(cmd.kind) as LibraryKind;
      const payload = cmd.payload === undefined || cmd.payload === "current" ? currentPayload(kind) : cmd.payload;
      if (payload === null || payload === undefined) throw new Error(`nothing to save for kind ${kind}`);
      const e = await api.librarySave({ kind, name: String(cmd.name ?? "assistant"), tags: Array.isArray(cmd.tags) ? (cmd.tags as string[]) : [], notes: String(cmd.notes ?? ""), payload });
      return { id: e.id, name: e.name };
    }
    case "list_presets":
      return { entries: await api.libraryList((cmd.kind as LibraryKind) ?? null, (cmd.project as string) ?? null, (cmd.query as string) ?? null, "date_desc") };
    case "load_preset": {
      const full = await api.libraryGet(String(cmd.id));
      const screen = await applyLibraryPayload(full.kind as LibraryKind, full.payload);
      if (cmd.go !== false) useStore.getState().setScreen(screen);
      return { screen, name: full.name };
    }
    case "session":
      await applySession(cmd.payload, cmd.restore_layers !== false);
      return {};
    case "rhythm_render": {
      project();
      const r = s.prefs.rhythm;
      const res = await api.rhythmRender(String(cmd.path), { ...r, chord: s.padChord, root_midi: s.prefs.drone?.rootMidi ?? null, intonation: s.prefs.drone?.intonation ?? "profile" }, typeof cmd.bars === "number" ? cmd.bars : null, cmd.stems !== false);
      return res;
    }
    case "error_clear":
      s.setError(null);
      return { cleared: true };
    case "create_project": {
      if (s.project) {
        await api.closeProject();
        s.setLoaded(null, null);
        s.setProject(null, null);
        s.setMessage(null);
      }
      const pv = await api.createProject(String(cmd.name ?? "smoke"), null);
      useStore.getState().setProject(pv, null);
      // `style`: one of the nine starters as a template (item 8).
      const style = isStarterId(cmd.style) ? await applyTemplate(cmd.style, false) : null;
      return { name: pv.name, root: pv.root, style };
    }
    case "instrument_list":
      return await api.instrumentList(null, null);
    case "instrument_render": {
      const preset = await api.instrumentGet(String(cmd.id));
      return await api.instrumentRender(String(cmd.out), preset.voice, typeof cmd.seconds === "number" ? cmd.seconds : null, typeof cmd.midi === "number" ? cmd.midi : null);
    }
    case "reference_remove":
      await api.referenceRemove(String(cmd.id));
      return { removed: cmd.id };
    case "rhythm_status": {
      // Fresh from the backend (the store copy is only refreshed where meters are shown).
      const st = await api.rhythmStatus();
      useStore.getState().setRhythmStatus(st);
      return st;
    }
    case "tuner": {
      // Tuner screen settings (persisted prefs) + the live reading, so the assistant can
      // answer "how close am I" without a screenshot.
      if (cmd.a4 !== undefined) s.setPref("tunerA4", cmd.a4 === null ? null : clampA4(Number(cmd.a4)));
      if (typeof cmd.instrument === "string" && TUNER_INSTRUMENTS.includes(cmd.instrument as TunerInstrument)) s.setPref("tunerInstrument", cmd.instrument as TunerInstrument);
      if (cmd.lock !== undefined) s.setPref("tunerLock", cmd.lock === null ? null : Math.round(Number(cmd.lock)));
      const st = useStore.getState();
      const a4 = st.prefs.tunerA4 ?? st.project?.a4_hz ?? 440;
      const hz = st.monitor?.pitch_hz ?? null;
      const confidence = st.monitor?.pitch_confidence ?? 0;
      const midiFrac = hz !== null && confidence >= 0.5 ? tunerHzToMidi(hz, a4) : null;
      const reading = midiFrac !== null ? readTuner(midiFrac, targetsFor(st.prefs.tunerInstrument, st.prefs.tunerLock)) : null;
      return {
        a4,
        a4_pref: st.prefs.tunerA4,
        instrument: st.prefs.tunerInstrument,
        lock: st.prefs.tunerLock,
        listening: st.monitor !== null,
        min_hz: st.monitor?.min_hz ?? null,
        hz,
        confidence,
        reading: reading ? { target_midi: reading.target.midi, label: reading.target.label, cents: Math.round(reading.cents * 10) / 10, direction: reading.direction } : null,
      };
    }
    case "now_playing":
      // Every source of sound as the bottom bar shows it (take, notes, background, song, metronome, live).
      return readNowPlaying();
    case "stop_all": {
      // Silence: stops every engine except a running recording.
      await stopAll();
      await new Promise((r) => setTimeout(r, 150));
      return readNowPlaying();
    }
    case "load_take": {
      // The ▶ path of the take list without sound: load the take (default: the latest).
      const pv = project();
      const id = typeof cmd.id === "string" ? cmd.id : latestTake(pv.takes)?.id;
      if (!id) throw new Error("no takes in the project");
      await loadTake(id);
      const st = useStore.getState();
      const tk = pv.takes.find((x) => x.id === id);
      return { loaded_id: st.loaded?.id ?? null, name: tk ? `${tk.number}${tk.label ? ` ${tk.label}` : ""}` : null, duration_s: st.loaded?.duration_seconds ?? null, analysed: st.analysis !== null, notes: st.analysis?.notes.length ?? 0 };
    }
    case "play_take": {
      // Sound! Load (if needed) and play a take from its start (or `from_s`); `stop_take` stops it.
      const pv = project();
      const id = typeof cmd.id === "string" ? cmd.id : latestTake(pv.takes)?.id;
      if (!id) throw new Error("no takes in the project");
      if (useStore.getState().loaded?.id !== id) await loadTake(id);
      await playLoaded(typeof cmd.from_s === "number" ? cmd.from_s : 0, false);
      const st = useStore.getState();
      return { loaded_id: st.loaded?.id ?? null, mode: st.mode, position_s: st.player?.position_seconds ?? null, duration_s: st.loaded?.duration_seconds ?? null };
    }
    case "stop_take": {
      await stopPlaying();
      const st = useStore.getState();
      return { mode: st.mode, cursor_s: st.cursor };
    }
    // ---- keyboard everywhere (K1/K2) ----
    case "what_next": {
      const st = useStore.getState();
      const n = whatNext({ hasProject: !!st.project, takes: st.project?.takes.length ?? 0, hasScale: !!st.project?.scale, loaded: !!st.loaded, layersOn: st.layers, clips: st.arrangeClips, exportedMs: st.lastSongExportMs, screen: st.screen });
      return { key: n.key, screen: n.screen, text: t[n.key] };
    }
    case "keyboard": {
      // Reads or sets the dock: on, source ("voice" | "instrument"), octave (24..96 base
      // MIDI), lock, velocity, midi_input (port name or null). Returns the dock's state.
      const st = useStore.getState();
      if (typeof cmd.on === "boolean") st.setPref("keyboardOn", cmd.on);
      if (cmd.source === "voice" || cmd.source === "instrument") st.setPref("keyboardSource", cmd.source);
      if (typeof cmd.octave === "number") st.setPref("keyboardOctave", Math.max(24, Math.min(96, Math.round(cmd.octave))));
      if (typeof cmd.lock === "boolean") st.setPref("keyboardLock", cmd.lock);
      if (typeof cmd.velocity === "number") st.setPref("keyboardVelocity", Math.max(0.05, Math.min(1, cmd.velocity)));
      if (cmd.midi_input === null || typeof cmd.midi_input === "string") st.setPref("midiInput", cmd.midi_input);
      await new Promise((r) => setTimeout(r, 250));
      // K4: `bend` (semitones) / `mod` (0..1) go straight to the target, like the wheels.
      let control: [number | null, number | null] | null = null;
      if (typeof cmd.bend === "number" || typeof cmd.mod === "number") control = await api.keyboardControl(typeof cmd.bend === "number" ? cmd.bend : null, typeof cmd.mod === "number" ? cmd.mod : null);
      const p = useStore.getState();
      const synth = await api.liveSynthStatus().catch(() => null);
      return { on: p.prefs.keyboardOn, source: p.prefs.keyboardSource, octave: p.prefs.keyboardOctave, lock: p.prefs.keyboardLock, velocity: p.prefs.keyboardVelocity, midi_input: p.prefs.midiInput, midi_ports: p.midiPorts, synth_running: synth !== null, designer_voice: p.designerVoice !== null, held: heldNotes(), bend: control?.[0] ?? null, mod: control?.[1] ?? null };
    }
    case "note": {
      // Sound! One keyboard note where the dock points: { midi, on, velocity? }; "notes_off" releases all.
      const midi = Number(cmd.midi);
      if (!(midi >= 0 && midi <= 127)) throw new Error("midi outside 0..127");
      await keyboardNote(midi, cmd.on !== false, typeof cmd.velocity === "number" ? cmd.velocity : undefined);
      return { midi, on: cmd.on !== false, held: heldNotes() };
    }
    case "notes_off": {
      await keyboardAllOff();
      return { held: [] };
    }
    // ---- audio editor (home-studio item 5 = E1) ----
    case "edit_open": {
      // Opens a take (default: the latest) as the editor's working copy; the Edit tab follows.
      const pv = project();
      const id = typeof cmd.id === "string" ? cmd.id : latestTake(pv.takes)?.id;
      if (!id) throw new Error("no takes in the project");
      const st = await api.editOpen(id);
      useStore.getState().setEditTake(id);
      return st;
    }
    case "edit_status":
      return await api.editStatus();
    case "edit":
      // One operation: `edit` = { op: cut|copy|delete|paste|silence|trim|insert_silence|gain|normalize|fade_in|fade_out|reverse, a, b, at, seconds, db, target_dbfs }.
      return await api.editApply(cmd.edit as EditOp);
    case "edit_undo":
      return await api.editUndo();
    case "edit_redo":
      return await api.editRedo();
    case "edit_jump":
      return await api.editJump(Number(cmd.step));
    case "edit_snap":
      return { seconds: await api.editSnap(Number(cmd.seconds)) };
    case "edit_peaks": {
      const pk = await api.editPeaks(typeof cmd.bins === "number" ? cmd.bins : 200, typeof cmd.from_s === "number" ? cmd.from_s : null, typeof cmd.to_s === "number" ? cmd.to_s : null);
      const peak = pk.max.reduce((m, v, i) => Math.max(m, Math.abs(v), Math.abs(pk.min[i])), 0);
      return { bins: pk.max.length, duration_seconds: pk.duration_seconds, peak };
    }
    case "edit_play": {
      // Sound! Plays the working copy from from_s (to to_s when given); stop_take stops it.
      const st = await api.editPlay(typeof cmd.from_s === "number" ? cmd.from_s : 0, typeof cmd.to_s === "number" ? cmd.to_s : null, s.prefs.selectedOutput);
      return st;
    }
    case "edit_save": {
      const r = await api.editSave(typeof cmd.label === "string" ? cmd.label : null);
      useStore.getState().setProject(r.project);
      return r;
    }
    case "edit_close": {
      await api.editClose();
      useStore.getState().setEditTake(null);
      return { closed: true };
    }
    case "import_audio": {
      project();
      const pv = await api.importAudio(String(cmd.path));
      useStore.getState().setProject(pv);
      const last = pv.takes.reduce((a, b) => (a && a.number > b.number ? a : b), pv.takes[0]);
      return { take_id: last?.id ?? null, takes: pv.takes.length };
    }
    case "arrangement":
      project();
      return await api.arrangementGet();
    case "set_arrangement": {
      project();
      return await api.setArrangement(cmd.arrangement as import("./api").Arrangement);
    }
    case "song_map":
      return project().song_map;
    case "set_song_map": {
      project();
      const pv = await api.setSongMap(cmd.song_map as SongMap);
      useStore.getState().setProject(pv);
      return pv;
    }
    case "export_song": {
      // The same request the studio's "Export mix" builds: the switched-on layers with
      // their settings, through the one offline graph, with stems.
      const p = project();
      const scale = p.scale ?? null;
      const profile = p.instrument ?? null;
      const dp = s.prefs.drone;
      const pp = s.prefs.pad;
      const dRoot = s.droneRootOverride ?? droneRoot(dp, scale, profile);
      const pRoot = padRoot(pp, dp, scale, profile);
      const padNotes = s.padChord ?? (pRoot === null ? null : voicingNotes(pp.voicing, pRoot, scale));
      const include = {
        clips: true,
        drone: dRoot !== null && (cmd.drone ?? true) ? droneSettings(dp, dRoot) : null,
        pad: padNotes && (cmd.pad ?? true) ? padSettings(pp, padNotes) : null,
        ambient: cmd.ambient === true,
        scapes: cmd.scapes === true,
        rhythm: cmd.rhythm === false ? null : { ...(s.prefs.rhythm ?? {}), chord: s.padChord, root_midi: dRoot, intonation: dp.intonation },
      };
      // `target_lufs`: a number normalises to it, null leaves the mix as rendered,
      // absent = the studio's own setting (default −14).
      const targetLufs = cmd.target_lufs === undefined ? (s.prefs.exportLoudness ?? null) : typeof cmd.target_lufs === "number" ? cmd.target_lufs : null;
      // `format`: float32 | pcm24 | pcm16 | flac24 | flac16 (default: the studio's setting).
      const format = (typeof cmd.format === "string" ? cmd.format : s.prefs.exportFormat) as WavFormat;
      return await api.exportSong(String(cmd.path), include as never, format ?? null, null, typeof cmd.seconds === "number" ? cmd.seconds : null, cmd.stems !== false, targetLufs);
    }
    case "instrument_audition": {
      // Plays a preset through the designer's audition path for a few seconds (sound!).
      const preset = await api.instrumentGet(String(cmd.id));
      const secs = typeof cmd.seconds === "number" ? Math.min(20, Math.max(0.5, cmd.seconds)) : 3;
      await api.instrumentAuditionStart(preset.voice, s.prefs.selectedOutput, null, -6);
      await new Promise((r) => window.setTimeout(r, secs * 1000));
      await api.instrumentAuditionStop();
      return { played: preset.name, seconds: secs };
    }
    case "clap_scan": {
      const folders = Array.isArray(cmd.folders) && cmd.folders.length > 0 ? (cmd.folders as string[]) : await api.clapDefaultFolders();
      return { folders, files: await api.clapScan(folders) };
    }
    case "clap_render":
      return await api.clapRender(String(cmd.path), String(cmd.plugin_id), String(cmd.out), typeof cmd.midi === "number" ? cmd.midi : null);
    case "transport_status":
      return await api.transportStatus();
    case "transport_play": {
      // Starts the song timeline (sound!) — the caller tells the user first.
      project();
      const st = await api.transportPlay(s.prefs.selectedOutput, typeof cmd.from_s === "number" ? cmd.from_s : null);
      s.setTransport(st);
      return st;
    }
    case "transport_stop": {
      const st = await api.transportStop();
      s.setTransport(st);
      return st;
    }
    case "transport_seek": {
      const st = await api.transportSeek(Number(cmd.seconds));
      s.setTransport(st);
      return st;
    }
    case "transport_loop": {
      project();
      const st = await api.transportLoop(typeof cmd.start_bar === "number" ? cmd.start_bar : null, typeof cmd.bars === "number" ? cmd.bars : null);
      s.setTransport(st);
      return st;
    }
    case "reference_phrases": {
      const r = await api.referencePhrases(String(cmd.id));
      return { ...r, phrases: r.phrases.map((p) => ({ ...p, notes: p.notes.map((n) => ({ s: +n.start_s.toFixed(3), d: +n.dur_s.toFixed(3), m: n.midi, c: Math.round(n.cents), v: +n.velocity.toFixed(2) })) })) };
    }
    case "reference_phrase_to_take": {
      project();
      const pv = await api.referencePhraseToTake(String(cmd.id), Number(cmd.index));
      useStore.getState().setProject(pv);
      return { takes: pv.takes.length };
    }
    case "reference_add": {
      const info = await api.referenceAdd(String(cmd.path));
      return { id: info.id, name: info.name, duration_s: info.duration_s };
    }
    case "reference_list":
      return { references: await api.referenceList() };
    case "reference_report": {
      const r = await api.referenceReport(String(cmd.id));
      return r ? { ...r, energy: undefined, density: undefined, low: undefined } : { analyzed: false };
    }
    case "reference_analyze": {
      const r = await api.referenceAnalyze(String(cmd.id));
      return { ...r, energy: undefined, density: undefined, low: undefined };
    }
    case "open_project": {
      // Opens a project by its folder path (the Project menu does the same): closes the
      // current one first so nothing of it leaks into the next.
      if (s.project) {
        await api.closeProject();
        s.setLoaded(null, null);
        s.setProject(null, null);
        s.setMessage(null);
      }
      const r = await api.openProject(String(cmd.path));
      useStore.getState().setProject(r.project, r.report);
      return { name: r.project.name, root: r.project.root, takes: r.project.takes.length };
    }
    case "academy_lessons": {
      const v = await api.academyLessons();
      return { count: v.lessons.length, context: v.context, lessons: v.lessons.map((l) => ({ id: l.id, level: l.level, track: l.track, type: l.exercise.type, passed: !!l.progress.passed_at_ms })) };
    }
    case "academy_instance":
      return await api.academyInstance(String(cmd.id), typeof cmd.seed === "number" ? cmd.seed : null);
    case "academy_score_rhythm":
      return await api.academyScoreRhythm(cmd.onsets as number[], cmd.grid as number[], (cmd.level as Level) ?? "beginner");
    case "academy_record":
      return await api.academyRecord(String(cmd.id), Number(cmd.score), Boolean(cmd.passed));
    case "make_song": {
      project();
      const r = await makeSong((cmd.style as StarterId | undefined) ?? useStore.getState().prefs.makeStyle, typeof cmd.take_id === "string" ? cmd.take_id : null);
      return { ...r, facts: r.facts.map((f) => f.key + (f.detail ? `: ${f.detail}` : "")) };
    }
    case "live_start": {
      await startLive();
      return { running: useStore.getState().liveRunning };
    }
    case "live_tick":
      return await api.liveTick();
    case "live_set":
      return await api.liveSet({ locks: (cmd.locks as Locks | undefined) ?? null, bypass: typeof cmd.bypass === "boolean" ? cmd.bypass : null });
    case "live_stop":
      await stopLive();
      return { running: false };
    case "trainer_history":
      return await api.academyTrainerHistory(typeof cmd.limit === "number" ? cmd.limit : 20);
    case "trainer_log":
      return { count: await api.academyTrainerLog(cmd.entry as TrainerLogEntry) };
    case "fingering_overrides": {
      const instrument = String(cmd.instrument ?? "kaval-d");
      if (Array.isArray(cmd.overrides)) {
        const saved = await api.fingeringOverridesSet(instrument, cmd.overrides as FingeringOverride[]);
        useStore.getState().setFingeringOverrides(instrument, saved);
        return { overrides: saved };
      }
      return { overrides: await api.fingeringOverridesGet(instrument) };
    }
    case "academy_reset":
      await api.academyReset(typeof cmd.id === "string" ? cmd.id : null);
      return { ok: true };
    case "recent_projects":
      return { recent: await api.recentProjects() };
    case "set_scale": {
      project();
      const pv = await api.setScale((cmd.scale as Scale | null) ?? null);
      useStore.getState().setProject(pv);
      return { scale: pv.scale };
    }
    case "set_timing": {
      project();
      const pv = await api.setTiming(cmd.timing as Timing);
      useStore.getState().setProject(pv);
      return { timing: pv.timing };
    }
    default:
      throw new Error(`unknown op: ${String(cmd.op)}`);
  }
}

/** Poll the inbox and keep `state.json` fresh while the bridge is on (Project menu). */
export function useAssistantBridge() {
  const enabled = useStore((s) => s.prefs.assistantBridge ?? true);
  const last = useRef("");
  // Always the latest runner, so a hot-reloaded module (new ops) is picked up without a restart.
  const run = useRef(runAssistantCommand);
  run.current = runAssistantCommand;

  useEffect(() => {
    if (!enabled) return;
    let busy = false;
    const id = window.setInterval(async () => {
      if (busy) return;
      busy = true;
      try {
        const cmds = await api.assistantPoll();
        for (const c of cmds) {
          let reply: unknown;
          try {
            reply = { ok: true, result: await run.current(c.body as AssistantCmd) };
          } catch (e) {
            reply = { ok: false, error: describeError(e) };
          }
          await api.assistantReply(c.name, reply).catch(() => undefined);
        }
        if (cmds.length > 0) last.current = "";
      } catch {
        // The shell is not ready yet (or the folder is unavailable): try again next tick.
      } finally {
        busy = false;
      }
    }, POLL_MS);
    return () => window.clearInterval(id);
  }, [enabled]);

  useEffect(() => {
    if (!enabled) return;
    const id = window.setInterval(() => {
      const snap = snapshot();
      const text = JSON.stringify(snap, (k, v) => (k === "at" ? undefined : v));
      if (text === last.current) return;
      last.current = text;
      api.assistantWriteState(snap).catch(() => undefined);
    }, STATE_MS);
    return () => window.clearInterval(id);
  }, [enabled]);
}
