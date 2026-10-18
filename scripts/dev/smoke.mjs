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
  const out = execFileSync(process.execPath, [EF, "send", ...list.map((c) => JSON.stringify(c))], { encoding: "utf8", timeout: 120_000 });
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
const audible = (p, min = 0.02) => {
  const s = wavStats(p);
  if (!(s.peak > min && s.peak <= 1.0)) throw new Error(`${path.basename(p)} peak ${s.peak.toFixed(3)}`);
  return `${path.basename(p)} ${s.seconds.toFixed(1)} s peak ${s.peak.toFixed(2)}`;
};

const SCREENS = ["home", "play", "compose", "studio", "ambient", "soundscapes", "rhythm", "library", "instrument", "songmap", "samples", "references", "start", "help"];

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
  await step("export: whole song offline with stems", () => {
    const out = path.join(OUT_DIR, "song.wav");
    const r = expectOk(send({ op: "export_song", path: out, seconds: 6 }));
    if (!fs.existsSync(r.path)) throw new Error("no file");
    return `${path.basename(r.path)} ${wavStats(r.path).seconds.toFixed(1)} s, stems ${r.stems.length}`;
  });
  if (SOUND) {
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
