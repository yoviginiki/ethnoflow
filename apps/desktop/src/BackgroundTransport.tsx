import { droneRoot, padRoot } from "./layers";
import { messages } from "./i18n";
import { DEFAULT_DRONE, DEFAULT_PAD, useStore } from "./store";

interface Props {
  /** One row: play/stop, the two layer switches and the level. */
  compact?: boolean;
}

/**
 * Play / stop for the background (drone + pad) wherever it is needed: the
 * accompanist, the studio transport, the floating bar and the track editor. The
 * layers themselves run in `useBackgroundLayers`; this only flips the switches and
 * remembers which ones were on, so "play" brings the same background back.
 */
export function BackgroundTransport({ compact = false }: Props) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const layers = useStore((s) => s.layers);
  const lastLayers = useStore((s) => s.lastLayers);
  const setLayer = useStore((s) => s.setLayer);
  const setLastLayers = useStore((s) => s.setLastLayers);
  const masterDb = useStore((s) => s.prefs.masterDb ?? 0);
  const setPref = useStore((s) => s.setPref);
  const project = useStore((s) => s.project);
  const dronePrefs = useStore((s) => s.prefs.drone ?? DEFAULT_DRONE);
  const padPrefs = useStore((s) => s.prefs.pad ?? DEFAULT_PAD);
  const scale = project?.scale ?? null;
  const profile = project?.instrument ?? null;
  const canDrone = droneRoot(dronePrefs, scale, profile) !== null;
  const canPad = padRoot(padPrefs, dronePrefs, scale, profile) !== null;
  const canAmbient = (useStore((s) => s.ambient?.layers.length) ?? 0) > 0;
  const timing = project?.timing;
  const canRhythm = !!timing && timing.bpm !== null && timing.numerator !== null && timing.unit !== null && (scale !== null || canDrone);
  const on = layers.drone || layers.pad || layers.ambient || layers.rhythm;
  const canPlay = canDrone || canPad || canAmbient || canRhythm;

  const play = () => {
    const want = {
      drone: (lastLayers.drone || (!lastLayers.pad && !lastLayers.ambient && !lastLayers.rhythm)) && canDrone,
      pad: lastLayers.pad && canPad,
      ambient: lastLayers.ambient && canAmbient,
      rhythm: lastLayers.rhythm && canRhythm,
    };
    if (!want.drone && !want.pad && !want.ambient && !want.rhythm) {
      if (canDrone) want.drone = true;
      else if (canPad) want.pad = true;
      else if (canAmbient) want.ambient = true;
      else if (canRhythm) want.rhythm = true;
    }
    setLayer("drone", want.drone);
    setLayer("pad", want.pad);
    setLayer("ambient", want.ambient);
    setLayer("rhythm", want.rhythm);
  };
  const stop = () => {
    setLastLayers({ ...layers });
    setLayer("drone", false);
    setLayer("pad", false);
    setLayer("ambient", false);
    setLayer("rhythm", false);
  };

  return (
    <span className={`bg-transport ${compact ? "compact" : ""}`}>
      {on ? (
        <button onClick={stop} aria-label={t.bgStop} title={t.bgStop}>
          ■ {compact ? t.background : t.bgStop}
        </button>
      ) : (
        <button onClick={play} disabled={!project || !canPlay} aria-label={t.bgPlay} title={canPlay ? t.bgPlay : t.bgNeedsRoot}>
          ▶ {compact ? t.background : t.bgPlay}
        </button>
      )}
      <label className="inline">
        <input type="checkbox" checked={layers.drone} disabled={!canDrone} onChange={(e) => setLayer("drone", e.target.checked)} />
        {compact ? t.droneShort : t.drone}
      </label>
      <label className="inline">
        <input type="checkbox" checked={layers.pad} disabled={!canPad} onChange={(e) => setLayer("pad", e.target.checked)} />
        {compact ? t.padShort : t.pad}
      </label>
      <label className="inline">
        <input type="checkbox" checked={layers.ambient} disabled={!canAmbient} onChange={(e) => setLayer("ambient", e.target.checked)} />
        {compact ? t.ambientShort : t.areaAmbient}
      </label>
      <label className="inline">
        <input type="checkbox" checked={layers.rhythm} disabled={!canRhythm} onChange={(e) => setLayer("rhythm", e.target.checked)} />
        {compact ? t.rhythmShort : t.rhythmTitle}
      </label>
      <label className="inline" title={t.backgroundLevel}>
        🔊
        <input type="range" min={-40} max={6} step={1} value={masterDb} onChange={(e) => setPref("masterDb", Number(e.target.value))} style={{ width: compact ? 80 : 140 }} />
        <span className="mono-num">
          {masterDb > 0 ? "+" : ""}
          {masterDb} dB
        </span>
      </label>
      {!compact && !canPlay && <span className="muted small">{t.bgNeedsRoot}</span>}
    </span>
  );
}
