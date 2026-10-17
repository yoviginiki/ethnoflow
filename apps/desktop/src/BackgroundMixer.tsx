import { useEffect, useState } from "react";
import { api } from "./api";
import { Knob } from "./Knob";
import { formatDb, linearToDb, meterFraction } from "./format";
import { messages } from "./i18n";
import { DEFAULT_MIXER, useStore, type LayerName, type MixerStrip } from "./store";

const LAYERS: LayerName[] = ["drone", "pad", "ambient", "rhythm"];
const HZ_MIN = 40;
const HZ_MAX = 20_000;
const hzToPos = (hz: number) => Math.log2(Math.max(HZ_MIN, hz) / HZ_MIN) / Math.log2(HZ_MAX / HZ_MIN);
const posToHz = (p: number) => Math.round(HZ_MIN * Math.pow(HZ_MAX / HZ_MIN, Math.min(1, Math.max(0, p))));
const fmtHz = (hz: number) => (hz >= 1000 ? `${(hz / 1000).toFixed(1)} kHz` : `${Math.round(hz)} Hz`);
const fmtS = (s: number) => `${s.toFixed(1)} s`;

/**
 * Background mixer (owner request 2026-10-17): one strip per background layer — drone,
 * pad, atmosphere, rhythm — with trim, pan, low/high cut and fade in/out times, plus the
 * master level. The strips live in the shell as buses folded into every member slot
 * (`bus_cmds.rs`); switching a layer on or off fades it over its own times.
 */
export function BackgroundMixer() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const mixer = s.prefs.mixer ?? DEFAULT_MIXER;
  const layers = s.layers;
  const masterDb = s.prefs.masterDb ?? 0;
  const [peaks, setPeaks] = useState<number[]>([0, 0, 0, 0]);
  const anyOn = layers.drone || layers.pad || layers.ambient || layers.rhythm;
  const names: Record<LayerName, string> = { drone: t.drone, pad: t.pad, ambient: t.areaAmbient, rhythm: t.rhythmTitle };

  useEffect(() => {
    if (!anyOn) {
      setPeaks([0, 0, 0, 0]);
      return;
    }
    const id = window.setInterval(() => {
      api
        .busStatus()
        .then(setPeaks)
        .catch(() => undefined);
    }, 150);
    return () => window.clearInterval(id);
  }, [anyOn]);

  const set = (name: LayerName, patch: Partial<MixerStrip>) => s.setPref("mixer", { ...mixer, [name]: { ...mixer[name], ...patch } });

  return (
    <div className="bg-mixer" role="group" aria-label={t.bgMixer}>
      {LAYERS.map((name, i) => {
        const m = mixer[name];
        const on = layers[name];
        const pk = linearToDb(peaks[i] ?? 0);
        return (
          <div key={name} className={`bg-strip ${on ? "" : "off"}`}>
            <div className="analysis-row">
              <strong>{names[name]}</strong>
              <button className={`small ${on ? "primary" : "ghost"}`} onClick={() => s.setLayer(name, !on)} aria-pressed={on} title={on ? t.busOffHint : t.busOnHint}>
                {on ? t.busOn : t.busOff}
              </button>
            </div>
            <div className="hmeter" aria-hidden="true">
              <div className={`hmeter-fill ${pk > -3 ? "hot" : pk > -12 ? "warm" : ""}`} style={{ width: `${meterFraction(pk) * 100}%` }} />
            </div>
            <div className="bg-strip-knobs">
              <div className="ss-knob narrow">
                <Knob value={m.gain_db} min={-40} max={12} step={0.5} reset={0} size={52} label={`${t.trimLabel}: ${names[name]}`} text={m.gain_db <= -39.5 ? "off" : formatDb(m.gain_db)} onChange={(v) => set(name, { gain_db: v })} />
                <span className="small">{t.trimLabel}</span>
              </div>
              <div className="ss-knob narrow">
                <Knob value={m.pan} min={-1} max={1} step={0.05} reset={0} size={44} label={`${t.panLabel}: ${names[name]}`} text={m.pan === 0 ? "C" : m.pan < 0 ? `L${Math.round(-m.pan * 100)}` : `R${Math.round(m.pan * 100)}`} onChange={(v) => set(name, { pan: v })} />
                <span className="small">{t.panShort}</span>
              </div>
              <div className="ss-knob narrow">
                <Knob value={hzToPos(m.highpass_hz)} min={0} max={1} step={0.005} reset={0} size={44} label={`${t.highpassLabel}: ${names[name]}`} text={fmtHz(m.highpass_hz)} onChange={(p) => set(name, { highpass_hz: Math.min(8000, Math.max(10, posToHz(p))) })} />
                <span className="small">HP</span>
              </div>
              <div className="ss-knob narrow">
                <Knob value={hzToPos(m.lowpass_hz)} min={0} max={1} step={0.005} reset={1} size={44} label={`${t.lowpassLabel}: ${names[name]}`} text={fmtHz(m.lowpass_hz)} onChange={(p) => set(name, { lowpass_hz: posToHz(p) })} />
                <span className="small">LP</span>
              </div>
              <div className="ss-knob narrow">
                <Knob value={m.fadeIn} min={0} max={30} step={0.1} reset={2} size={44} label={`${t.fadeInLabel}: ${names[name]}`} text={fmtS(m.fadeIn)} onChange={(v) => set(name, { fadeIn: v })} />
                <span className="small">{t.fadeInLabel}</span>
              </div>
              <div className="ss-knob narrow">
                <Knob value={m.fadeOut} min={0} max={30} step={0.1} reset={3} size={44} label={`${t.fadeOutLabel}: ${names[name]}`} text={fmtS(m.fadeOut)} onChange={(v) => set(name, { fadeOut: v })} />
                <span className="small">{t.fadeOutLabel}</span>
              </div>
            </div>
          </div>
        );
      })}
      <div className="bg-strip master">
        <strong>{t.backgroundLevel}</strong>
        <div className="ss-knob">
          <Knob value={masterDb} min={-40} max={6} step={1} reset={0} size={60} label={t.backgroundLevel} text={masterDb <= -39.5 ? "off" : formatDb(masterDb)} onChange={(v) => s.setPref("masterDb", v)} />
          <span className="small">{t.levelLabel}</span>
        </div>
        {masterDb > 0 && <span className="badge warn">{t.headphoneWarn}</span>}
        <div className="muted small">{t.bgMixerHint}</div>
      </div>
    </div>
  );
}
