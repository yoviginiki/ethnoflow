import { useState } from "react";
import { api, describeError, SCALE_KINDS, type Scale, type ScaleKind } from "./api";
import { BackgroundTransport } from "./BackgroundTransport";
import { scaleFromKind } from "./AnalysisPanel";
import { DRONE_PRESETS, DronePanel } from "./DronePanel";
import { messages, type StringMessageKey } from "./i18n";
import { IntentBadge } from "./IntentPanel";
import { PadPanel } from "./PadPanel";
import { RhythmPanel } from "./RhythmPanel";
import { DEFAULT_DRONE, DEFAULT_PAD, DEFAULT_RHYTHM, useStore, type DronePrefs, type PadPrefs } from "./store";
import { TimingPanel } from "./TimingPanel";

/** Styles: what each one sets on the layers. Styles that need later phases are shown
 *  as planned, never as buttons that pretend. */
interface Style {
  key: StringMessageKey;
  phase: number | null;
  drone: Partial<DronePrefs> & { on: boolean };
  pad: Partial<PadPrefs> & { on: boolean };
}
const STYLES: Style[] = [
  { key: "styleTraditional", phase: null, drone: { ...DRONE_PRESETS.dronePresetTraditional, on: true, levelDb: -10 }, pad: { on: false } },
  { key: "styleLiquid", phase: null, drone: { ...DRONE_PRESETS.dronePresetDeep, on: true, levelDb: -16 }, pad: { on: true, emotion: "melancholy", voicing: "scale_triad", levelDb: -12 } },
  { key: "styleCinematic", phase: null, drone: { ...DRONE_PRESETS.dronePresetCinematic, on: true, levelDb: -14 }, pad: { on: true, emotion: "calm", voicing: "fifth9", levelDb: -12 } },
  { key: "styleDnb", phase: null, drone: { ...DRONE_PRESETS.dronePresetDeep, on: false, levelDb: -18 }, pad: { on: true, emotion: "melancholy", voicing: "scale_7th", levelDb: -14 } },
  { key: "styleNoKey", phase: null, drone: { on: false }, pad: { on: false } },
];

/** Compose First (F3.5): a sounding background in a chosen tonal centre and mode,
 *  without recording a melody first; then "Play & record" takes the instrument. */
