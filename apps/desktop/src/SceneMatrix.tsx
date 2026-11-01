import { useState } from "react";
import { describeError, type Ambient, type Scene } from "./api";
import { formatDb, formatTime } from "./format";
import { messages } from "./i18n";
import { RACKS, type RackKind } from "./rackAdapter";
import { recordChange } from "./rackHistory";
import { useStore } from "./store";

/**
 * "When does which layer play": the journey's scenes as a table — rows are the rack's
 * layers, columns its scenes; a cell is the layer's level in that scene (−60 = silent),
 * a column head its duration and transition. Edits go to the rack's program, so the
 * engine, the export and the presets all see them (owner 1.11: "кога кое ще се пуска").
 */
export function SceneMatrix({ kind }: { kind: RackKind }) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const R = RACKS[kind];
  const rack = R.read(s);
  const [busy, setBusy] = useState(false);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));
  if (!rack) return null;
  const scenes = rack.program.scenes;
  if (scenes.length === 0) return <div className="muted small">{t.scenesNone}</div>;

  const commit = async (next: Ambient, label: string) => {
    setBusy(true);
    try {
      const v = await R.set(next);
      recordChange(kind, rack, v.ambient, label, t.hStart);
      R.store(v.ambient);
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };
  /** Recompute start times after a duration change; keep the loop region on the whole program. */
  const relayout = (list: Scene[]): Scene[] => {
    let at = 0;
    return list.map((sc) => {
      const out = { ...sc, start_s: at };
      at += sc.duration_s;
      return out;
    });
  };
  const levelOf = (sc: Scene, layerId: string) => sc.snapshot.layers.find((l) => l.id === layerId)?.gain_db ?? null;
  const setLevel = (si: number, layerId: string, gain_db: number) => {
    const layer = rack.layers.find((l) => l.id === layerId);
    if (!layer) return;
    const scenesNext = scenes.map((sc, i) => {
      if (i !== si) return sc;
      const has = sc.snapshot.layers.some((l) => l.id === layerId);
      const layers = has
        ? sc.snapshot.layers.map((l) => (l.id === layerId ? { ...l, gain_db } : l))
        : [...sc.snapshot.layers, { id: layer.id, gain_db, pan: layer.pan, lowpass_hz: layer.lowpass_hz, highpass_hz: layer.highpass_hz, mute: layer.mute, source: layer.source }];
      return { ...sc, snapshot: { ...sc.snapshot, layers } };
    });
    void commit({ ...rack, program: { ...rack.program, scenes: scenesNext } }, t.hSceneLevel(layer.name, scenes[si].name));
  };
  const setDuration = (si: number, duration_s: number) => {
    const list = relayout(scenes.map((sc, i) => (i === si ? { ...sc, duration_s: Math.max(5, Math.round(duration_s)) } : sc)));
    const total = list.reduce((a, sc) => a + sc.duration_s, 0);
    void commit({ ...rack, program: { scenes: list, loop_region: rack.program.loop_region ? [0, total] : null } }, t.hSceneDuration(scenes[si].name));
  };
  const setTransition = (si: number, transition_s: number) => {
    const list = scenes.map((sc, i) => (i === si ? { ...sc, transition_s: Math.max(0, Math.min(sc.duration_s, Math.round(transition_s))) } : sc));
    void commit({ ...rack, program: { ...rack.program, scenes: list } }, t.hSceneDuration(scenes[si].name));
  };
  const total = scenes.reduce((a, sc) => a + sc.duration_s, 0);

  return (
    <div className="scene-matrix-wrap" aria-busy={busy}>
      <table className="scene-matrix">
        <thead>
          <tr>
            <th>{t.scenesLayer}</th>
            {scenes.map((sc, si) => (
              <th key={sc.id}>
                <div>{sc.name}</div>
                <label className="inline small" title={t.sceneDuration}>
                  <input type="number" min={5} max={3600} value={sc.duration_s} onChange={(e) => setDuration(si, Number(e.target.value))} style={{ width: 64 }} /> s
                </label>
                <label className="inline small" title={t.sceneTransition}>
                  ↗ <input type="number" min={0} max={sc.duration_s} value={sc.transition_s} onChange={(e) => setTransition(si, Number(e.target.value))} style={{ width: 52 }} /> s
                </label>
                <div className="muted small mono-num">{formatTime(sc.start_s)}</div>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rack.layers.map((l) => (
            <tr key={l.id}>
              <td className="small">{l.name}</td>
              {scenes.map((sc, si) => {
                const v = levelOf(sc, l.id);
                const off = v === null || v <= -59.5;
                return (
                  <td key={sc.id} className={off ? "off" : ""}>
                    <input
                      type="range"
                      min={-60}
                      max={6}
                      step={1}
                      value={v ?? -60}
                      onChange={(e) => setLevel(si, l.id, Number(e.target.value))}
                      aria-label={`${l.name} · ${sc.name}`}
                      title={off ? t.sceneSilent : formatDb(v ?? -60)}
                    />
                    <span className="mono-num small">{off ? "—" : formatDb(v ?? -60)}</span>
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="muted small">
        {t.scenesTotal}: {formatTime(total)} · {t.scenesHint}
      </div>
    </div>
  );
}
