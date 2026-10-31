#!/usr/bin/env node
// Smoke test of the RUNNING app through the assistant bridge (docs/ASSISTANT_BRIDGE.md).
// Drives every screen and every major feature path, checks replies, measured audio and
// the app's own error state, and writes docs/testing/SMOKE_REPORT.md with PASS/FAIL per
// step. Run while `npm run tauri dev` (or the installed app with the bridge on) is up:
//
//   node scripts/dev/smoke.mjs [--project <path>] [--sound] [--quick]
//
// Without --sound every audible step is skipped (offline renders are still measured).
// Exit code 1 when any step fails. Each step is small so a failure names the feature.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";

const argv = process.argv.slice(2);
const flag = (n) => argv.includes(n);
const opt = (n, d) => {
  const i = argv.indexOf(n);
  return i >= 0 && argv[i + 1] ? argv[i + 1] : d;
};
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")), "..", "..");
const EF = path.join(ROOT, "scripts", "assistant", "ef.mjs");
const SOUND = flag("--sound");
const QUICK = flag("--quick");
const OUT_DIR = path.join(ROOT, "qa", "renders", "smoke");
fs.mkdirSync(OUT_DIR, { recursive: true });
const PROJECT = opt("--project", path.join(os.homedir(), "Documents", "EthnoFlow", "_smoke.ethnoflow"));

const results = [];
let state = null;