export function Compose() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const scale = project.scale;
  const dronePrefs = s.prefs.drone ?? DEFAULT_DRONE;
  const padPrefs = s.prefs.pad ?? DEFAULT_PAD;
  const [tension, setTension] = useState(false);
  const busy = s.pending !== null;
  const fail = (e: unknown) => s.setError(describeError(e));

  const chooseScale = async (tonic_pc: number | null, kind: ScaleKind | null) => {
    try {
      s.setPending(t.saving);
      const sc: Scale | null = tonic_pc === null || kind === null ? null : scaleFromKind({ tonic_pc, degrees_cents: [], kind });
      s.setProject(await api.setScale(sc));
      if (sc) {
        // The layers follow the tonal centre: roots in the octave below middle C, triad from the mode.
        const root = 48 + sc.tonic_pc;
        s.setPref("drone", { ...dronePrefs, rootMidi: root });
        s.setPref("pad", { ...padPrefs, rootMidi: root, voicing: padPrefs.voicing.startsWith("scale") ? padPrefs.voicing : "scale_triad" });
      }
    } catch (e) {
      fail(e);
    } finally {
      s.setPending(null);
    }
  };

  const applyStyle = async (st: Style) => {
    if (st.key === "styleDnb") {
      // Deep Ethno DnB: 172 BPM in 4/4 when the project has no tempo yet (the user
      // pressed the style), liquid drums + Reese bass + pad; the drone steps back.
      const tm = project.timing;
      if (tm.bpm === null || tm.numerator === null || tm.unit === null) {
        try {
          s.setProject(await api.setTiming({ bpm: tm.bpm ?? 172, numerator: tm.numerator ?? 4, unit: tm.unit ?? 4, grouping: tm.grouping }));
        } catch (e) {
          fail(e);
          return;
        }
      }
      const rp = s.prefs.rhythm ?? DEFAULT_RHYTHM;
      s.setPref("rhythm", {
        ...rp,
        drums: { ...rp.drums, on: true, style: "liquid", complexity: 0.45, level_db: -10 },
        bass: { ...rp.bass, on: true, voice: 2, density: 0.5, syncopation: 0.4, wobble_div: 8, wobble_depth: 0.4, sidechain: 0.7, level_db: -12 },
      });
      s.setLayer("rhythm", true);
    }
    if (st.key === "styleNoKey") {
      // Ambient No Key: textures without a tonal centre live in the Atmosphere rack.
      s.setLayer("drone", false);
      s.setLayer("pad", false);
      s.setScreen("ambient");
      return;
    }
    const { on: droneOn, ...d } = st.drone;
    const { on: padOn, ...p } = st.pad;
    s.setPref("drone", { ...dronePrefs, ...d });
    s.setPref("pad", { ...padPrefs, ...p });
    s.setLayer("drone", droneOn);
    s.setLayer("pad", padOn);
  };

  const playAndRecord = () => {
    if (!s.layers.drone && !s.layers.pad) s.setLayer("drone", true);
    s.setPendingRecord(true);
    s.setScreen("play");
  };

  const tonic = scale?.tonic_pc ?? null;
  const kind = scale?.kind ?? null;

  return (
    <main className="compose">
      <section className="panel">
        <h2>{t.composeTitle}</h2>
        <div className="muted small">{t.composeIntro}</div>
        <div className="analysis-row">
          <label>
            {t.tonalCentre}
            <select value={tonic ?? ""} disabled={busy} onChange={(e) => void chooseScale(e.target.value === "" ? null : Number(e.target.value), kind ?? "aeolian")}>
              <option value="">{t.chooseTonic}</option>
              {t.noteNames.map((n, pc) => (
                <option key={pc} value={pc}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t.scaleLabel}
            <select value={kind ?? ""} disabled={busy || tonic === null} onChange={(e) => void chooseScale(tonic, e.target.value ? (e.target.value as ScaleKind) : null)}>
              <option value="">{t.chooseMode}</option>
              {SCALE_KINDS.map((k) => (
                <option key={k} value={k}>
                  {t.scaleKinds[k]}
                </option>
              ))}
            </select>
          </label>
          <label className="inline">
            <input type="checkbox" checked={tension} onChange={(e) => setTension(e.target.checked)} />
            {t.allowTension}
          </label>
        </div>
        {scale && scale.kind === null && <div className="muted small">{t.customScaleKept}</div>}
        {!scale && <div className="muted small">{t.noScaleYet}</div>}
      </section>

      <TimingPanel />

      <section className="panel">
        <h3>{t.styleLabel}</h3>
        <div className="style-row">
          {STYLES.map((st) =>
            st.phase === null ? (
              <button key={st.key} className="ghost" onClick={() => void applyStyle(st)} disabled={busy || (!scale && st.key !== "styleNoKey")}>
                {t[st.key]}
              </button>
            ) : (
              <span key={st.key} className="area small" aria-disabled="true">
                {t[st.key]} <span className="badge">{`${t.availableFromPhase} ${st.phase}`}</span>
              </span>
            ),
          )}
        </div>
        <div className="muted small">{t.styleHint}</div>
      </section>

      <section className="panel analysis-row">
        <IntentBadge onOpen={() => s.setScreen("play")} />
        <span className="muted small">{t.intentComposeHint}</span>
      </section>

      <DronePanel scale={scale} profile={project.instrument} />
      <PadPanel scale={scale} profile={project.instrument} allowTension={tension} />

      <RhythmPanel />

      <section className="panel">
        <h3>{t.backgroundTransport}</h3>
        <div className="analysis-row">
          <BackgroundTransport />
        </div>
        <span className="muted small">{t.backgroundLevelHint}</span>
      </section>

      <section className="panel analysis-row">
        <button className="ghost" onClick={() => s.setScreen("play")}>
          {t.toStudio}
        </button>
        <button className="ghost" onClick={() => s.setScreen("studio")}>
          {t.areaStudio}
        </button>
        <button className="primary" onClick={playAndRecord} disabled={busy || (!s.layers.drone && !s.layers.pad && (dronePrefs.rootMidi ?? scale) === null)}>
          {t.playAndRecord}
        </button>
        <span className="muted small">{t.playAndRecordHint}</span>
      </section>
    </main>
  );
}
