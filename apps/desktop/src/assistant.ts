import { useEffect, useRef } from "react";
import { api, describeError, type Ambient as AmbientRack, type AmbientLayer, type LayerKind, type LayerSource, type LibraryKind, type Scale, type Timing, type Level } from "./api";
import { defaultSource, makeLayer, MAX_AMBIENT_LAYERS } from "./ambientPresets";
import { applyLibraryPayload, applySession, collectSession, mergeSessionPrefs, pickSessionPrefs, rackForProject, SESSION_PREF_KEYS } from "./session";
import { useStore, type LayerName, type Screen } from "./store";
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
const LAYER_NAMES: LayerName[] = ["drone", "pad", "ambient", "rhythm"];

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
      const next = await setRack(rackForProject(cmd.ambient as AmbientRack, new Set(p.takes.map((t) => t.id))));
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
      return { name: pv.name, root: pv.root };
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
        rhythm: cmd.rhythm === false ? null : { ...(s.prefs.rhythm ?? {}), chord: s.padChord, root_midi: dRoot, intonation: dp.intonation },
      };
      return await api.exportSong(String(cmd.path), include as never, s.prefs.exportFormat ?? null, null, typeof cmd.seconds === "number" ? cmd.seconds : null, cmd.stems !== false);
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
