import { useEffect } from "react";
import { api, describeError, NOTE_CLASSES, type IntentMode, type IntentSettings, type NoteClass, type PerformanceAnalysis } from "./api";
import { messages, type Messages } from "./i18n";
import { DEFAULT_INTENT, useStore } from "./store";

const MODES: IntentMode[] = ["stable", "assisted", "creative"];

export function modeLabel(t: Messages, s: IntentSettings): string {
  if (!s.enabled) return t.intentOff;
  return s.mode === "stable" ? t.intentStable : s.mode === "creative" ? t.intentCreative : t.intentAssisted;
}

/** Always-visible badge of the active mode (owner decision 2026-10-17): off / Stable /
 *  Assisted / Creative, plus a quick on/off switch. Used in the trainer, the studio and
 *  Compose First. */
export function IntentBadge({ onOpen }: { onOpen?: () => void }) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const intent = s.prefs.intent ?? DEFAULT_INTENT;
  const cls = intent.enabled ? `intent-badge on ${intent.mode}` : "intent-badge off";
  return (
    <span className={cls} role="status" title={t.intentBadgeHint}>
      <label className="inline">
        <input type="checkbox" checked={intent.enabled} onChange={(e) => s.setPref("intent", { ...intent, enabled: e.target.checked })} />
        {t.intentShort}
      </label>
      <strong>{modeLabel(t, intent)}</strong>
      {onOpen && (
        <button className="ghost small" onClick={onOpen}>
          {t.intentSettings}
        </button>
      )}
    </span>
  );
}

/** Refresh the F2.11 analysis whenever the notes or the settings change. */
export function usePerformanceAnalysis() {
  const notes = useStore((s) => s.analysis?.notes ?? null);
  const scaleKey = useStore((s) => (s.project?.scale ? `${s.project.scale.tonic_pc}:${s.project.scale.kind}:${s.project.scale.degrees_cents.length}` : ""));
  const profileKey = useStore((s) => (s.project?.instrument ? `${s.project.instrument.id}:${s.project.instrument.version}` : ""));
  const intent = useStore((s) => s.prefs.intent ?? DEFAULT_INTENT);
  useEffect(() => {
    if (!notes) {
      useStore.getState().setPerformance(null);
      return;
    }
    let cancelled = false;
    api
      .performanceAnalysis(intent)
      .then((p) => !cancelled && useStore.getState().setPerformance(p))
      .catch((e) => useStore.getState().setError(describeError(e)));
    return () => {
      cancelled = true;
    };
  }, [notes, scaleKey, profileKey, intent]);
}

export function classLabel(t: Messages, c: NoteClass): string {
  return t.noteClasses[c];
}

/** Why the harmony stayed (or moved) after each phrase, in words. */
export function contextLine(t: Messages, p: PerformanceAnalysis, i: number): string {
  const c = p.contexts[i];
  const intent = p.intents[i];
  const name = `${t.noteNames[c.tonic_pc]} ${t.scaleKinds[c.kind]}`;
  const parts: string[] = [];
  if (c.switched) parts.push(t.whySwitched(name));
  else parts.push(t.whyStayed(name, Math.round(c.confidence * 100)));
  if (intent.probable_errors > 0 || intent.noise > 0) parts.push(t.whyIgnored(intent.probable_errors, intent.noise));
  if (intent.ornaments > 0) parts.push(t.whyOrnaments(intent.ornaments));
  if (c.evidence_for_change) {
    const e = c.evidence_for_change;
    const rival = `${t.noteNames[e.tonic_pc]} ${t.scaleKinds[e.kind]}`;
    parts.push(e.held_by_lock ? t.whyLocked(rival, Math.round(e.probability * 100)) : t.whyEvidence(rival, Math.round(e.probability * 100), e.consecutive_phrases));
  }
  return parts.join(" ");
}

