import { useEffect, useState } from "react";
import { api, type Ambient, type Recipe, type TakeMeta, type TakeView } from "./api";
import { defaultSource, makeLayer } from "./ambientPresets";
import { formatTime } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";

interface Props {
  take: TakeView;
  selectTake: (id: string) => Promise<void>;
  withPending: <T>(label: string, fn: () => Promise<T>) => Promise<T | undefined>;
}

export const DEFAULT_RECIPE: Recipe = { dc_remove: false, highpass_hz: null, gain_db: 0, normalize_peak_dbfs: null, fade_in_s: 0, fade_out_s: 0, loop_crossfade_s: 0, reverse: false };
export const CATEGORIES = ["water", "wind", "wood", "metal", "breath", "voice", "instrument", "room", "other"] as const;
const COMMIT_MS = 250;

/**
 * Sample Lab (F1A.6): a non-destructive recipe on the loaded take (the original file
 * never changes), silence-based trim suggestion, zero-crossing snap, tags and licence,
 * and "make it a layer" in the Atmosphere.
 */
export function SampleLab({ take, selectTake, withPending }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [recipe, setRecipe] = useState<Recipe>(take.recipe ?? DEFAULT_RECIPE);
  const [meta, setMeta] = useState<TakeMeta>(take.meta);
  const [open, setOpen] = useState(false);
  const [threshold, setThreshold] = useState(-45);
  const [report, setReport] = useState<{ sound_start: number | null; sound_end: number | null; silences: [number, number][] } | null>(null);
  const busy = s.pending !== null || s.mode !== "idle";

  // A different take: show its own recipe and tags.
  useEffect(() => {
    setRecipe(take.recipe ?? DEFAULT_RECIPE);
    setMeta(take.meta);
    setReport(null);
  }, [take.id, take.recipe, take.meta]);

  // Recipe changes apply shortly after the last slider move (the take reloads).
  const [pendingRecipe, setPendingRecipe] = useState<Recipe | null>(null);
  useEffect(() => {
    if (!pendingRecipe) return;
    const id = window.setTimeout(() => {
      const r = pendingRecipe;
      setPendingRecipe(null);
      void (async () => {
        const pv = await withPending(t.saving, () => api.setTakeRecipe(take.id, r));
        if (pv) {
          s.setProject(pv);
          await selectTake(take.id);
        }
      })();
    }, COMMIT_MS);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingRecipe]);

  const patchRecipe = (p: Partial<Recipe>) => {
    const next = { ...recipe, ...p };
    setRecipe(next);
    setPendingRecipe(next);
  };
  const saveMeta = async (m: TakeMeta) => {
    setMeta(m);
    const pv = await withPending(t.saving, () => api.setTakeMeta(take.id, m));
    if (pv) s.setProject(pv);
  };
  const analyze = async () => {
    const r = await withPending(t.analyzing, () => api.analyzeSilence(take.id, threshold));
    if (r) setReport(r);
  };
  const applySuggestedTrim = async () => {
    if (!report || report.sound_start === null || report.sound_end === null) return;
    const atEnd = take.duration_seconds !== null && report.sound_end >= take.duration_seconds - 0.005;
    const pv = await withPending(t.saving, () => api.setTrim(take.id, report.sound_start!, atEnd ? null : report.sound_end));
    if (pv) {
      s.setProject(pv);
      await selectTake(take.id);
    }
  };
  const snapZero = async () => {
    const pv = await withPending(t.saving, () => api.snapTrimToZero(take.id));
    if (pv) {
      s.setProject(pv);
      await selectTake(take.id);
    }
  };
  const toLayer = async () => {
    const rack: Ambient = s.ambient ? s.ambient : (await api.ambientGet()).ambient;
    if (rack.layers.length >= 8) {
      s.setError(t.maxLayers);
      return;
    }
    const name = take.label?.trim() || `${t.take} ${take.number}`;
    const layer = makeLayer(name, defaultSource("loop", take.id), { gain_db: -16, license_ref: meta.license || null });
    try {
      const v = await api.setAmbient({ ...rack, layers: [...rack.layers, layer] });
      s.setAmbient(v.ambient);
      s.setMessage(`${t.layerAdded}: ${name}`);
      s.setScreen("ambient");
    } catch (e) {
      s.setError(String(e));
    }
  };
  const isIdentity = JSON.stringify(recipe) === JSON.stringify(DEFAULT_RECIPE);

  return (
    <section className="panel sample-lab">
      <div className="analysis-row">
        <button className="ghost" onClick={() => setOpen((v) => !v)} aria-expanded={open}>
          {open ? "▾" : "▸"} {t.sampleLab}
        </button>
        {!isIdentity && <span className="badge">{t.recipeActive}</span>}
        {meta.category && <span className="badge">{t.categories[meta.category as (typeof CATEGORIES)[number]] ?? meta.category}</span>}
        <span className="muted small">{t.sampleLabHint}</span>
      </div>
      {open && (
        <>
          <div className="drone-sliders">
            <label className="drone-slider">
              <span>{t.recipeGain}</span>
              <input type="range" min={-24} max={24} step={0.5} value={recipe.gain_db} disabled={busy} onChange={(e) => patchRecipe({ gain_db: Number(e.target.value) })} onDoubleClick={() => patchRecipe({ gain_db: 0 })} />
              <span className="mono-num small">{recipe.gain_db > 0 ? "+" : ""}{recipe.gain_db} dB</span>
            </label>
            <label className="drone-slider">
              <span>
                <input type="checkbox" checked={recipe.normalize_peak_dbfs !== null} disabled={busy} onChange={(e) => patchRecipe({ normalize_peak_dbfs: e.target.checked ? -3 : null })} /> {t.recipeNormalize}
              </span>
              <input type="range" min={-24} max={0} step={0.5} value={recipe.normalize_peak_dbfs ?? -3} disabled={busy || recipe.normalize_peak_dbfs === null} onChange={(e) => patchRecipe({ normalize_peak_dbfs: Number(e.target.value) })} />
              <span className="mono-num small">{recipe.normalize_peak_dbfs ?? -3} dBFS</span>
            </label>
            <label className="drone-slider">
              <span>{t.fadeIn}</span>
              <input type="range" min={0} max={5} step={0.05} value={recipe.fade_in_s} disabled={busy} onChange={(e) => patchRecipe({ fade_in_s: Number(e.target.value) })} />
              <span className="mono-num small">{recipe.fade_in_s.toFixed(2)} s</span>
            </label>
            <label className="drone-slider">
              <span>{t.fadeOut}</span>
              <input type="range" min={0} max={5} step={0.05} value={recipe.fade_out_s} disabled={busy} onChange={(e) => patchRecipe({ fade_out_s: Number(e.target.value) })} />
              <span className="mono-num small">{recipe.fade_out_s.toFixed(2)} s</span>
            </label>
            <label className="drone-slider">
              <span>
                <input type="checkbox" checked={recipe.highpass_hz !== null} disabled={busy} onChange={(e) => patchRecipe({ highpass_hz: e.target.checked ? 80 : null })} /> {t.recipeHighpass}
              </span>
              <input type="range" min={0} max={1} step={0.005} value={Math.log((recipe.highpass_hz ?? 80) / 5) / Math.log(400)} disabled={busy || recipe.highpass_hz === null} onChange={(e) => patchRecipe({ highpass_hz: Math.round(5 * Math.pow(400, Number(e.target.value))) })} />
              <span className="mono-num small">{recipe.highpass_hz ?? 80} Hz</span>
            </label>
            <label className="drone-slider">
              <span>{t.recipeLoopXf}</span>
              <input type="range" min={0} max={2} step={0.01} value={recipe.loop_crossfade_s} disabled={busy} onChange={(e) => patchRecipe({ loop_crossfade_s: Number(e.target.value) })} />
              <span className="mono-num small">{recipe.loop_crossfade_s.toFixed(2)} s</span>
            </label>
          </div>
          <div className="analysis-row">
            <label className="inline">
              <input type="checkbox" checked={recipe.dc_remove} disabled={busy} onChange={(e) => patchRecipe({ dc_remove: e.target.checked })} />
              {t.recipeDc}
            </label>
            <label className="inline">
              <input type="checkbox" checked={recipe.reverse} disabled={busy} onChange={(e) => patchRecipe({ reverse: e.target.checked })} />
              {t.reverseLabel}
            </label>
            <button className="ghost small" disabled={busy || isIdentity} onClick={() => patchRecipe(DEFAULT_RECIPE)}>
              {t.recipeReset}
            </button>
          </div>
          <div className="analysis-row">
            <label className="inline">
              {t.silenceThreshold}
              <input type="number" min={-90} max={-10} step={1} value={threshold} onChange={(e) => setThreshold(Number(e.target.value))} />
              dBFS
            </label>
            <button className="ghost small" disabled={busy} onClick={() => void analyze()}>
              {t.findSilence}
            </button>
            {report && (
              <>
                <span className="mono-num small">
                  {report.sound_start !== null && report.sound_end !== null ? `${t.soundFrom} ${formatTime(report.sound_start)} ${t.soundTo} ${formatTime(report.sound_end)} · ${report.silences.length} ${t.silentParts}` : t.allSilent}
                </span>
                <button className="ghost small" disabled={busy || report.sound_start === null} onClick={() => void applySuggestedTrim()}>
                  {t.trimToSound}
                </button>
              </>
            )}
            <button className="ghost small" disabled={busy || (take.trim_in_seconds === 0 && take.trim_out_seconds === null)} onClick={() => void snapZero()} title={t.snapZeroHint}>
              {t.snapZero}
            </button>
          </div>
          <div className="meta-grid">
            <label>
              {t.metaCategory}
              <select value={meta.category} disabled={busy} onChange={(e) => void saveMeta({ ...meta, category: e.target.value })}>
                <option value="">—</option>
                {CATEGORIES.map((c) => (
                  <option key={c} value={c}>
                    {t.categories[c]}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t.metaLocation}
              <input value={meta.location} maxLength={200} disabled={busy} onChange={(e) => setMeta({ ...meta, location: e.target.value })} onBlur={() => void saveMeta(meta)} placeholder={t.metaLocationHint} />
            </label>
            <label>
              {t.metaMic}
              <input value={meta.mic} maxLength={120} disabled={busy} onChange={(e) => setMeta({ ...meta, mic: e.target.value })} onBlur={() => void saveMeta(meta)} />
            </label>
            <label>
              {t.metaLicense}
              <input value={meta.license} maxLength={200} disabled={busy} onChange={(e) => setMeta({ ...meta, license: e.target.value })} onBlur={() => void saveMeta(meta)} placeholder={t.metaLicenseHint} />
            </label>
            <label>
              {t.metaTags}
              <input value={meta.tags.join(", ")} maxLength={300} disabled={busy} onChange={(e) => setMeta({ ...meta, tags: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) })} onBlur={() => void saveMeta(meta)} placeholder={t.metaTagsHint} />
            </label>
            <label className="wide">
              {t.metaNotes}
              <textarea value={meta.notes} rows={2} maxLength={2000} disabled={busy} onChange={(e) => setMeta({ ...meta, notes: e.target.value })} onBlur={() => void saveMeta(meta)} />
            </label>
          </div>
          <div className="analysis-row">
            <button disabled={busy} onClick={() => void toLayer()}>
              {t.toAmbientLayer}
            </button>
            <span className="muted small">{t.toAmbientLayerHint}</span>
          </div>
        </>
      )}
    </section>
  );
}
