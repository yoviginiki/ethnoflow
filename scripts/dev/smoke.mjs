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
/** Onset times (s) of a float WAV: a 5 ms frame whose RMS jumps 4× over the previous 50 ms. */
function onsetTimes(p) {
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
  const total = Math.floor(data.length / 4 / fmt.ch);
  const hop = Math.round(fmt.sr * 0.005);
  const rms = [];
  for (let f = 0; f + hop <= total; f += hop) {
    let acc = 0;
    for (let i = f; i < f + hop; i++) for (let c = 0; c < fmt.ch; c++) acc += data.readFloatLE((i * fmt.ch + c) * 4) ** 2;
    rms.push(Math.sqrt(acc / (hop * fmt.ch)));
  }
  const out = [];
  let last = -1;
  for (let i = 10; i < rms.length; i++) {
    const prev = rms.slice(i - 10, i).reduce((a, v) => a + v, 0) / 10;
    if (rms[i] > 0.003 && rms[i] > prev * 4 && (last < 0 || i - last > 8)) {
      out.push((i * hop) / fmt.sr);
      last = i;
    }
  }
  return out;
}
const audible = (p, min = 0.02) => {
  const s = wavStats(p);
  if (!(s.peak > min && s.peak <= 1.0)) throw new Error(`${path.basename(p)} peak ${s.peak.toFixed(3)}`);
  return `${path.basename(p)} ${s.seconds.toFixed(1)} s peak ${s.peak.toFixed(2)}`;
};

