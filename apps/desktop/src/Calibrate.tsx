import { useEffect, useState } from "react";
import { api, describeError, type ProfileView } from "./api";
import { noteParts } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

interface Props {
  profileId: string;
  onClose: (changed: boolean) => void;
}

const CALIBRATION_SECONDS = 3.5;

/** "Meet my instrument": measure each note of the range from a held tone. */
export function Calibrate({ profileId, onClose }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [p, setP] = useState<ProfileView | null>(null);
  const [idx, setIdx] = useState(0);
  const [listening, setListening] = useState(false);
  const [countdown, setCountdown] = useState(0);
  const [last, setLast] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [changed, setChanged] = useState(false);

  useEffect(() => {
    api.getProfile(profileId).then((v) => {
      setP(v);
      const first = v.range.findIndex((r) => !r.measured);
      setIdx(first >= 0 ? first : 0);
    }).catch((e) => setError(describeError(e)));
  }, [profileId]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !listening) {
        e.stopPropagation();
        onClose(changed);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose, changed, listening]);

  if (!p) return null;
  const cur = p.range[idx];
  const name = (m: number) => {
    const n = noteParts(m);
    return `${t.noteNames[n.pc]}${n.octave}`;
  };
  const measuredNote = cur ? p.notes.find((n) => n.midi === cur.midi) : undefined;

  const measure = async () => {
    if (!cur) return;
    setError(null);
    setLast(null);
    setListening(true);
    setCountdown(CALIBRATION_SECONDS);
    const timer = setInterval(() => setCountdown((c) => Math.max(0, c - 0.1)), 100);
    try {
      const r = await api.calibrateNote(profileId, cur.midi, s.prefs.selectedInput, s.prefs.sampleRate);
      setP(r.profile);
      setChanged(true);
      const heardName = name(r.detected_midi);
      const same = r.detected_midi === cur.midi;
      setLast(
        `${t.detectedAs}: ${heardName} ${r.detected_cents >= 0 ? "+" : ""}${r.detected_cents.toFixed(0)} c` +
          (same ? "" : ` (≠ ${name(cur.midi)})`) +
          ` · ${t.heldFor} ${r.held_seconds.toFixed(1)} s · ${t.stability} ±${r.note.stability_cents.toFixed(1)} c`,
      );
      if (same && idx + 1 < p.range.length) setIdx(idx + 1);
    } catch (e) {
      const d = describeError(e);
      setError(d.startsWith("no_held_tone") ? t.noHeldTone : d);
    } finally {
      clearInterval(timer);
      setListening(false);
      setCountdown(0);
    }
  };

  const remove = async () => {
    if (!cur) return;
    try {
      setP(await api.removeProfileNote(profileId, cur.midi));
      setChanged(true);
    } catch (e) {
      setError(describeError(e));
    }
  };

  const play = async (exact: boolean) => {
    if (!cur) return;
    try {
      await api.playProfileNote(profileId, cur.midi, s.prefs.selectedOutput, exact);
    } catch (e) {
      setError(describeError(e));
    }
  };

  const d = p.diagnosis;
  const diagnosisText = d
    ? Math.abs(d.mean_offset_cents) > 10 && d.spread_cents < 10
      ? t.diagnosisSystematic
      : d.spread_cents > 15
        ? t.diagnosisRandom
        : t.diagnosisOk
    : null;

  return (
    <div className="modal-backdrop" role="presentation">
      <div className="modal calibrate" role="dialog" aria-modal="true" aria-labelledby="cal-title">
        <h2 id="cal-title">
          {t.meetInstrument}: {p.name} ({t.instrumentTypes[p.instrument]}{p.tuning ? `, ${p.tuning}` : ""})
        </h2>
        <p className="muted small">{t.meetIntro}</p>

        <div className="cal-notes" role="list">
          {p.range.map((r, i) => (
            <button
              key={r.midi}
              role="listitem"
              className={`cal-note ${r.measured ? "measured" : ""} ${i === idx ? "current" : ""}`}
              onClick={() => setIdx(i)}
              disabled={listening}
              title={r.measured ? t.measured : t.notMeasured}
            >
              {name(r.midi)}
            </button>
          ))}
        </div>

        {cur && (
          <div className="cal-current">
            <div className="cal-target">
              <span className="tuner-name">{name(cur.midi)}</span>
              {measuredNote && (
                <span className="small mono-num muted">
                  {measuredNote.measured_hz.toFixed(1)} Hz · {t.offset} {measuredNote.cents_offset >= 0 ? "+" : ""}
                  {measuredNote.cents_offset.toFixed(0)} c · {t.stability} ±{measuredNote.stability_cents.toFixed(1)} c
                </span>
              )}
            </div>
            <div className="modal-actions cal-actions">
              <button className="ghost small" onClick={() => setIdx(Math.max(0, idx - 1))} disabled={listening || idx === 0}>
                ← {t.prevNote}
              </button>
              <button className={listening ? "danger" : "rec"} onClick={measure} disabled={listening}>
                {listening ? `${t.listening} ${countdown.toFixed(1)} s` : measuredNote ? `● ${t.remeasure}` : `● ${t.playAndHold}`}
              </button>
              {measuredNote?.sample && (
                <button className="ghost small" onClick={() => play(true)} disabled={listening}>
                  ♪ {t.playSample}
                </button>
              )}
              {measuredNote && (
                <button className="ghost small" onClick={remove} disabled={listening}>
                  {t.delete}
                </button>
              )}
              <button className="ghost small" onClick={() => setIdx(Math.min(p.range.length - 1, idx + 1))} disabled={listening || idx + 1 >= p.range.length}>
                {t.skip} →
              </button>
            </div>
            {last && <div className="banner ok">{last}</div>}
            {error && (
              <div className="banner error" role="alert">
                {error}
              </div>
            )}
          </div>
        )}

        {d && (
          <div className="small muted cal-diag">
            {t.diagnosis}: {d.notes} {t.notes} · {t.meanOffset} {d.mean_offset_cents >= 0 ? "+" : ""}
            {d.mean_offset_cents.toFixed(1)} c · {t.spread} ±{d.spread_cents.toFixed(1)} c · {t.stability} ±{d.mean_stability_cents.toFixed(1)} c
            <br />
            {diagnosisText}
          </div>
        )}

        <div className="modal-actions">
          <button onClick={() => onClose(changed)} disabled={listening}>
            {t.save}
          </button>
        </div>
      </div>
    </div>
  );
}