function send(cmds) {
  const list = Array.isArray(cmds) ? cmds : [cmds];
  const out = execFileSync(process.execPath, [EF, "send", ...list.map((c) => JSON.stringify(c))], { encoding: "utf8", timeout: 180_000, env: { ...process.env, EF_TIMEOUT_MS: "150000" } });
  return JSON.parse(out);
}
function readState() {
  state = JSON.parse(execFileSync(process.execPath, [EF, "state"], { encoding: "utf8" }));
  return state;
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function step(name, fn) {
  const t0 = Date.now();
  try {
    const detail = (await fn()) ?? "";
    const st = readState();
    if (st.error) {
      results.push({ name, ok: false, detail: `app error: ${st.error} (${detail})`, ms: Date.now() - t0 });
      send({ op: "error_clear" });
    } else {
      results.push({ name, ok: true, detail: String(detail), ms: Date.now() - t0 });
    }
  } catch (e) {
    results.push({ name, ok: false, detail: String(e?.message ?? e).split("\n")[0].slice(0, 300), ms: Date.now() - t0 });
    try {
      send({ op: "error_clear" });
    } catch {
      /* ignore */
    }
  }
  const r = results[results.length - 1];
  console.log(`${r.ok ? "PASS" : "FAIL"}  ${name}  ${r.detail ? "— " + r.detail : ""}`);
}
function expectOk(reply) {
  const r = Array.isArray(reply) ? reply : [reply];
  for (const x of r) if (!x.ok) throw new Error(x.error ?? "not ok");
  return r.length === 1 ? r[0].result : r.map((x) => x.result);
}
function wavStats(p) {
  const b = fs.readFileSync(p);
  let o = 12;
  let fmt = null;
  let data = null;
  while (o + 8 <= b.length) {
    const id = b.toString("ascii", o, o + 4);
    const sz = b.readUInt32LE(o + 4);
    if (id === "fmt ") fmt = { ch: b.readUInt16LE(o + 10), sr: b.readUInt32LE(o + 12), bits: b.readUInt16LE(o + 22) };
    if (id === "data") data = b.subarray(o + 8, Math.min(b.length, o + 8 + sz));
    o += 8 + sz + (sz & 1);
  }
  if (!fmt || !data) throw new Error(`not a WAV: ${p}`);
  let peak = 0;
  let n = 0;
  const step = fmt.bits === 32 ? 4 : fmt.bits === 24 ? 3 : 2;
  for (let i = 0; i + step <= data.length; i += step) {
    let v;
    if (step === 4) v = data.readFloatLE(i);
    else if (step === 3) {
      let x = data[i] | (data[i + 1] << 8) | (data[i + 2] << 16);
      if (x & 0x800000) x -= 0x1000000;
      v = x / 8388608;
    } else v = data.readInt16LE(i) / 32768;
    const a = Math.abs(v);
    if (a > peak) peak = a;
    n++;
  }
  return { peak, seconds: n / fmt.ch / fmt.sr, ch: fmt.ch, sr: fmt.sr };
}
/** Per-channel RMS over a time window (seconds). */
function channelRms(p, fromS = 0, toS = Infinity) {
  const b = fs.readFileSync(p);
  let o = 12;
  let fmt = null;
  let data = null;
  while (o + 8 <= b.length) {
    const id = b.toString("ascii", o, o + 4);
    const sz = b.readUInt32LE(o + 4);
    if (id === "fmt ") fmt = { ch: b.readUInt16LE(o + 10), sr: b.readUInt32LE(o + 12), bits: b.readUInt16LE(o + 22) };
    if (id === "data") data = b.subarray(o + 8, Math.min(b.length, o + 8 + sz));
    o += 8 + sz + (sz & 1);
  }
  if (!fmt || !data || fmt.bits !== 32) throw new Error("expected a float WAV");
  const sums = new Array(fmt.ch).fill(0);
  let frames = 0;
  const total = data.length / 4 / fmt.ch;
  const a = Math.max(0, Math.floor(fromS * fmt.sr));
  const z = Math.min(total, Math.floor(toS * fmt.sr));
  for (let f = a; f < z; f++) {
    for (let c = 0; c < fmt.ch; c++) {
      const v = data.readFloatLE((f * fmt.ch + c) * 4);
      sums[c] += v * v;
    }
    frames++;
  }
  return sums.map((s) => Math.sqrt(s / Math.max(1, frames)));
}
const audible = (p, min = 0.02) => {
  const s = wavStats(p);
  if (!(s.peak > min && s.peak <= 1.0)) throw new Error(`${path.basename(p)} peak ${s.peak.toFixed(3)}`);
  return `${path.basename(p)} ${s.seconds.toFixed(1)} s peak ${s.peak.toFixed(2)}`;
};

const SCREENS = ["home", "play", "tuner", "compose", "studio", "ambient", "soundscapes", "rhythm", "library", "instrument", "songmap", "samples", "references", "academy", "create", "start", "help"];

async function main() {
  await step("bridge: ping", () => expectOk(send({ op: "ping" })).pong === true || Promise.reject(new Error("no pong")));
  // The smoke test runs against the real app: remember what it will touch (global prefs,
  // the open project, the screen) and put everything back at the end.
  const before = readState();
  const original = { prefs: { rhythm: before.prefs.rhythm, mixer: before.prefs.mixer }, project: before.project?.root ?? null, screen: before.screen ?? "home" };
  await step("project: open or create the smoke project", () => {
    const st = readState();
    if (st.project && st.project.root === PROJECT) return "already open";
    try {
      const r = expectOk(send({ op: "open_project", path: PROJECT }));
      return `opened ${r.name}`;
    } catch {
      const r = expectOk(send({ op: "create_project", name: path.basename(PROJECT, ".ethnoflow") }));
      return `created ${r.name}`;
    }
  });
  await step("project: scale and timing round trip", () => {
    const [a, b] = expectOk(send([
      { op: "set_scale", scale: { tonic_pc: 2, degrees_cents: [0, 200, 300, 500, 700, 900, 1000], kind: "dorian" } },
      { op: "set_timing", timing: { bpm: 120, numerator: 7, unit: 8, grouping: [2, 2, 3] } },
    ]));
    if (a.scale.kind !== "dorian" || b.timing.bpm !== 120 || b.timing.numerator !== 7) throw new Error(JSON.stringify([a, b]));
    return "D dorian, 120 BPM 7/8 (2+2+3)";
  });
  for (const sc of SCREENS) {
    await step(`screen: ${sc}`, async () => {
      const r = expectOk(send({ op: "screen", screen: sc }));
      await sleep(250);
      if (r.screen !== sc) throw new Error(`screen is ${r.screen}`);
    });
  }
  await step("prefs: partial nested merge keeps the other fields", () => {
    const r = expectOk(send({ op: "set_prefs", prefs: { rhythm: { ostinato: { on: true } } } }));
    const o = r.prefs.rhythm.ostinato;
    if (!o.on || typeof o.steps !== "number") throw new Error(JSON.stringify(o));
    return `ostinato steps ${o.steps}`;
  });
  await step("instruments: every factory preset renders audibly (offline)", () => {
    const list = expectOk(send({ op: "instrument_list" }));
    const factory = list.filter((i) => i.factory);
    if (factory.length < 21) throw new Error(`only ${factory.length} factory presets`);
    const quiet = [];
    for (const i of QUICK ? factory.slice(0, 6) : factory) {
      const out = path.join(OUT_DIR, `inst-${i.id.replace(/[^a-z0-9.]/gi, "_")}.wav`);
      expectOk(send({ op: "instrument_render", id: i.id, out, seconds: 2 }));
      const s = wavStats(out);
      if (!(s.peak > 0.03)) quiet.push(`${i.id} ${s.peak.toFixed(3)}`);
    }
    if (quiet.length) throw new Error(`quiet: ${quiet.join(", ")}`);
    return `${QUICK ? 6 : factory.length} presets, all audible`;
  });
  await step("rhythm: offline render with the four parts", () => {
    // A known baseline: bus trims at 0, part levels where the stems are clearly measurable.
    expectOk(send({ op: "set_prefs", prefs: { mixer: { drone: { gain_db: 0 }, pad: { gain_db: 0 }, ambient: { gain_db: 0 }, rhythm: { gain_db: 0 } }, rhythm: { drums: { on: true, level_db: -10 }, bass: { on: true, level_db: -12 }, ostinato: { on: true, level_db: -10 }, arp: { on: true, level_db: -12 }, instruments: {}, extras: [], follow_map: false } } }));
    const out = path.join(OUT_DIR, "rhythm-four.wav");
    const r = expectOk(send({ op: "rhythm_render", path: out, bars: 2 }));
    if (r.stems.length !== 4) throw new Error(`${r.stems.length} stems`);
    for (const s of r.stems) audible(s);
    return audible(out, 0.05);
  });
  await step("rhythm: Instrument Rack parts render their own stems", () => {
    const extra = (id, kind, extraFields) => ({ id, kind, on: true, level_db: -12, instrument: null, density: 0.5, humanize: 0.2, fill_every_bars: 2, gap: 0.1, bars_per_chord: 1, step_beats: 0.5, phrase_bars: 2, leaps: 0.2, register_low: 60, register_high: 84, seed: 3, locked: false, ...extraFields });
    expectOk(send({ op: "set_prefs", prefs: { rhythm: { extras: [extra("x1", "tapan", { instrument: "factory:tapan.folk" }), extra("x2", "chord_pad", { instrument: "factory:chordpad.warm", register_low: 48, register_high: 67 }), extra("x3", "lead", { instrument: "factory:lead.kaval", register_low: 62, register_high: 86 })] } } }));
    const out = path.join(OUT_DIR, "rhythm-rack.wav");
    const r = expectOk(send({ op: "rhythm_render", path: out, bars: 2 }));
    const names = r.stems.map((p) => path.basename(p));
    for (const n of ["tapan-1.wav", "chordpad-2.wav", "lead-3.wav"]) if (!names.includes(n)) throw new Error(`missing stem ${n} in ${names.join(",")}`);
    for (const s of r.stems) audible(s);
    return `${r.stems.length} stems`;
  });
  await step("references: add a render, analyse, phrases", () => {
    const src = path.join(OUT_DIR, "rhythm-rack.wav");
    const info = expectOk(send({ op: "reference_add", path: src }));
    const rep = expectOk(send({ op: "reference_analyze", id: info.id }));
    if (!rep.tempo.length) throw new Error("no tempo candidates");
    const top = rep.tempo[0].bpm;
    const ok = [120, 60, 240].some((t) => Math.abs(top - t) < 3) || rep.tempo.some((c) => Math.abs(c.bpm - 120) < 3);
    if (!ok) throw new Error(`tempo ${rep.tempo.map((t) => t.bpm).join("/")}`);
    const ph = expectOk(send({ op: "reference_phrases", id: info.id }));
    expectOk(send({ op: "reference_remove", id: info.id }));
    return `tempo ${top} BPM, ${rep.segments.length} segments, ${ph.phrases.length} phrases, voiced ${Math.round(ph.voiced_share * 100)}%`;
  });
  await step("song map: sections written and read back, follower reports a section", () => {
    const map = expectOk(send({ op: "song_map" }));
    const bt = (map.ppq * 4 * map.meter_map[0].numerator) / map.meter_map[0].unit;
    const sec = (id, role, a, b) => ({ id, role, name: role, start_tick: a * bt, end_tick: b * bt, energy: 0.5, density: 0.5, tension: 0.5, roles: [], transition_out: null, locked: false, notes: "" });
    const next = { ...map, sections: [sec("s1", "intro", 0, 4), sec("s2", "build", 4, 8), sec("s3", "drop", 8, 16), sec("s4", "break", 16, 20)] };
    const pv = expectOk(send({ op: "set_song_map", song_map: next }));
    if (pv.song_map.sections.length !== 4) throw new Error("sections not stored");
    expectOk(send({ op: "set_prefs", prefs: { rhythm: { follow_map: true } } }));
    const barSeconds = (60 / 120) * (4 * 7) / 8;
    const t = expectOk(send([{ op: "transport_seek", seconds: 5 * barSeconds + 0.1 }, { op: "transport_status" }]))[1];
    if (t.bar !== 6 || t.section !== "build") throw new Error(`bar ${t.bar} section ${t.section}`);
    return `bar ${t.bar} in "${t.section}"`;
  });
  await step("transport: loop in bars stored and cleared", () => {
    const a = expectOk(send({ op: "transport_loop", start_bar: 3, bars: 4 }));
    if (!a.loop_bars || a.loop_bars[0] !== 3 || a.loop_bars[1] !== 7) throw new Error(JSON.stringify(a.loop_bars));
    const b = expectOk(send({ op: "transport_loop" }));
    if (b.loop_bars) throw new Error("loop not cleared");
    return "3–6";
  });
  await step("academy: lessons validate, instances are seeded, scoring and progress round trip", () => {
    const v = expectOk(send({ op: "academy_lessons" }));
    if (v.count < 45) throw new Error(`only ${v.count} lessons`);
    if (!v.lessons.some((l) => l.id === "l5-06-first-song" && l.type === "guided")) throw new Error("no Create-a-song wizard lesson");
    if (!v.lessons.some((l) => l.track === "kaval")) throw new Error("no kaval track");
    const a = expectOk(send({ op: "academy_instance", id: "l2-02-intervals", seed: 5 }));
    const b = expectOk(send({ op: "academy_instance", id: "l2-02-intervals", seed: 5 }));
    if (JSON.stringify(a.instance) !== JSON.stringify(b.instance)) throw new Error("instance not reproducible");
    if (a.instance.kind !== "ear" || a.instance.rounds.length !== 8) throw new Error("expected 8 ear rounds");
    const lo = a.context.lo_midi, hi = a.context.hi_midi;
    for (const r of a.instance.rounds) for (const n of r.prompt) if (n.midi < lo || n.midi > hi) throw new Error(`prompt ${n.midi} outside ${lo}..${hi}`);
    const clap = expectOk(send({ op: "academy_instance", id: "l1-04-pulse", seed: 1 }));
    const sc = expectOk(send({ op: "academy_score_rhythm", onsets: clap.instance.grid, grid: clap.instance.grid, level: "beginner" }));
    if (Math.abs(sc.score - 1) > 1e-9) throw new Error(`perfect claps scored ${sc.score}`);
    const before = v.lessons.find((l) => l.id === "l0-03-clipping").passed;
    const rec = expectOk(send({ op: "academy_record", id: "l0-03-clipping", score: 1, passed: true }));
    if (!rec.passed_at_ms) throw new Error("record did not pass");
    const after = expectOk(send({ op: "academy_lessons" })).lessons.find((l) => l.id === "l0-03-clipping").passed;
    if (!after) throw new Error("progress not read back");
    if (!before) expectOk(send({ op: "academy_reset", id: "l0-03-clipping" }));
    return `${v.count} lessons, context ${lo}..${hi}, rhythm score ${sc.score}`;
  });
  await step("trainer: history log round trip; fingering overrides saved, applied and restored", () => {
    const before = expectOk(send({ op: "trainer_history", limit: 500 })).length;
    const entry = { at_ms: 0, take_id: null, mode: "rhythm", level: "beginner", tempo_percent: 100, expected: 8, hits: 7, wrong_pitch: 1, early: 0, late: 0, missed: 0, extra: 0, mean_abs_cents: 12.5, mean_abs_ms: 31, latency_ms: 0 };
    expectOk(send({ op: "trainer_log", entry }));
    const hist = expectOk(send({ op: "trainer_history", limit: 500 }));
    if (hist.length !== Math.min(500, before + 1) || hist[0].hits !== 7) throw new Error(`history ${before} → ${hist.length}`);
    const orig = expectOk(send({ op: "fingering_overrides", instrument: "kaval-d" })).overrides;
    const set = expectOk(send({ op: "fingering_overrides", instrument: "kaval-d", overrides: [...orig, { midi: 69, register: 1, holes: "ccoxoooo", alt: false, note: null }] })).overrides;
    if (!set.some((o) => o.midi === 69 && o.holes === "ccoxoooo")) throw new Error("override not saved");
    const bad = send({ op: "fingering_overrides", instrument: "kaval-d", overrides: [{ midi: 69, register: 1, holes: "zzzz", alt: false, note: null }] })[0];
    if (bad.ok) throw new Error("bad holes accepted");
    expectOk(send({ op: "fingering_overrides", instrument: "kaval-d", overrides: orig }));
    return `history ${hist.length}, override round trip ok`;
  });
  await step("live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced)", async () => {
    const started = send({ op: "live_start" })[0];
    if (!started.ok) {
      const msg = String(started.error ?? "");
      if (/device|input|no_device|NoDevice|not found/i.test(msg)) return "not run: no input device (" + msg + ")";
      throw new Error(msg);
    }
    if (!started.result.running) throw new Error("not running after start");
    await sleep(700);
    const v1 = expectOk(send({ op: "live_tick" }));
    if (!v1.running || !v1.status) throw new Error("tick without status");
    const v2 = expectOk(send({ op: "live_set", locks: { tonic: true, mode: true, chord: true, drone: true }, bypass: true }));
    if (!v2.status.bypass || !v2.status.locks.chord) throw new Error("locks/bypass not applied");
    expectOk(send({ op: "live_set", locks: { tonic: false, mode: false, chord: false, drone: false }, bypass: false }));
    const stopped = expectOk(send({ op: "live_stop" }));
    if (stopped.running) throw new Error("still running");
    const v3 = expectOk(send({ op: "live_tick" }));
    if (v3.running) throw new Error("tick says running after stop");
    return `status after ${v1.status.seconds.toFixed(2)} s: ${v1.status.notes} notes, ${v1.status.phrases} phrases, mode ${v1.status.mode}`;
  });
  await step("make the song: one call turns the newest take into scale + tempo + background + map + track (silent)", () => {
    const r = expectOk(send({ op: "make_song", style: "liquid_dnb" }));
    if (!Array.isArray(r.facts) || r.facts.length < 5) throw new Error("too few decisions: " + JSON.stringify(r.facts));
    const st = expectOk(send({ op: "state" }));
    if (!st.project.timing.bpm) throw new Error("no tempo after make_song");
    const arr = expectOk(send({ op: "arrangement" })).arrangement;
    if (!arr.tracks.some((tr) => tr.clips.length > 0 && tr.highpass_hz >= 100)) throw new Error("melody track not set up");
    // Silent run: switch the background it started back off.
    expectOk(send({ op: "layers", drone: false, pad: false, ambient: false, rhythm: false }));
    return `${r.facts.length} decisions, ${st.project.timing.bpm} BPM, sections ${r.sections}`;
  });
  await step("take: the ▶ path — load the latest take through load_take (no sound) and see it loaded", () => {
    const r = expectOk(send({ op: "load_take" }));
    if (!r.loaded_id) throw new Error(JSON.stringify(r));
    return `take ${r.name}, ${r.duration_s?.toFixed(1)} s, analysed ${r.analysed} (${r.notes} notes)`;
  });
  await step("now playing: the bar's snapshot has every source, stop_all leaves silence", async () => {
    const np = expectOk(send({ op: "now_playing" }));
    for (const k of ["recording", "take", "notes", "background", "song", "metronome", "live", "any"]) if (!(k in np)) throw new Error(`missing ${k}`);
    const after = expectOk(send({ op: "stop_all" }));
    if (after.any !== false) throw new Error(JSON.stringify(after));
    return `before: any=${np.any}; after stop_all: silence`;
  });
  await step("tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape", () => {
    const before = expectOk(send({ op: "tuner" }));
    const r = expectOk(send({ op: "tuner", a4: 432, instrument: "guitar", lock: 45 }));
    if (r.a4 !== 432 || r.instrument !== "guitar" || r.lock !== 45) throw new Error(JSON.stringify(r));
    if (!("hz" in r) || !("reading" in r) || !("listening" in r)) throw new Error("reading fields missing");
    const bad = send({ op: "tuner", a4: 10 })[0];
    if (!bad.ok || bad.result.a4 !== 400) throw new Error(`A4 not clamped: ${JSON.stringify(bad)}`);
    expectOk(send({ op: "tuner", a4: before.a4_pref, instrument: before.instrument, lock: before.lock }));
    return `a4 ${r.a4} Hz, ${r.instrument}, lock ${r.lock}, listening ${r.listening}`;
  });
  await step("tuner: the sound bed switches the microphone to the bass range (30 Hz) and back", async () => {
    const before = expectOk(send({ op: "tuner" }));
    expectOk(send({ op: "screen", screen: "tuner" }));
    await sleep(600);
    expectOk(send({ op: "tuner", instrument: "kaval-d", lock: null }));
    await sleep(900);
    const k = expectOk(send({ op: "tuner" }));
    if (!k.listening) {
      expectOk(send({ op: "tuner", instrument: before.instrument, lock: before.lock }));
      return "no input device, range not checked";
    }
    if (k.min_hz !== 70) throw new Error(`kaval range ${k.min_hz}`);
    expectOk(send({ op: "tuner", instrument: "monochord", lock: 26 }));
    await sleep(900); // the screen restarts the monitor with the new range
    const bed = expectOk(send({ op: "tuner" }));
    if (bed.min_hz !== 30 || !bed.listening) throw new Error(`bed ${JSON.stringify({ min_hz: bed.min_hz, listening: bed.listening })}`);
    expectOk(send({ op: "tuner", instrument: before.instrument, lock: before.lock }));
    await sleep(600);
    return "kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored";
  });
  await step("sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM)", async () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0"><work><work-title>Smoke sheet</work-title></work>
<part-list><score-part id="P1"><part-name>Kaval</part-name></score-part></part-list>
<part id="P1"><measure number="1">
  <attributes><divisions>2</divisions><time><beats>7</beats><beat-type>8</beat-type></time></attributes>
  <direction><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>60</per-minute></metronome></direction-type></direction>
  <note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration></note>
  <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration></note>
  <note><rest/><duration>1</duration></note>
  <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>2</duration><tie type="start"/></note>
</measure><measure number="2">
  <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>2</duration><tie type="stop"/></note>
  <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration></note>
  <note><chord/><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration></note>
  <note><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration></note>
</measure></part></score-partwise>`;
    const p = path.join(OUT_DIR, "smoke-sheet.musicxml");
    fs.writeFileSync(p, xml);
    const r = expectOk(send({ op: "import_audio", path: p }));
    if (!r.take_id) throw new Error("no take_id");
    await sleep(2200); // the state snapshot is rewritten ~1.5 s after a change
    const st = readState();
    const tk = st.project.takes.find((t) => t.id === r.take_id);
    if (!tk || tk.source !== "midi_file") throw new Error(`take ${JSON.stringify(tk)}`);
    const secs = tk.frames && tk.sample_rate ? tk.frames / tk.sample_rate : null;
    // 6 s of music at 60 BPM (plus the synth's release tail).
    if (secs !== null && (secs < 5.9 || secs > 8)) throw new Error(`length ${secs.toFixed(2)} s`);
    return `take ${tk.number} (${tk.source})${secs !== null ? `, ${secs.toFixed(1)} s` : ""}`;
  });
  await step("presets: save, list, load", () => {
    const saved = expectOk(send({ op: "save_preset", kind: "rhythm", name: "smoke rhythm", tags: ["smoke"], notes: "automated" }));
    const list = expectOk(send({ op: "list_presets", kind: "rhythm", query: "smoke" })).entries;
    if (!list.some((p) => p.name === "smoke rhythm")) throw new Error("not listed");
    expectOk(send({ op: "load_preset", id: saved.id, go: false }));
    return saved.id;
  });
  await step("CLAP: fixture scan + render", () => {
    const folder = path.join(ROOT, "target", "alt", "debug");
    const clap = path.join(folder, "ethnoflow-fixture.clap");
    if (!fs.existsSync(clap)) return "not run: build the fixture first (cargo test -p ethnoflow-plugin-host)";
    const scan = expectOk(send({ op: "clap_scan", folders: [folder] }));
    const f = scan.files.find((x) => x.path.endsWith("ethnoflow-fixture.clap"));
    if (!f || !f.plugins.length) throw new Error("fixture not listed");
    const out = path.join(OUT_DIR, "clap-fixture.wav");
    expectOk(send({ op: "clap_render", path: clap, plugin_id: f.plugins[0].id, out, midi: 62 }));
    return audible(out, 0.05);
  });
  await step("atmosphere: choir + bloom layers render audibly through the export", () => {
    const a = expectOk(send({ op: "add_layer", kind: "choir", name: "smoke choir" }));
    const b = expectOk(send({ op: "add_layer", kind: "bloom", name: "smoke bloom", source: { density: 60, note_s: 0.5, attack_s: 0.02 } }));
    const out = path.join(OUT_DIR, "atmo.wav");
    try {
      const r = expectOk(send({ op: "export_song", path: out, seconds: 6, ambient: true, rhythm: false, drone: false, pad: false }));
      const stem = r.stems.find((p) => /atmosphere/i.test(path.basename(p)));
      if (!stem) throw new Error(`no atmosphere stem in ${r.stems.map((p) => path.basename(p)).join(",")}`);
      return audible(stem, 0.02);
    } finally {
      send({ op: "remove_layer", id: a.id ?? a.layer?.id ?? "smoke choir" });
      send({ op: "remove_layer", name: "smoke bloom" });
      send({ op: "remove_layer", name: "smoke choir" });
    }
  });
  await step("studio: track strip — pan and send are real in the export", () => {
    const src = path.join(OUT_DIR, "rhythm-four.wav");
    const imp = expectOk(send({ op: "import_audio", path: src }));
    if (!imp.take_id) throw new Error("no take after import");
    const arr = expectOk(send({ op: "arrangement" })).arrangement;
    const track = (pan, sendAmt) => ({ id: "smoke-t1", name: "smoke", gain_db: 0, pan, mute: false, solo: false, lowpass_hz: 20000, highpass_hz: 10, send: sendAmt, clips: [{ id: "smoke-c1", take_id: imp.take_id, start: 0, offset: 0, length: 2.5, gain_db: 0, fade_in: 0, fade_out: 0, mute: false }] });
    try {
      expectOk(send({ op: "set_arrangement", arrangement: { ...arr, tracks: [track(-1, 0)], loop_region: null } }));
      const left = path.join(OUT_DIR, "strip-left.wav");
      const r1 = expectOk(send({ op: "export_song", path: left, seconds: 3, rhythm: false, drone: false, pad: false, ambient: false }));
      const stem = r1.stems.find((p) => /clips/i.test(path.basename(p)));
      if (!stem) throw new Error("no clips stem");
      const [l, rr] = channelRms(stem, 0.2, 2.3);
      if (!(l > rr * 8)) throw new Error(`pan -1 but L ${l.toFixed(4)} vs R ${rr.toFixed(4)}`);
      expectOk(send({ op: "set_arrangement", arrangement: { ...arr, tracks: [track(0, 1)], loop_region: null } }));
      const sent = path.join(OUT_DIR, "strip-send.wav");
      const r2 = expectOk(send({ op: "export_song", path: sent, seconds: 5, rhythm: false, drone: false, pad: false, ambient: false }));
      const tail = channelRms(r2.path, 3.2, 4.8)[0];
      expectOk(send({ op: "set_arrangement", arrangement: { ...arr, tracks: [track(0, 0)], loop_region: null } }));
      const dry = path.join(OUT_DIR, "strip-dry.wav");
      const r3 = expectOk(send({ op: "export_song", path: dry, seconds: 5, rhythm: false, drone: false, pad: false, ambient: false }));
      const dryTail = channelRms(r3.path, 3.2, 4.8)[0];
      if (!(tail > dryTail * 4 && tail > 1e-4)) throw new Error(`send tail ${tail.toExponential(2)} vs dry ${dryTail.toExponential(2)}`);
      return `L/R ${(l / Math.max(rr, 1e-9)).toFixed(0)}×, reverb tail ${(20 * Math.log10(tail)).toFixed(0)} dBFS`;
    } finally {
      send({ op: "set_arrangement", arrangement: { ...arr, tracks: [], loop_region: null } });
    }
  });
  await step("export: whole song offline with stems", () => {
    const out = path.join(OUT_DIR, "song.wav");
    const r = expectOk(send({ op: "export_song", path: out, seconds: 6 }));
    if (!fs.existsSync(r.path)) throw new Error("no file");
    // The background must be in the export even while the live layers are off (the live
    // fade mute and the section overlay are not export state).
    const drums = r.stems.find((f) => /drums.wav$/.test(f));
    if (!drums) throw new Error("no drums stem");
    const dp = wavStats(drums).peak;
    if (dp < 0.01) throw new Error(`drums stem silent (peak ${dp.toFixed(4)})`);
    return `${path.basename(r.path)} ${wavStats(r.path).seconds.toFixed(1)} s, stems ${r.stems.length}, drums peak ${dp.toFixed(2)}`;
  });
  if (SOUND) {
    await step("sound: play_take plays the latest take for 1 s, stop_take stops it", async () => {
      const r = expectOk(send({ op: "play_take" }));
      if (r.mode !== "playing") throw new Error(JSON.stringify(r));
      await sleep(1000);
      const st = expectOk(send({ op: "stop_take" }));
      if (st.mode !== "idle" || !(st.cursor_s > 0.5)) throw new Error(JSON.stringify(st));
      return `played ${st.cursor_s.toFixed(2)} s of ${r.duration_s?.toFixed(1)} s`;
    });
    await step("sound: stop_all silences a playing take and the background at once", async () => {
      expectOk(send({ op: "play_take" }));
      expectOk(send({ op: "layers", drone: true }));
      await sleep(700);
      const np = expectOk(send({ op: "now_playing" }));
      if (!np.take || !np.background.drone) throw new Error(JSON.stringify(np));
      const after = expectOk(send({ op: "stop_all" }));
      if (after.any) throw new Error(JSON.stringify(after));
      return "take + drone → stop_all → silence";
    });
    await step("sound: rhythm live for 2 s (meters move)", async () => {
      expectOk(send({ op: "layers", rhythm: true }));
      await sleep(2500);
      const rs = expectOk(send({ op: "rhythm_status" }));
      expectOk(send({ op: "layers", rhythm: false }));
      if (!rs || !rs.playing) throw new Error("rhythm status not playing");
      if (!(Math.max(...rs.peaks) > 0.001)) throw new Error("no signal on the part meters");
      return `bar ${rs.bar}, peaks ${rs.peaks.map((p) => p.toFixed(2)).join("/")}${rs.section ? ", section " + rs.section.name : ""}`;
    });
    await step("sound: drone + pad for 1.5 s", async () => {
      expectOk(send({ op: "layers", drone: true, pad: true }));
      await sleep(1500);
      expectOk(send({ op: "layers", drone: false, pad: false }));
    });
    await step("sound: instrument audition 1 s", () => expectOk(send({ op: "instrument_audition", id: "factory:bass.pluck", seconds: 1 })).played);
  }
  await step("cleanup: your prefs, project and screen restored", () => {
    if (original.prefs.rhythm || original.prefs.mixer) expectOk(send({ op: "set_prefs", prefs: original.prefs }));
    if (original.project && original.project !== PROJECT) expectOk(send({ op: "open_project", path: original.project }));
    expectOk(send({ op: "screen", screen: original.screen }));
    return original.project ? path.basename(original.project) : "no project was open";
  });

  const passed = results.filter((r) => r.ok).length;
  const lines = [
    `# Smoke report — ${new Date().toISOString()}`,
    "",
    `App driven through the assistant bridge. ${passed}/${results.length} steps passed${SOUND ? " (with sound steps)" : " (silent run: sound steps skipped)"}.`,
    "",
    "| Step | Result | Detail | ms |",
    "|---|---|---|---|",
    ...results.map((r) => `| ${r.name} | ${r.ok ? "PASS" : "**FAIL**"} | ${r.detail.replace(/\|/g, "/")} | ${r.ms} |`),
    "",
    "Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.",
    "",
  ];
  fs.mkdirSync(path.join(ROOT, "docs", "testing"), { recursive: true });
  fs.writeFileSync(path.join(ROOT, "docs", "testing", "SMOKE_REPORT.md"), lines.join("\n"));
  console.log(`\n${passed}/${results.length} passed → docs/testing/SMOKE_REPORT.md`);
  process.exit(passed === results.length ? 0 : 1);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