const SCREENS = ["home", "play", "tuner", "compose", "studio", "ambient", "scapes", "rhythm", "progression", "live", "library", "instrument", "songmap", "samples", "references", "academy", "create", "start", "help"];

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
    const prog = expectOk(send({ op: "progression" }));
    if (!prog.progression.follow || prog.progression.genre !== "liquid") throw new Error(`style did not set the progression: ${JSON.stringify(prog.progression.genre)}`);
    const sc = expectOk(send({ op: "scapes" }));
    if (sc.current !== "cyber-rain") throw new Error(`style did not build the journey: ${sc.current}`);
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
  await step("home: the five areas open their main screens, Continue opens Play with the latest take", () => {
    const list = expectOk(send({ op: "area" }));
    const want = ["play", "background", "song", "learn", "library"];
    if (JSON.stringify(list.areas) !== JSON.stringify(want)) throw new Error(JSON.stringify(list));
    const opened = [];
    for (const id of want) {
      const r = expectOk(send({ op: "area", area: id }));
      opened.push(`${id}→${r.screen}`);
    }
    const c = expectOk(send({ op: "continue" }));
    if (c.screen !== "play" || !c.loaded_id || c.loaded_id !== c.latest_id) throw new Error(JSON.stringify(c));
    return `${opened.join(", ")}; continue → play with the latest take loaded`;
  });
  await step("samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders)", () => {
    const before = expectOk(send({ op: "sample_folders" })).folders;
    const probe = path.join(os.tmpdir(), "ethnoflow-smoke-samples");
    const added = expectOk(send({ op: "sample_folders", add: [probe] })).folders;
    if (!added.includes(probe)) throw new Error(JSON.stringify(added));
    const after = expectOk(send({ op: "sample_folders", remove: [probe] })).folders;
    if (after.includes(probe) || after.length !== before.length) throw new Error(JSON.stringify(after));
    return `${before.length} folder(s) kept, probe added and removed`;
  });
  await step("nyquist: every plug-in in the folder runs with its defaults (bundled + Documents\\EthnoFlow\\Plugins)", () => {
    const { checks } = expectOk(send({ op: "ny_check" }));
    if (!Array.isArray(checks) || checks.length < 5) throw new Error(`checks ${JSON.stringify(checks).slice(0, 200)}`);
    const bad = checks.filter((c) => !c.ok);
    if (bad.length) throw new Error(`${bad.length} failing: ${bad.map((c) => `${c.name}: ${c.error}`).join(" | ")}`);
    const slow = checks.reduce((m, c) => (c.ms > m.ms ? c : m), checks[0]);
    return `${checks.length} plug-ins OK (${checks.filter((c) => c.kind === "generate").length} generators); slowest ${slow.name} ${slow.ms} ms`;
  });

  await step("nyquist generator → WAV sample for the sample lists (bundled Tone, 1.5 s pluck at D4)", () => {
    const r = expectOk(send({ op: "ny_generate_wav", plugin: "bundled:tone", values: [{ var: "pitch", value: 62 }, { var: "dur", value: 1.5 }, { var: "kind", value: 0 }], name: "smoke tone" }));
    try {
      if (!fs.existsSync(r.path) || !/Samples[\\/]Generated/.test(r.path)) throw new Error(`generated ${JSON.stringify(r)}`);
      if (Math.abs(r.seconds - 1.5) > 0.01) throw new Error(`generated length ${r.seconds.toFixed(3)} s`);
      const folders = expectOk(send({ op: "sample_folders" })).folders;
      if (!folders.some((f) => /Generated$/.test(f))) throw new Error(`Generated folder not in sample folders: ${JSON.stringify(folders)}`);
      return `${r.path.split(/[\\/]/).pop()} · ${r.seconds.toFixed(2)} s · ${r.sample_rate} Hz · ${r.channels} ch`;
    } finally {
      fs.rmSync(r.path, { force: true });
    }
  });

  await step("soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored", () => {
    const list = expectOk(send({ op: "scapes" }));
    if (!Array.isArray(list.worlds) || list.worlds.length < 6) throw new Error(JSON.stringify(list));
    const atmoBefore = JSON.stringify(readState().ambient ?? null);
    const r = expectOk(send({ op: "scapes_build", world: "rhodope-night", minutes: 2, seed: 7 }));
    try {
      if (r.scenes.length !== 4 || Math.abs(r.end_s - 120) > 2 || r.lanes !== 3 || r.playing !== false) throw new Error(JSON.stringify({ scenes: r.scenes.length, end_s: r.end_s, lanes: r.lanes, playing: r.playing }));
      const now = expectOk(send({ op: "scapes" }));
      if (now.current !== "rhodope-night" || !now.phase || now.on !== false) throw new Error(JSON.stringify(now));
      const atmoAfter = JSON.stringify(readState().ambient ?? null);
      if (atmoAfter !== atmoBefore) throw new Error("the atmosphere rack changed");
      return `${r.layers} layers in the Soundscapes rack, scenes ${r.scenes.map((sc) => `${sc.name} ${sc.duration_s}s`).join(" → ")}, atmosphere untouched`;
    } finally {
      if (r.previous) expectOk(send({ op: "set_scapes", ambient: r.previous }));
    }
  });
  await step("progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent)", () => {
    const before = expectOk(send({ op: "progression" })).progression;
    try {
      const g = expectOk(send({ op: "progression_set", generate: true, genre: "liquid", seed: 7, bars: 8 }));
      if (g.total_bars !== 8 || g.progression.steps.length < 2 || g.labels.length !== g.progression.steps.length) throw new Error(JSON.stringify(g));
      const e = expectOk(send({ op: "progression_set", steps: [{ degree: 1, quality: "sus2", bars: 4 }, { degree: 6, quality: "maj7", bars: 4 }], follow: true }));
      if (!e.progression.edited || e.progression.follow !== true || e.total_bars !== 8) throw new Error(JSON.stringify(e));
      const np = expectOk(send({ op: "now_playing" }));
      if (np.any !== false) throw new Error("follow must not start sound by itself");
      return `liquid seed 7: ${g.labels.join(" · ")}; edited to ${e.labels.join(" · ")}, follow on, silence`;
    } finally {
      expectOk(send({ op: "progression_set", follow: false, steps: before.steps }));
    }
  });
  await step("follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode", () => {
    const built = expectOk(send({ op: "scapes_build", world: "rhodope-night", minutes: 2, seed: 3 }));
    try {
      const rack = readState().scapes;
      if (!rack) throw new Error("no scapes rack in state");
      expectOk(send({ op: "set_scapes", ambient: { ...rack, follow_chord: true } }));
      expectOk(send({ op: "chord_pitches", midi: [50, 53, 57] }));
      const on = expectOk(send({ op: "rack_pitches", rack: "scapes" }));
      const pcOf = (hz) => ((Math.round(69 + 12 * Math.log2(hz / 440)) % 12) + 12) % 12;
      const tonal = on.layers.filter((l) => l.hz.length > 0);
      if (tonal.length === 0) throw new Error("no tonal layers in the journey");
      for (const l of tonal) for (const hz of l.hz) if (![2, 5, 9].includes(pcOf(hz))) throw new Error(`${l.name}: ${hz.toFixed(1)} Hz is not D / F / A`);
      expectOk(send({ op: "chord_pitches", midi: null }));
      const off = expectOk(send({ op: "rack_pitches", rack: "scapes" }));
      const offTonal = off.layers.filter((l) => l.hz.length > 0);
      const outside = offTonal.some((l) => l.hz.some((hz) => ![2, 5, 9].includes(pcOf(hz))));
      if (!outside) throw new Error("without a chord the layers should play the whole mode again");
      return `${tonal.length} tonal layers on D F A, back to the mode without a chord`;
    } finally {
      if (built.previous) expectOk(send({ op: "set_scapes", ambient: built.previous }));
    }
  });
  await step("the atmosphere knows the bar: with sync = every beat, Soundscapes bells land on the beat grid of the export", () => {
    const built = expectOk(send({ op: "scapes_build", world: "rhodope-night", minutes: 2, seed: 3 }));
    try {
      const rack = readState().scapes;
      if (!rack || !rack.layers.length) throw new Error("no scapes rack in state");
      // One short, dense bells layer on the beat grid; nothing else in the rack.
      const base = rack.layers.find((l) => l.source.kind === "bells") ?? rack.layers[0];
      const bells = { ...base, mute: false, solo: false, gain_db: -6, source: { kind: "bells", density: 120, decay_s: 0.25, brightness: 0.5, velocity_spread: 0, tension: 0, tonal: true, register: [76, 88] } };
      expectOk(send({ op: "set_scapes", ambient: { ...rack, layers: [bells], program: { scenes: [], loop_region: null }, automation: [], sync: 1 } }));
      const bpm = readState().project?.timing?.bpm ?? 120;
      const out = path.join(OUT_DIR, "on-the-bar.wav");
      const r = expectOk(send({ op: "export_song", path: out, seconds: 8, scapes: true, ambient: false, drone: false, pad: false, target_lufs: null }));
      const stem = r.stems.find((p) => /soundscapes/i.test(path.basename(p)));
      if (!stem) throw new Error("no soundscapes stem");
      const beat = 60 / bpm;
      const onsets = onsetTimes(stem).filter((t) => t > 0.3);
      if (onsets.length < 6) throw new Error(`only ${onsets.length} onsets`);
      const onGrid = onsets.filter((t) => Math.abs(t / beat - Math.round(t / beat)) * beat < 0.03);
      if (onGrid.length < onsets.length * 0.8) throw new Error(`${onGrid.length}/${onsets.length} onsets on the ${bpm} BPM grid: ${onsets.map((t) => t.toFixed(3)).join(" ")}`);
      // Free-running (sync 0): the same layer strikes off the grid too.
      expectOk(send({ op: "set_scapes", ambient: { ...rack, layers: [bells], program: { scenes: [], loop_region: null }, automation: [], sync: 0 } }));
      const free = path.join(OUT_DIR, "off-the-bar.wav");
      const r2 = expectOk(send({ op: "export_song", path: free, seconds: 8, scapes: true, ambient: false, drone: false, pad: false, target_lufs: null }));
      const stem2 = r2.stems.find((p) => /soundscapes/i.test(path.basename(p)));
      const onsets2 = onsetTimes(stem2).filter((t) => t > 0.3);
      const off2 = onsets2.filter((t) => Math.abs(t / beat - Math.round(t / beat)) * beat >= 0.03);
      if (onsets2.length >= 6 && off2.length === 0) throw new Error("free-running bells all on the grid?");
      return `${onGrid.length}/${onsets.length} on the grid at ${bpm} BPM; free: ${off2.length}/${onsets2.length} off`;
    } finally {
      if (built.previous) expectOk(send({ op: "set_scapes", ambient: built.previous }));
    }
  });
  await step("history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them", () => {
    const h0 = expectOk(send({ op: "rack_history", rack: "scapes" }));
    if (h0.count < 2) throw new Error(`only ${h0.count} step(s): ${JSON.stringify(h0.labels)}`);
    const u = expectOk(send({ op: "rack_history", rack: "scapes", undo: 1 }));
    if (u.index !== h0.index - 1) throw new Error(JSON.stringify(u));
    const r = expectOk(send({ op: "rack_history", rack: "scapes", redo: 1 }));
    if (r.index !== h0.index || r.layers !== h0.layers) throw new Error(JSON.stringify(r));
    return `${h0.count} steps (${h0.labels.slice(-3).join(" ← ")}), undo → step ${u.index + 1}, redo → step ${r.index + 1}`;
  });
  await step("soundscapes: the old screen id opens Atmosphere in the modules view (alias)", () => {
    const r = expectOk(send({ op: "screen", screen: "soundscapes" }));
    if (r.screen !== "ambient") throw new Error(`screen is ${r.screen}`);
    const st = readState();
    if (st.prefs?.ambientView !== undefined && st.prefs.ambientView !== "modules") throw new Error(`ambientView ${st.prefs.ambientView}`);
    return `screen soundscapes → ambient (modules view)`;
  });
  await step("areas: every tab of Play / Background / Song / Learn / Library is a screen of its area", () => {
    const list = expectOk(send({ op: "area" }));
    const want = { play: ["play", "edit", "tuner"], background: ["compose", "ambient", "scapes", "rhythm", "progression", "instrument", "live"], song: ["studio", "songmap", "create", "start"], learn: ["academy", "references"], library: ["library", "samples"] };
    for (const id of Object.keys(want)) if (JSON.stringify(list.tabs?.[id]) !== JSON.stringify(want[id])) throw new Error(`${id}: ${JSON.stringify(list.tabs?.[id])}`);
    let n = 0;
    for (const [id, tabs] of Object.entries(want)) {
      for (const sc of tabs) {
        const r = expectOk(send({ op: "screen", screen: sc }));
        if (r.screen !== sc) throw new Error(`screen is ${r.screen}`);
        const a = expectOk(send({ op: "area" }));
        if (a.current !== id) throw new Error(`${sc}: area is ${a.current}, want ${id}`);
        n++;
      }
    }
    return `${n} tabs in 5 areas, each under its own area`;
  });
  await step("keyboard: the dock switches on with the project's voice (live synth runs), points at the instrument, off", async () => {
    const before = expectOk(send({ op: "keyboard" }));
    try {
      const on = expectOk(send({ op: "keyboard", on: true, source: "voice", octave: 60 }));
      if (!on.on || on.source !== "voice" || !on.synth_running) throw new Error(`on ${JSON.stringify(on)}`);
      if (SOUND) {
        expectOk(send({ op: "note", midi: 62, on: true, velocity: 0.6 }));
        await new Promise((r) => setTimeout(r, 500));
        const h = expectOk(send({ op: "note", midi: 62, on: false }));
        if (h.held.length !== 0) throw new Error("note still held");
      }
      // K4: the wheels' values reach the target through the same command the MIDI thread uses.
      const w = expectOk(send({ op: "keyboard", bend: 2, mod: 0.5 }));
      if (w.bend !== 2 || w.mod !== 0.5) throw new Error(`bend/mod ${JSON.stringify([w.bend, w.mod])}`);
      expectOk(send({ op: "keyboard", bend: 0, mod: 0 }));
      // K3: the pad source turns a key into the pad's voicing while held, and gives it back.
      const pad0 = expectOk(send({ op: "keyboard", source: "pad" }));
      const down = expectOk(send({ op: "note", midi: 62, on: true }));
      await new Promise((r) => setTimeout(r, 300));
      const padHeld = expectOk(send({ op: "keyboard" }));
      if (!Array.isArray(padHeld.pad_chord) || !padHeld.pad_chord.includes(62)) throw new Error(`pad chord while held: ${JSON.stringify(padHeld.pad_chord)} (${JSON.stringify(down)})`);
      expectOk(send({ op: "note", midi: 62, on: false }));
      await new Promise((r) => setTimeout(r, 300));
      const padUp = expectOk(send({ op: "keyboard" }));
      if (JSON.stringify(padUp.pad_chord) !== JSON.stringify(pad0.pad_chord)) throw new Error(`pad chord not restored: ${JSON.stringify(padUp.pad_chord)} vs ${JSON.stringify(pad0.pad_chord)}`);
      expectOk(send({ op: "keyboard", source: "drone" }));
      expectOk(send({ op: "note", midi: 50, on: true }));
      await new Promise((r) => setTimeout(r, 300));
      const droneHeld = expectOk(send({ op: "keyboard" }));
      if (droneHeld.drone_root !== 50) throw new Error(`drone root while held: ${droneHeld.drone_root}`);
      expectOk(send({ op: "notes_off" }));
      await new Promise((r) => setTimeout(r, 300));
      if (expectOk(send({ op: "keyboard" })).drone_root !== pad0.drone_root) throw new Error("drone root not restored");
      expectOk(send({ op: "stop_all" }));
      const inst = expectOk(send({ op: "keyboard", source: "instrument" }));
      if (inst.source !== "instrument" || inst.synth_running) throw new Error(`instrument ${JSON.stringify(inst)}`);
      const off = expectOk(send({ op: "keyboard", on: false, source: "voice" }));
      if (off.on || off.synth_running) throw new Error(`off ${JSON.stringify(off)}`);
      return `ports ${before.midi_ports.length}, synth on → instrument → off`;
    } finally {
      send({ op: "keyboard", on: before.on, source: before.source, octave: before.octave });
    }
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
    const takeId = readState().project?.takes?.[0]?.id;
    if (takeId) expectOk(send({ op: "add_layer", kind: "sampler", name: "smoke sampler", take_id: takeId, source: { density: 120, note_s: 0.4, release_s: 0.2, cutoff_hz: 12000, zones: [{ take_id: takeId, root_midi: 48 }, { take_id: takeId, root_midi: 60 }] } }));
    if (takeId) {
      // state.json lags the bridge by a second or so: ask for the snapshot directly.
      const fresh = expectOk(send({ op: "state" }));
      const z = (fresh.ambient?.layers ?? []).find((l) => l.name === "smoke sampler")?.source?.zones;
      if (!z || z.length !== 2) throw new Error("the sampler's two zones did not round-trip");
    }
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
      send({ op: "remove_layer", name: "smoke sampler" });
    }
  });
  await step("atmosphere: a Pulse layer (S3) swells audibly in the export and its params round-trip", () => {
    // Full presence, 1 s period: 6 s of export must carry the pulse.
    const p = expectOk(send({ op: "add_layer", kind: "pulse", name: "smoke pulse", source: { period_s: 1, presence: 1, color: 0.2, softness: 0.4 }, patch: { gain_db: 0 } }));
    const out = path.join(OUT_DIR, "pulse.wav");
    try {
      const fresh = expectOk(send({ op: "state" }));
      const l = (fresh.ambient?.layers ?? []).find((x) => x.name === "smoke pulse");
      if (!l || l.source?.kind !== "pulse" || l.source.period_s !== 1 || l.source.presence !== 1) throw new Error(`pulse layer ${JSON.stringify(l?.source)}`);
      const r = expectOk(send({ op: "export_song", path: out, seconds: 6, ambient: true, rhythm: false, drone: false, pad: false, target_lufs: null }));
      const stem = r.stems.find((x) => /atmosphere/i.test(path.basename(x)));
      if (!stem) throw new Error("no atmosphere stem");
      return audible(stem, 0.02);
    } finally {
      send({ op: "remove_layer", id: p.id ?? "smoke pulse" });
      send({ op: "remove_layer", name: "smoke pulse" });
    }
  });
  await step("studio: track strip — pan, send, EQ / compressor and the loudness target are real in the export", () => {
    const src = path.join(OUT_DIR, "rhythm-four.wav");
    const imp = expectOk(send({ op: "import_audio", path: src }));
    if (!imp.take_id) throw new Error("no take after import");
    const arr = expectOk(send({ op: "arrangement" })).arrangement;
    const track = (pan, sendAmt, fx = {}, envelope = []) => ({ id: "smoke-t1", name: "smoke", gain_db: 0, pan, mute: false, solo: false, lowpass_hz: 20000, highpass_hz: 10, send: sendAmt, ...fx, clips: [{ id: "smoke-c1", take_id: imp.take_id, start: 0, offset: 0, length: 2.5, gain_db: 0, fade_in: 0, fade_out: 0, mute: false, envelope }] });
    try {
      expectOk(send({ op: "set_arrangement", arrangement: { ...arr, tracks: [track(-1, 0)], loop_region: null } }));
      const left = path.join(OUT_DIR, "strip-left.wav");
      const r1 = expectOk(send({ op: "export_song", path: left, seconds: 3, rhythm: false, drone: false, pad: false, ambient: false, target_lufs: null }));
      const stem = r1.stems.find((p) => /clips/i.test(path.basename(p)));
      if (!stem) throw new Error("no clips stem");
      const [l, rr] = channelRms(stem, 0.2, 2.3);
      if (!(l > rr * 8)) throw new Error(`pan -1 but L ${l.toFixed(4)} vs R ${rr.toFixed(4)}`);
      expectOk(send({ op: "set_arrangement", arrangement: { ...arr, tracks: [track(0, 1)], loop_region: null } }));
      const sent = path.join(OUT_DIR, "strip-send.wav");
      const r2 = expectOk(send({ op: "export_song", path: sent, seconds: 5, rhythm: false, drone: false, pad: false, ambient: false, target_lufs: null }));
      const tail = channelRms(r2.path, 3.2, 4.8)[0];
      expectOk(send({ op: "set_arrangement", arrangement: { ...arr, tracks: [track(0, 0)], loop_region: null } }));
      const dry = path.join(OUT_DIR, "strip-dry.wav");
      const r3 = expectOk(send({ op: "export_song", path: dry, seconds: 5, rhythm: false, drone: false, pad: false, ambient: false, target_lufs: null }));
      const dryTail = channelRms(r3.path, 3.2, 4.8)[0];
      if (!(tail > dryTail * 4 && tail > 1e-4)) throw new Error(`send tail ${tail.toExponential(2)} vs dry ${dryTail.toExponential(2)}`);
      if (r3.gain_db !== 0) throw new Error(`target_lufs null but gain ${r3.gain_db}`);
      // Home-studio item 4: the track compressor (threshold −36, ratio 20) squeezes the
      // clips stem (0.1 ms attack so the drum transients are caught too).
      expectOk(send({ op: "set_arrangement", arrangement: { ...arr, tracks: [track(0, 0, { comp_threshold_db: -36, comp_ratio: 20, comp_attack_ms: 0.1, comp_release_ms: 300 })], loop_region: null } }));
      const squeezed = path.join(OUT_DIR, "strip-comp.wav");
      const r4 = expectOk(send({ op: "export_song", path: squeezed, seconds: 3, rhythm: false, drone: false, pad: false, ambient: false, target_lufs: null }));
      const dryPeak = wavStats(r3.stems.find((p) => /clips/i.test(path.basename(p)))).peak;
      const compPeak = wavStats(r4.stems.find((p) => /clips/i.test(path.basename(p)))).peak;
      if (!(compPeak < dryPeak * 0.5 && compPeak > 0.001)) throw new Error(`compressor: peak ${compPeak.toFixed(4)} vs dry ${dryPeak.toFixed(4)}`);
      // A3: a gain envelope (full until 1.2 s, silent from 1.3 s) shapes the clips stem.
      expectOk(send({ op: "set_arrangement", arrangement: { ...arr, tracks: [track(0, 0, {}, [{ at: 0, db: 0 }, { at: 1.2, db: 0 }, { at: 1.3, db: -60 }, { at: 2.5, db: -60 }])], loop_region: null } }));
      const envd = path.join(OUT_DIR, "strip-env.wav");
      const r6 = expectOk(send({ op: "export_song", path: envd, seconds: 3, rhythm: false, drone: false, pad: false, ambient: false, target_lufs: null }));
      const envStem = r6.stems.find((p) => /clips/i.test(path.basename(p)));
      const envHead = channelRms(envStem, 0.2, 1.1)[0];
      const envTail = channelRms(envStem, 1.5, 2.4)[0];
      if (!(envHead > 0.01 && envTail < envHead * 0.05)) throw new Error(`envelope: head ${envHead.toFixed(4)} tail ${envTail.toFixed(4)}`);
      // And the loudness target: the same track normalised to −14 LUFS lands within 1 LU.
      const loud = path.join(OUT_DIR, "strip-loud.wav");
      const r5 = expectOk(send({ op: "export_song", path: loud, seconds: 3, rhythm: false, drone: false, pad: false, ambient: false, target_lufs: -14 }));
      if (typeof r5.lufs_after !== "number" || Math.abs(r5.lufs_after + 14) > 1.0) throw new Error(`normalised to ${r5.lufs_after} LUFS (from ${r5.lufs}, gain ${r5.gain_db})`);
      return `L/R ${(l / Math.max(rr, 1e-9)).toFixed(0)}×, reverb tail ${(20 * Math.log10(tail)).toFixed(0)} dBFS, comp peak ${compPeak.toFixed(2)} vs ${dryPeak.toFixed(2)}, envelope tail ${(envTail / envHead * 100).toFixed(1)} %, ${r5.lufs.toFixed(1)} → ${r5.lufs_after.toFixed(1)} LUFS`;
    } finally {
      send({ op: "set_arrangement", arrangement: { ...arr, tracks: [], loop_region: null } });
    }
  });
  await step("editor: cut / undo / redo / paste / silence / jump, then save as a new take", () => {
    const src = path.join(OUT_DIR, "rhythm-four.wav");
    const imp = expectOk(send({ op: "import_audio", path: src }));
    try {
      const st0 = expectOk(send({ op: "edit_open", id: imp.take_id }));
      const len = st0.seconds;
      if (!(len > 2)) throw new Error(`take too short (${len} s)`);
      const near = (x, y, what) => {
        if (Math.abs(x - y) > 0.01) throw new Error(`${what}: ${x.toFixed(3)} vs ${y.toFixed(3)}`);
      };
      const st1 = expectOk(send({ op: "edit", edit: { op: "cut", a: len / 2 - 0.5, b: len / 2 + 0.5 } }));
      near(st1.seconds, len - 1, "cut 1 s");
      if (Math.abs(st1.clipboard_seconds - 1) > 0.01) throw new Error("clipboard");
      near(expectOk(send({ op: "edit_undo" })).seconds, len, "undo");
      near(expectOk(send({ op: "edit_redo" })).seconds, len - 1, "redo");
      near(expectOk(send({ op: "edit", edit: { op: "paste", at: 0 } })).seconds, len, "paste");
      const st5 = expectOk(send({ op: "edit", edit: { op: "silence", a: 0, b: 0.5 } }));
      const quiet = expectOk(send({ op: "edit_peaks", bins: 50, from_s: 0, to_s: 0.5 }));
      if (quiet.peak > 1e-6) throw new Error(`silence peak ${quiet.peak}`);
      const loud = expectOk(send({ op: "edit_peaks", bins: 50, from_s: 0.5, to_s: 1.5 }));
      if (!(loud.peak > 0.01)) throw new Error("the pasted audio is gone");
      if (st5.versions.length !== 4 || st5.index !== 3 || !st5.dirty) throw new Error(`history ${JSON.stringify(st5.versions)}`);
      // A7 (E2 + E3): the limiter caps the peak, the compressor squeezes it, the EQ keeps the length.
      const pk0 = st5.peak_dbfs;
      const lim = expectOk(send({ op: "edit", edit: { op: "limit", a: 0, b: len, ceiling_db: -12 } }));
      if (!(lim.peak_dbfs <= -11.9)) throw new Error(`limiter: peak ${lim.peak_dbfs} dBFS (was ${pk0})`);
      const cmp = expectOk(send({ op: "edit", edit: { op: "compress", a: 0, b: len, threshold_db: -40, ratio: 20, attack_ms: 0.1, release_ms: 200, makeup_db: 0 } }));
      if (!(cmp.peak_dbfs < lim.peak_dbfs - 6)) throw new Error(`compressor: ${lim.peak_dbfs} → ${cmp.peak_dbfs} dBFS`);
      const eqd = expectOk(send({ op: "edit", edit: { op: "eq", a: 0, b: len, bands: [{ kind: "low_shelf", hz: 200, gain_db: -12, q: 1 }, { kind: "peak", hz: 1000, gain_db: 3, q: 1 }, { kind: "high_shelf", hz: 4000, gain_db: 0, q: 1 }] } }));
      near(eqd.seconds, len, "eq keeps the length");
      if (eqd.versions.length !== 7) throw new Error(`history after effects: ${eqd.versions.length}`);
      // A5: a noise profile from the silent head (0..0.5 s) and a denoise pass keep the length and never raise the peak.
      const prof = expectOk(send({ op: "edit", edit: { op: "noise_profile", a: 0, b: 0.5 } }));
      if (!prof.noise_profile) throw new Error("no noise profile after taking one");
      const dn = expectOk(send({ op: "edit", edit: { op: "denoise", a: 0, b: len, reduction_db: 12, sensitivity: 1.5 } }));
      near(dn.seconds, len, "denoise keeps the length");
      if (dn.peak_dbfs > eqd.peak_dbfs + 0.5) throw new Error(`denoise raised the peak ${eqd.peak_dbfs} → ${dn.peak_dbfs}`);
      // A5 repairs round-trip: declip / de-ess / declick keep the length and never raise the peak.
      let last = dn;
      for (const edit of [{ op: "declip", a: 0, b: len, threshold: 0.95 }, { op: "deess", a: 0, b: len, threshold_db: -24, amount_db: 8 }, { op: "declick", a: 0, b: len, sensitivity: 8 }]) {
        const r = expectOk(send({ op: "edit", edit }));
        near(r.seconds, len, `${edit.op} keeps the length`);
        if (r.peak_dbfs > last.peak_dbfs + 0.5) throw new Error(`${edit.op} raised the peak ${last.peak_dbfs} → ${r.peak_dbfs}`);
        last = r;
      }
      // A6: stretching the first second by 1.5 adds 0.5 s; a pitch shift keeps the length.
      const stretched = expectOk(send({ op: "edit", edit: { op: "stretch", a: 0, b: 1, factor: 1.5 } }));
      near(stretched.seconds, len + 0.5, "stretch ×1.5 of 1 s");
      const pitched = expectOk(send({ op: "edit", edit: { op: "pitch", a: 0, b: len + 0.5, semitones: 2 } }));
      near(pitched.seconds, len + 0.5, "pitch keeps the length");
      // A7 second slice: echo and reverb keep the length.
      near(expectOk(send({ op: "edit", edit: { op: "echo", a: 0, b: len + 0.5, delay_s: 0.2, feedback: 0.3, mix: 0.3 } })).seconds, len + 0.5, "echo keeps the length");
      near(expectOk(send({ op: "edit", edit: { op: "reverb", a: 0, b: len + 0.5, size: 0.5, damp: 0.4, mix: 0.3 } })).seconds, len + 0.5, "reverb keeps the length");
      // A11: the silenced head is found as a silence; the spectrum has a peak; contrast head vs. body.
      const sil = expectOk(send({ op: "edit_silences", threshold_db: -60, min_s: 0.2 })).silences;
      if (!sil.length || sil[0][0] > 0.05 || sil[0][1] < 0.4) throw new Error(`silences ${JSON.stringify(sil)}`);
      const spec = expectOk(send({ op: "edit_spectrum", a: 1, b: len + 0.5, bands: 32 }));
      if (!(spec.peak_db > -60)) throw new Error(`spectrum peak ${spec.peak_db}`);
      const ctr = expectOk(send({ op: "edit_contrast", fg_a: 1, fg_b: len + 0.5, bg_a: 0, bg_b: 0.4 }));
      if (!(ctr.difference_db > 20)) throw new Error(`contrast ${ctr.difference_db} dB`);
      // A12: a macro of two ops adds two history steps and keeps the length.
      const before = expectOk(send({ op: "edit_status" }));
      const mac = expectOk(send({ op: "edit_macro", ops: [{ op: "gain", a: 0, b: 1, db: -3 }, { op: "fade_out", a: 0, b: 1 }] }));
      if (mac.versions.length !== before.versions.length + 2) throw new Error(`macro steps ${before.versions.length} → ${mac.versions.length}`);
      near(mac.seconds, before.seconds, "macro keeps the length");
      // A9: Audacity Nyquist plug-ins — the bundled gain plug-in lowers the peak of [1, 2) by 6 dB,
      // the bundled analyze plug-in reports dBFS, the tremolo keeps the length.
      const ny = expectOk(send({ op: "ny_list" }));
      const gain = ny.plugins.find((p) => p.id === "bundled:gain");
      const measure = ny.plugins.find((p) => p.id === "bundled:measure");
      if (!gain || !measure || gain.controls.length !== 1 || gain.controls[0].var !== "gain") throw new Error(`bundled plug-ins ${JSON.stringify(ny.plugins.map((p) => p.id))}`);
      const peakBefore = expectOk(send({ op: "edit_spectrum", a: 1, b: 2, bands: 16 })).peak_db;
      const nyst = expectOk(send({ op: "edit", edit: { op: "nyquist", a: 1, b: 2, plugin: "bundled:gain", values: [{ var: "gain", value: -6 }] } }));
      near(nyst.seconds, mac.seconds, "nyquist gain keeps the length");
      const peakAfter = expectOk(send({ op: "edit_spectrum", a: 1, b: 2, bands: 16 })).peak_db;
      if (!(peakBefore - peakAfter > 4.5 && peakBefore - peakAfter < 7.5)) throw new Error(`nyquist gain: peak ${peakBefore.toFixed(1)} → ${peakAfter.toFixed(1)} dB`);
      const an = expectOk(send({ op: "ny_analyze", plugin: "bundled:measure", a: 1, b: 2 }));
      if (!an.message || !an.message.includes("dBFS")) throw new Error(`nyquist analyze: ${JSON.stringify(an)}`);
      const trem = expectOk(send({ op: "edit", edit: { op: "nyquist", a: 1, b: 2, plugin: "bundled:tremolo", values: [{ var: "depth", value: 50 }, { var: "rate", value: 6 }, { var: "shape", value: 1 }] } }));
      near(trem.seconds, mac.seconds, "nyquist tremolo keeps the length");
      if (trem.versions.length !== mac.versions.length + 2) throw new Error(`nyquist steps ${mac.versions.length} → ${trem.versions.length}`);
      // A generate plug-in at the cursor grows the take; the result goes out as a WAV sample.
      const gen = expectOk(send({ op: "edit", edit: { op: "nyquist", a: 0.5, b: 0.5, plugin: "bundled:tremolo", values: [] } }));
      if (!(gen.seconds > trem.seconds - 1e-6)) throw new Error(`generate did not grow the take: ${trem.seconds} → ${gen.seconds}`);
      const wav = expectOk(send({ op: "edit_export_wav", a: 1, b: 2, name: "smoke nyquist" }));
      if (!fs.existsSync(wav.path) || !/Samples[\\/]Generated/.test(wav.path)) throw new Error(`export wav ${JSON.stringify(wav)}`);
      near(wav.seconds, 1, "exported sample length");
      fs.rmSync(wav.path, { force: true });
      near(expectOk(send({ op: "edit_jump", step: 1 })).seconds, len - 1, "jump to step 1");
      const saved = expectOk(send({ op: "edit_save", label: "smoke edit" }));
      const tk = saved.project.takes.find((x) => x.id === saved.take_id);
      if (!tk || tk.derived_from !== imp.take_id) throw new Error("the saved take is not derived from the original");
      near(tk.duration_seconds, len - 1, "saved length");
      const orig = saved.project.takes.find((x) => x.id === imp.take_id);
      near(orig.duration_seconds, len, "the original is untouched");
      return `${len.toFixed(2)} s → cut → ${(len - 1).toFixed(2)} s; 4 steps; saved ${tk.label}`;
    } finally {
      send({ op: "edit_close" });
    }
  });
  await step("export: FLAC 24-bit mix and stems (magic bytes, smaller than WAV)", () => {
    const out = path.join(OUT_DIR, "song-flac.flac");
    const r = expectOk(send({ op: "export_song", path: out, seconds: 4, format: "flac24", rhythm: false, drone: false, pad: false, ambient: true, target_lufs: null }));
    if (!/\.flac$/i.test(r.path) || !fs.existsSync(r.path)) throw new Error(`no flac at ${r.path}`);
    const head = fs.readFileSync(r.path).subarray(0, 4).toString("ascii");
    if (head !== "fLaC") throw new Error(`magic ${JSON.stringify(head)}`);
    const stem = r.stems.find((p) => /\.flac$/i.test(p));
    if (!stem || fs.readFileSync(stem).subarray(0, 4).toString("ascii") !== "fLaC") throw new Error("stems are not FLAC");
    const wav = path.join(OUT_DIR, "song-pcm24.wav");
    const w = expectOk(send({ op: "export_song", path: wav, seconds: 4, format: "pcm24", rhythm: false, drone: false, pad: false, ambient: true, target_lufs: null, stems: false }));
    if (!(r.bytes < w.bytes * 0.9)) throw new Error(`flac ${r.bytes} B vs wav ${w.bytes} B`);
    return `${(r.bytes / 1024).toFixed(0)} kB FLAC vs ${(w.bytes / 1024).toFixed(0)} kB WAV 24, ${r.stems.length} stems`;
  });
  await step("export: one file per song-map section (A10 multiple export)", () => {
    const markers = readState().project?.markers ?? [];
    if (markers.length === 0) throw new Error("the smoke project has no markers");
    const dir = path.join(OUT_DIR, "sections");
    fs.mkdirSync(dir, { recursive: true });
    const r = expectOk(send({ op: "export_sections", dir, seconds: 6, rhythm: false, drone: false, pad: false, ambient: true, target_lufs: null, format: "pcm16" }));
    if (!r.files.length) throw new Error("no section files");
    for (const f of r.files) if (!fs.existsSync(f)) throw new Error(`missing ${f}`);
    const first = wavStats(r.files[0]);
    return `${r.files.length} files, first ${first.seconds.toFixed(2)} s`;
  });
  await step("what next: the header hint follows the path (takes → background → export)", () => {
    const n = expectOk(send({ op: "what_next" }));
    if (!n.key || !n.screen || !n.text) throw new Error(JSON.stringify(n));
    return `${n.key} → ${n.screen}`;
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
  await step("template: a new project from the liquid DnB style has the progression, the world and the layers set", () => {
    const tname = `smoke-template-${Date.now().toString(36)}`;
    const r = expectOk(send({ op: "create_project", name: tname, style: "liquid_dnb" }));
    try {
      if (r.style !== "liquid_dnb") throw new Error(JSON.stringify(r));
      const st = expectOk(send({ op: "state" }));
      const genre = st.prefs?.progression?.genre;
      const world = st.scapes_status?.world ?? st.scapes?.tags?.find?.((x) => x === "cyber-rain");
      if (genre !== "liquid") throw new Error(`progression genre ${genre}`);
      if (!st.prefs?.rhythm?.on && !st.prefs?.drone) throw new Error("no background prefs");
      return `genre ${genre}, world ${world ?? "(journey built)"}`;
    } finally {
      expectOk(send({ op: "open_project", path: PROJECT }));
      // The template project was only a check: remove it again.
      const dir = path.dirname(PROJECT);
      for (const d of fs.readdirSync(dir)) if (d.startsWith("smoke-template-")) fs.rmSync(path.join(dir, d), { recursive: true, force: true });
    }
  });
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
