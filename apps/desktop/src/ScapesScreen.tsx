import { useEffect, useRef, useState } from "react";
import { Ambient } from "./Ambient";
import { Fold } from "./Fold";
import { SamplePicker } from "./SamplePicker";
import { SceneMatrix } from "./SceneMatrix";
import { Soundscapes } from "./Soundscapes";
import { recordChange } from "./rackHistory";
import { api, describeError, type PresetSummary } from "./api";
import { KIND_LABEL } from "./ambientPresets";
import { formatTime } from "./format";
import { messages } from "./i18n";
import { journeyWorld, phaseAt, PHASE_ROLES, PHASE_SHARE, WORLDS, worldById } from "./scapes";
import { seekJourney, startJourney, stopJourney } from "./scapesActions";
import { useStore } from "./store";

const MIN_MINUTES = 1;
const MAX_MINUTES = 60;
const DEFAULT_MINUTES = 8;
const STATUS_POLL_MS = 250;

/**
 * Soundscapes (stage S1 of docs/design/SOUNDSCAPES_GENERATIVE_PLAN.md): pick a world, a
 * length and a seed, and the app composes a whole journey — Intro → Development →
 * Climax → Outro — as an atmosphere rack with scenes and breathing lanes, then plays it.
 * The result is an ordinary rack: Atmosphere (Layers › Program) edits it, presets keep it.
 */
