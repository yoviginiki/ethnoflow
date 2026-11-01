import { useState } from "react";
import { api, describeError } from "./api";
import { KIND_LABEL } from "./ambientPresets";
import { formatTime } from "./format";
import { messages } from "./i18n";
import { journeyWorld, phaseAt, PHASE_ROLES, PHASE_SHARE, WORLDS, worldById } from "./scapes";
import { startJourney, stopJourney } from "./scapesActions";
import { useStore } from "./store";

const MIN_MINUTES = 1;
const MAX_MINUTES = 60;
const DEFAULT_MINUTES = 8;

/**
 * Soundscapes (stage S1 of docs/design/SOUNDSCAPES_GENERATIVE_PLAN.md): pick a world, a
 * length and a seed, and the app composes a whole journey — Intro → Development →
 * Climax → Outro — as an atmosphere rack with scenes and breathing lanes, then plays it.
 * The result is an ordinary rack: Atmosphere (Layers › Program) edits it, presets keep it.
 */
export function ScapesScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [worldId, setWorldId] = useState<string>(journeyWorld(s.ambient) ?? WORLDS[0].id);
  const [minutes, setMinutes] = useState(DEFAULT_MINUTES);
  const [seed, setSeed] = useState(() => 1 + Math.floor(Math.random() * 1_000_000));
  const [loop, setLoop] = useState(false);
  const [busy, setBusy] = useState(false);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  const world = worldById(worldId) ?? WORLDS[0];
  const current = journeyWorld(s.ambient);
  const status = s.ambientStatus;
  const running = !!status?.program_playing && s.layers.ambient;
  const phase = current ? phaseAt(s.ambient, status?.program_position ?? 0) : null;
  const endS = phase?.end_s ?? 0;

  const play = async () => {
    setBusy(true);
    try {
      await startJourney(world, { minutes, seed, loop, play: true });
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };
  const stop = () => void stopJourney().catch(fail);
  const savePreset = async () => {
    if (!s.ambient || !current) return;
    try {
      await api.ambientPresetSave(`${t[worldById(current)!.title]} · ${minutes} min`, ["scapes", current], s.ambient);
      s.setMessage(t.scapesSaved);
    } catch (e) {
      fail(e);
    }
  };

  return (
    <main className="compose scapes">
      <section className="panel">
        <h2>{t.scapesTitle}</h2>
        <div className="muted small">{t.scapesIntro}</div>
      </section>

      <section className="panel">
        <h3>{t.scapesWorlds}</h3>
        <div className="scapes-worlds" role="radiogroup" aria-label={t.scapesWorlds}>
          {WORLDS.map((w) => (
            <button key={w.id} role="radio" aria-checked={w.id === worldId} className={`world-card ${w.id === worldId ? "on" : ""} ${w.id === current ? "current" : ""}`} onClick={() => setWorldId(w.id)} data-world={w.id}>
              <span className="world-title">{t[w.title]}</span>
              <span className="muted small">{t[w.desc]}</span>
              <span className="muted small">
                {t.scapesInside} {w.layers.map((l) => t[KIND_LABEL[l.source.kind]]).join(" · ")}
              </span>
              {w.id === current && <span className="badge ok">{t.scapesCurrent}</span>}
            </button>
          ))}
        </div>
      </section>

      <section className="panel analysis-row" data-coach="scapes-transport">
        <label className="inline">
          {t.scapesMinutes}
          <input type="number" min={MIN_MINUTES} max={MAX_MINUTES} value={minutes} onChange={(e) => setMinutes(Math.max(MIN_MINUTES, Math.min(MAX_MINUTES, Number(e.target.value) || DEFAULT_MINUTES)))} style={{ width: 64 }} />
        </label>
        <label className="inline">
          <input type="checkbox" checked={loop} onChange={(e) => setLoop(e.target.checked)} />
          {t.scapesLoop}
        </label>
        <label className="inline">
          {t.scapesSeed}
          <span className="mono-num">{seed}</span>
          <button className="ghost small" onClick={() => setSeed(1 + Math.floor(Math.random() * 1_000_000))} title={t.scapesShuffleHint}>
            🎲 {t.scapesShuffle}
          </button>
        </label>
        {running ? (
          <button className="primary" onClick={stop}>
            ■ {t.scapesStop}
          </button>
        ) : (
          <button className="primary" onClick={() => void play()} disabled={busy || s.pending !== null} title={t.scapesPlayHint}>
            ▶ {t.scapesPlay}
          </button>
        )}
        <button className="ghost" onClick={() => void savePreset()} disabled={!current}>
          💾 {t.scapesSave}
        </button>
      </section>

      <section className="panel">
        <h3>{t.scapesTimeline}</h3>
        {current && phase ? (
          <>
            <div className="journey-bar" role="progressbar" aria-valuemin={0} aria-valuemax={Math.round(endS)} aria-valuenow={Math.round(status?.program_position ?? 0)}>
              {PHASE_ROLES.map((role, i) => (
                <span key={role} className={`journey-seg ${phase.index === i ? "on" : ""}`} style={{ flexGrow: PHASE_SHARE[role] }}>
                  {s.ambient?.program.scenes[i]?.name ?? role}
                </span>
              ))}
              <span className="journey-cursor" style={{ left: `${endS > 0 ? (Math.min(status?.program_position ?? 0, endS) / endS) * 100 : 0}%` }} />
            </div>
            <div className="muted small">
              {t[worldById(current)!.title]} · {phase.name} · {formatTime(status?.program_position ?? 0)} / {formatTime(endS)} · {t.scapesRemaining} {formatTime(phase.remaining_s)}
              {!running && ` · ${t.scapesPaused}`}
            </div>
          </>
        ) : (
          <div className="muted small">{t.scapesNoJourney}</div>
        )}
        <div className="muted small">{t.scapesEditHint}</div>
      </section>
    </main>
  );
}