/** Settings and the per-phrase explanation. */
export function IntentPanel() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const intent = s.prefs.intent ?? DEFAULT_INTENT;
  const perf = s.performance;
  const set = (patch: Partial<IntentSettings>) => s.setPref("intent", { ...intent, ...patch });
  const selected = s.selectedNote;
  const selectedClass = selected !== null ? perf?.classes.find((c) => c.index === selected) : undefined;
  const mark = (cls: NoteClass | null) => {
    if (selected === null) return;
    api
      .annotateNote(intent, selected, cls)
      .then((p) => s.setPerformance(p))
      .catch((e) => s.setError(describeError(e)));
  };

  return (
    <details className="panel intent" open>
      <summary>
        {t.intentTitle} · <IntentBadge />
      </summary>
      <div className="muted small">{t.intentHint}</div>
      <div className="analysis-row">
        {MODES.map((m) => (
          <label key={m} className="inline">
            <input type="radio" name="intentMode" checked={intent.mode === m} disabled={!intent.enabled} onChange={() => set({ mode: m })} />
            {m === "stable" ? t.intentStable : m === "assisted" ? t.intentAssisted : t.intentCreative}
          </label>
        ))}
      </div>
      <div className="drone-sliders">
        <label className="drone-slider">
          <span>{t.intentTolerance}</span>
          <input type="range" min={0} max={100} step={1} value={Math.round(intent.tolerance * 100)} disabled={!intent.enabled} onChange={(e) => set({ tolerance: Number(e.target.value) / 100 })} />
          <span className="mono-num small">{Math.round(intent.tolerance * 100)}</span>
        </label>
        <label className="drone-slider">
          <span>{t.intentSensitivity}</span>
          <input type="range" min={0} max={100} step={1} value={Math.round(intent.sensitivity * 100)} disabled={!intent.enabled} onChange={(e) => set({ sensitivity: Number(e.target.value) / 100 })} />
          <span className="mono-num small">{Math.round(intent.sensitivity * 100)}</span>
        </label>
      </div>
      <div className="analysis-row small">
        <span className="muted">{t.locks}:</span>
        {(["tonic", "mode", "chord", "drone"] as const).map((k) => (
          <label key={k} className="inline">
            <input type="checkbox" checked={intent.locks[k]} onChange={(e) => set({ locks: { ...intent.locks, [k]: e.target.checked } })} />
            {t.lockNames[k]}
          </label>
        ))}
      </div>
      {perf && (
        <>
          <div className="analysis-row small">
            <span className="muted">{t.intentSummary(perf.phrases.length, perf.switches)}</span>
            {NOTE_CLASSES.map((c) => {
              const n = perf.classes.filter((x) => x.class === c).length;
              return n > 0 ? (
                <span key={c} className={`class-chip ${c}`}>
                  {classLabel(t, c)} {n}
                </span>
              ) : null;
            })}
          </div>
          {selected !== null && selectedClass && (
            <div className="analysis-row small">
              <span>
                {t.noteSelected}: <span className={`class-chip ${selectedClass.class}`}>{classLabel(t, selectedClass.class)}</span> {Math.round(selectedClass.confidence * 100)} % ·{" "}
                {selectedClass.sustained ? t.deviationSustained : t.deviationBrief}
                {selectedClass.cents_from_scale !== null && ` · ${t.fromScale} ${selectedClass.cents_from_scale >= 0 ? "+" : ""}${selectedClass.cents_from_scale.toFixed(0)} ¢`}
                {selectedClass.cents_from_profile !== null && ` · ${t.fromProfile} ${selectedClass.cents_from_profile >= 0 ? "+" : ""}${selectedClass.cents_from_profile.toFixed(0)} ¢`}
                {" · "}
                {selectedClass.reasons.map((r) => t.reasons[r]).join(", ")}
              </span>
              <span className="muted">{t.markAs}:</span>
              {(["stable", "ornament", "probable_error"] as NoteClass[]).map((c) => (
                <button key={c} className="ghost small" onClick={() => mark(c)} disabled={selectedClass.reasons.includes("user_marked") && selectedClass.class === c}>
                  {classLabel(t, c)}
                </button>
              ))}
              {selectedClass.reasons.includes("user_marked") && (
                <button className="ghost small" onClick={() => mark(null)}>
                  {t.clearMark}
                </button>
              )}
            </div>
          )}
          <ol className="phrase-list small">
            {perf.phrases.map((ph, i) => (
              <li key={i} className={perf.contexts[i]?.switched ? "switched" : ""}>
                <span className="mono-num muted">
                  {ph.start.toFixed(1)}–{ph.end.toFixed(1)} s
                </span>{" "}
                {ph.breath_before && <span className="muted">{t.breath} · </span>}
                {contextLine(t, perf, i)}
              </li>
            ))}
          </ol>
        </>
      )}
    </details>
  );
}
