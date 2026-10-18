#!/usr/bin/env node
// Drive the running EthnoFlow Studio from the terminal through the assistant bridge
// (docs/ASSISTANT_BRIDGE.md). The app must be running with the bridge on.
//
//   node scripts/assistant/ef.mjs state                       print state.json
//   node scripts/assistant/ef.mjs send '{"op":"ping"}' [...]  send commands, print replies
//   node scripts/assistant/ef.mjs send-file cmds.json         one command or an array of commands
//   node scripts/assistant/ef.mjs dir                         print the bridge folder
//
// Commands are written atomically (tmp + rename) so the app never reads half a file.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const dir = process.env.ETHNOFLOW_ASSISTANT_DIR ?? path.join(process.env.APPDATA ?? path.join(os.homedir(), "AppData", "Roaming"), "eu.ensodo.ethnoflow", "assistant");
const [mode, ...rest] = process.argv.slice(2);

function readState() {
  const p = path.join(dir, "state.json");
  if (!fs.existsSync(p)) throw new Error(`no state yet at ${p} (is the app running with the bridge on?)`);
  return JSON.parse(fs.readFileSync(p, "utf8"));
}

async function send(cmds) {
  fs.mkdirSync(path.join(dir, "inbox"), { recursive: true });
  fs.mkdirSync(path.join(dir, "outbox"), { recursive: true });
  const replies = [];
  for (let i = 0; i < cmds.length; i++) {
    const name = `${Date.now()}-${String(i).padStart(2, "0")}-${Math.random().toString(36).slice(2, 7)}`;
    const tmp = path.join(dir, "inbox", `${name}.json.tmp`);
    fs.writeFileSync(tmp, JSON.stringify(cmds[i]));
    fs.renameSync(tmp, path.join(dir, "inbox", `${name}.json`));
    const out = path.join(dir, "outbox", `${name}.json`);
    const deadline = Date.now() + (Number(process.env.EF_TIMEOUT_MS) || 15_000);
    let reply = null;
    while (Date.now() < deadline) {
      if (fs.existsSync(out)) {
        try {
          reply = JSON.parse(fs.readFileSync(out, "utf8"));
          fs.unlinkSync(out);
          break;
        } catch {
          // being written
        }
      }
      await new Promise((r) => setTimeout(r, 100));
    }
    replies.push(reply ?? { ok: false, error: "timeout: no reply (app not running, bridge off, or no project open; EF_TIMEOUT_MS raises the wait)" });
  }
  return replies;
}

try {
  if (mode === "dir") {
    console.log(dir);
  } else if (mode === "state") {
    console.log(JSON.stringify(readState(), null, 2));
  } else if (mode === "send") {
    const cmds = rest.map((a) => JSON.parse(a));
    console.log(JSON.stringify(await send(cmds), null, 2));
  } else if (mode === "send-file") {
    const v = JSON.parse(fs.readFileSync(rest[0], "utf8"));
    console.log(JSON.stringify(await send(Array.isArray(v) ? v : [v]), null, 2));
  } else {
    console.error("usage: ef.mjs state | send <json>... | send-file <file> | dir");
    process.exit(2);
  }
} catch (e) {
  console.error(String(e?.message ?? e));
  process.exit(1);
}