export function ScapesScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [worldId, setWorldId] = useState<string>(journeyWorld(s.scapes) ?? WORLDS[0].id);
  const [minutes, setMinutes] = useState(DEFAULT_MINUTES);
  const [seed, setSeed] = useState(() => 1 + Math.floor(Math.random() * 1_000_000));
  const [loop, setLoop] = useState(false);
  const [busy, setBusy] = useState(false);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  const world = worldById(worldId) ?? WORLDS[0];
  const current = journeyWorld(s.scapes);
  const status = s.scapesStatus;
  const on = s.layers.scapes;
  const running = !!status?.program_playing && on;
  const phase = current ? phaseAt(s.scapes, status?.program_position ?? 0) : null;
  const endS = phase?.end_s ?? 0;
  const barRef = useRef<HTMLDivElement>(null);
  // My worlds: atmosphere presets tagged "scapes" (saved journeys with their scenes).
  const [mine, setMine] = useState<PresetSummary[]>([]);
  const [worldName, setWorldName] = useState("");
  const refreshMine = () =>
    api
      .ambientPresets()
      .then((list) => setMine(list.filter((p) => p.tags.includes("scapes") && !p.factory)))
      .catch(() => undefined);
  useEffect(() => {
    void refreshMine();
  }, []);
  const loadMine = async (p: PresetSummary) => {
    if (!s.scapes) return;
    setBusy(true);
    try {
      const rack = await api.ambientPresetLoad(p.id);
      const v = await api.setScapes({ ...rack, schema_version: s.scapes.schema_version });
      recordChange("scapes", s.scapes, v.ambient, t.hJourney(p.name), t.hStart);
      useStore.getState().setScapes(v.ambient);
      s.setMessage(`${t.scapesLoaded}: ${p.name}`);
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };
  const saveMine = async () => {
    if (!s.scapes) return;
    const name = worldName.trim() || `${current ? t[worldById(current)!.title] : t.scapesTitle} · ${new Date().toLocaleDateString()}`;
    try {
      await api.ambientPresetSave(name, ["scapes", ...(current ? [current] : [])], s.scapes);
      setWorldName("");
      await refreshMine();
      s.setMessage(`${t.scapesSaved}: ${name}`);
    } catch (e) {
      fail(e);
    }
  };
  const deleteMine = async (p: PresetSummary) => {
    try {
      await api.ambientPresetDelete(p.id);
      await refreshMine();
    } catch (e) {
      fail(e);
    }
  };

  // Live position while the rack runs (the engine's clock moves it; the UI only reads).
  useEffect(() => {
    if (!on) return;
    let cancelled = false;
    const tick = async () => {
      if (cancelled) return;
      try {
        const st = await api.scapesStatus();
        if (!cancelled) useStore.getState().setScapesStatus(st);
      } catch {
        return;
      }
      setTimeout(tick, STATUS_POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
    };
  }, [on]);

  const seekTo = (e: React.MouseEvent<HTMLDivElement>) => {
    const el = barRef.current;
    if (!el || endS <= 0 || !on) return;
    const r = el.getBoundingClientRect();
    const frac = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    void seekJourney(null, frac * endS).catch(fail);
  };

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

  return (
    <main className="compose scapes">
      <section className="panel">
        <h2>{t.scapesTitle}</h2>
        <div className="muted small">{t.scapesIntro}</div>
        <div className="muted small">{t.scapesOwnRack}</div>
      </section>

      {/* The studio (owner 1.11 evening): samples on the left to hear and add, the journey's
          modules with knobs on the right, the scenes table below; the full rack editor folded. */}
      <section className="panel" data-coach="scapes-studio">
        <h3>{t.scapesStudio}</h3>
        <div className="muted small">{t.scapesStudioHint}</div>
      </section>
      <div className="scapes-studio">
        <SamplePicker kind="scapes" />
        <div className="scapes-modules">
          <Soundscapes kind="scapes" />
        </div>
      </div>
      <section className="panel">
        <h3>{t.scapesWorlds}</h3>
        <div className="scapes-worlds chips" role="radiogroup" aria-label={t.scapesWorlds}>
          {WORLDS.map((w) => (
            <button key={w.id} role="radio" aria-checked={w.id === worldId} className={`world-chip ${w.id === worldId ? "on" : ""} ${w.id === current ? "current" : ""}`} onClick={() => setWorldId(w.id)} data-world={w.id} title={`${t[w.desc]} — ${t.scapesInside} ${w.layers.map((l) => t[KIND_LABEL[l.source.kind]]).join(" · ")}`}>
            {t[w.title]}
            {w.id === current && " ●"}
          </button>
          ))}
          {mine.map((p) => (
            <span key={p.id} className="world-chip mine" data-world={p.id} title={`${p.layer_count} ${t.layersWord}`}>
              <button className="ghost small" onClick={() => void loadMine(p)} disabled={busy || !s.scapes} title={t.scapesLoadWorld}>
                ★ {p.name}
              </button>
              <button className="ghost small" onClick={() => void deleteMine(p)} title={t.delete} aria-label={t.delete}>
                🗑
              </button>
            </span>
          ))}
        </div>
        <div className="analysis-row">
          <input value={worldName} onChange={(e) => setWorldName(e.target.value)} placeholder={t.scapesWorldName} maxLength={60} aria-label={t.scapesWorldName} />
          <button onClick={() => void saveMine()} disabled={!s.scapes || s.scapes.layers.length === 0} title={t.scapesSaveWorldHint}>
            ★ {t.scapesSaveWorld}
          </button>
          <span className="muted small">{t.scapesMyWorldsHint}</span>
        </div>
      </section>

      <section className="panel scapes-transport" data-coach="scapes-transport">
        <div className="analysis-row">
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
        {on ? (
          <button className="primary" onClick={stop}>
            ■ {t.scapesStop}
          </button>
        ) : (
          <button className="primary" onClick={() => void play()} disabled={busy || s.pending !== null} title={t.scapesPlayHint}>
            ▶ {t.scapesPlay}
          </button>
        )}
        </div>
      </section>

      <section className="panel">
        <h3>{t.scapesTimeline}</h3>
        {current && phase ? (
          <>
            <div className="analysis-row">
              <button className="ghost" disabled={!on} onClick={() => void seekJourney(!running, null).catch(fail)}>
                {running ? `❚❚ ${t.scapesPause}` : `▶ ${t.scapesResume}`}
              </button>
              <button className="ghost" disabled={!on} onClick={() => void seekJourney(null, 0).catch(fail)}>
                ⏮ {t.scapesToStart}
              </button>
              <span className="muted small">{t.scapesSeekHint}</span>
            </div>
            <div ref={barRef} className={`journey-bar ${on ? "seekable" : ""}`} role="progressbar" aria-valuemin={0} aria-valuemax={Math.round(endS)} aria-valuenow={Math.round(status?.program_position ?? 0)} onClick={seekTo} title={t.scapesSeekHint}>
              {PHASE_ROLES.map((role, i) => (
                <span key={role} className={`journey-seg ${phase.index === i ? "on" : ""}`} style={{ flexGrow: PHASE_SHARE[role] }}>
                  {s.ambient?.program.scenes[i]?.name ?? role}
                </span>
              ))}
              <span className="journey-cursor" style={{ left: `${endS > 0 ? (Math.min(status?.program_position ?? 0, endS) / endS) * 100 : 0}%` }} />
            </div>
            <div className="muted small">
              {t[worldById(current)!.title]} · {phase.name} · {formatTime(status?.program_position ?? 0)} / {formatTime(endS)} · {t.scapesRemaining} {formatTime(phase.remaining_s)}
              {on && !running && ` · ${t.scapesPaused}`}
            </div>
          </>
        ) : (
          <div className="muted small">{t.scapesNoJourney}</div>
        )}
        <div className="muted small">{t.scapesEditHint}</div>
      </section>

      <section className="panel" data-coach="scapes-scenes">
        <h3>{t.scapesScenes}</h3>
        <SceneMatrix kind="scapes" />
      </section>
      <Fold id="scapes-editor" label={t.scapesEditorTitle} chip={t.scapesEditorChip}>
        <div className="muted small">{t.scapesEditorHint}</div>
        <Ambient kind="scapes" />
      </Fold>
    </main>
  );
}
