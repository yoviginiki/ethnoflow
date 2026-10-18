import { useEffect, useState } from "react";
import { api, describeError, type LiveDecision, type LiveStatus, type LiveView, type Locks, type Strategy } from "./api";
import { strategyLabel } from "./HarmonyPanel";
import { noteParts } from "./format";
import { messages, type Messages } from "./i18n";
import { useStore } from "./store";

const POLL_MS = 150;

/**
 * Live accompanist (F8.1): mounted once in App. While a session runs, the follower's
 * decisions (chord tones + drone root, only at phrase boundaries) drive the pad and the
 * drone through the same `setFollow` path as the offline proposals; stopping releases
 * them. The audio graph never waits for this loop.
 */
export function useLiveFollow() {
  const running = useStore((s) => s.liveRunning);
  useEffect(() => {
    if (!running) return;
    let cancelled = false;
    const tick = async () => {
      if (cancelled) return;
      try {
        const v = await api.liveTick();
        if (cancelled) return;
        const st = useStore.getState();
        st.setLiveView(v);
        if (!v.running) {
          st.setLiveRunning(false);
          st.setFollow(null, null);
          return;
        }
        const last = [...v.decisions].reverse().find((d) => d.changed);
        if (last && !(v.status?.bypass ?? false)) {
          const notes = last.chord ? last.chord.map((t) => ({ midi: t.midi, cents: t.cents })) : null;
          st.setFollow(notes, last.drone_root);
        }
      } catch (e) {
        useStore.getState().setError(describeError(e));
      }
      if (!cancelled) setTimeout(tick, POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
    };
  }, [running]);
}

export async function startLive() {
  const st = useStore.getState();
  const h = st.prefs.harmony;
  try {
    st.setError(null);
    const v = await api.liveStart({
      intent: st.prefs.intent,
      harmony: { tension: h.tension, movement: h.movement, richness: h.richness, seed: h.seed, locks: st.prefs.intent.locks, register_low: h.registerLow, register_high: h.registerHigh },
      strategy: st.prefs.liveStrategy,
      device: st.prefs.selectedInput,
      sample_rate: st.prefs.sampleRate,
    });
    st.setLiveView(v);
    st.setLiveRunning(true);
  } catch (e) {
    st.setError(describeError(e));
  }
}

export async function stopLive() {
  const st = useStore.getState();
  st.setLiveRunning(false);
  st.setLiveView(null);
  st.setFollow(null, null);
  await api.liveStop().catch(() => undefined);
}

function holdText(t: Messages, s: LiveStatus): string {
  const h = s.last_hold;
  if (!h) return "";
  switch (h.kind) {
    case "low_confidence":
      return `${t.liveHoldLow} (${Math.round(h.context * 100)} % / ${Math.round(h.intent * 100)} %)`;
    case "locked":
      return t.liveHoldLocked;
    case "bypass":
      return t.liveHoldBypass;
    case "same_chord":
      return t.liveHoldSame;
    default:
      return t.liveHoldNoTones;
  }
}

function chordText(t: Messages, s: LiveStatus): string {
  if (!s.chord || s.chord.length === 0) return s.drone_root !== null ? `${t.liveDroneOnly} ${noteName(t, s.drone_root)}` : "—";
  const root = s.chord_root_pc !== null ? t.noteNames[s.chord_root_pc] : "";
  const kind = s.chord_kind ? (t.chordSuffix as Record<string, string>)[s.chord_kind] ?? "" : s.chord_shape ? ` ${(t.chordShapes as Record<string, string>)[s.chord_shape] ?? ""}` : "";
  return `${root}${kind} · ${s.chord.map((c) => noteName(t, c.midi)).join(" ")}`;
}

function noteName(t: Messages, midi: number) {
  const p = noteParts(midi);
  return `${t.noteNames[p.pc]}${p.octave}`;
}

/** Start/stop, mode, strategy, locks, bypass and what the follower hears and decides. */
export function LiveFollowPanel() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const view: LiveView | null = s.liveView;
  const st = view?.status ?? null;
  const [busy, setBusy] = useState(false);
  const locks = s.prefs.intent.locks;
  const setLocks = async (patch: Partial<Locks>) => {
    const next = { ...locks, ...patch };
    s.setPref("intent", { ...s.prefs.intent, locks: next });
    if (s.liveRunning) await api.liveSet({ locks: next, bypass: null }).then((v) => s.setLiveView(v)).catch((e) => s.setError(describeError(e)));
  };
  const toggleBypass = async () => {
    if (!s.liveRunning) return;
    const on = !(st?.bypass ?? false);
    await api
      .liveSet({ locks: null, bypass: on })
      .then((v) => {
        s.setLiveView(v);
        if (on) s.setFollow(null, null);
      })
      .catch((e) => s.setError(describeError(e)));
  };
  const lastDecision: LiveDecision | null = view?.decisions.length ? view.decisions[view.decisions.length - 1] : null;
  const mode = s.prefs.intent.enabled ? s.prefs.intent.mode : "off";
  return (
    <details className="panel live-follow" open>
      <summary>
        {t.liveTitle} · {s.liveRunning ? <span className="badge ok">{t.liveOn}</span> : <span className="badge">{t.liveOff}</span>}
      </summary>
      <div className="muted small">{t.liveHint}</div>
      <div className="analysis-row">
        <label className="inline small">
          {t.liveStrategy}
          <select value={s.prefs.liveStrategy} disabled={s.liveRunning} onChange={(e) => s.setPref("liveStrategy", e.target.value as Strategy)}>
            {(["drone_only", "modal_pads", "liquid_chords"] as Strategy[]).map((k) => (
              <option key={k} value={k}>
                {strategyLabel(t, k)}
              </option>
            ))}
          </select>
        </label>
        <span className="small muted">
          {t.liveMode}: <b>{mode === "off" ? t.intentOff : mode === "stable" ? t.intentStable : mode === "assisted" ? t.intentAssisted : t.intentCreative}</b> ({t.liveModeHint})
        </span>
        <span className="spacer" />
        {!s.liveRunning ? (
          <button
            disabled={busy || s.mode === "recording" || !s.project}
            onClick={async () => {
              setBusy(true);
              await startLive();
              setBusy(false);
            }}
          >
            ▶ {t.liveStart}
          </button>
        ) : (
          <button className="danger" onClick={() => void stopLive()}>
            ■ {t.liveStop}
          </button>
        )}
      </div>
      <div className="analysis-row small">
        <span className="muted">{t.liveLocks}:</span>
        <label className="inline">
          <input type="checkbox" checked={locks.tonic} onChange={(e) => void setLocks({ tonic: e.target.checked })} /> {t.lockTonic}
        </label>
        <label className="inline">
          <input type="checkbox" checked={locks.mode} onChange={(e) => void setLocks({ mode: e.target.checked })} /> {t.lockMode}
        </label>
        <label className="inline">
          <input type="checkbox" checked={locks.chord} onChange={(e) => void setLocks({ chord: e.target.checked })} /> {t.lockChord}
        </label>
        <label className="inline">
          <input type="checkbox" checked={locks.drone} onChange={(e) => void setLocks({ drone: e.target.checked })} /> {t.lockDrone}
        </label>
        <span className="spacer" />
        <button className={`ghost small ${st?.bypass ? "on" : ""}`} disabled={!s.liveRunning} onClick={() => void toggleBypass()} title={t.liveBypassHint}>
          {st?.bypass ? t.liveBypassOn : t.liveBypass}
        </button>
      </div>
      {st && (
        <div className="live-status">
          <div className="analysis-row small mono-num">
            <span>
              {t.liveContext}: <b>{st.tonic_pc !== null && st.kind ? `${t.noteNames[st.tonic_pc]} ${(t.scaleKinds as Record<string, string>)[st.kind] ?? st.kind}` : "—"}</b> {Math.round(st.context_confidence * 100)} %
            </span>
            <span>
              {t.liveHarmony}: <b>{chordText(t, st)}</b>
            </span>
            <span className="muted">
              {t.livePhrases} {st.phrases}
              {st.open_phrase_notes > 0 ? ` (+${st.open_phrase_notes})` : ""} · {t.liveChanges} {st.changes} · {t.liveHolds} {st.holds} · {t.liveSwitches} {st.switches}
            </span>
          </div>
          {st.last_hold && <div className="banner warnb small">{t.liveWhyHeld}: {holdText(t, st)}</div>}
          {lastDecision && lastDecision.changed && (
            <div className="muted small">
              {t.liveLastChange}: {t.livePhraseShort} {lastDecision.phrase + 1} @ {lastDecision.at_s.toFixed(1)} s
              {lastDecision.switched ? ` · ${t.liveSwitched}` : ""}
            </div>
          )}
          <div className="live-notes" aria-label={t.liveRecent}>
            {st.recent.map((n, i) => (
              <span key={i} className={`live-note ${n.class ?? ""}`} title={`${n.start.toFixed(2)}–${n.end.toFixed(2)} s · ${n.class ?? ""}`}>
                {noteName(t, n.midi)}
              </span>
            ))}
            {view && view.monitor_pitch_hz !== null && view.monitor_confidence >= 0.6 && (
              <span className="live-note now">{noteName(t, Math.round(12 * Math.log2(view.monitor_pitch_hz / (s.project?.a4_hz ?? 440)) + 69))}</span>
            )}
          </div>
          {st.alternatives.length > 1 && (
            <div className="muted small">
              {t.liveAlternatives}: {st.alternatives.slice(0, 3).map((h) => `${t.noteNames[h.tonic_pc]} ${(t.scaleKinds as Record<string, string>)[h.kind] ?? h.kind} ${Math.round(h.probability * 100)} %`).join(" · ")}
            </div>
          )}
        </div>
      )}
    </details>
  );
}
