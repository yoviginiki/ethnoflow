import { useEffect, useRef, useState } from "react";
import { api, describeError, type NyPlugin, type SampleEntry } from "./api";
import { NyControls, nyDefaults, nyValues, type NyValues } from "./NyControls";
import { messages } from "./i18n";
import { useStore } from "./store";

/**
 * A9: a Nyquist *generate* plug-in straight into the sample list (owner 1.11: "I want sound
 * generators"): pick one, set its controls, ▶ listens to the rendered WAV, "+" writes it into
 * Documents\EthnoFlow\Samples\Generated and hands it to the caller as a sample entry.
 */
export function NyGenerate({ onGenerated, busy }: { onGenerated: (e: SampleEntry) => Promise<void>; busy: boolean }) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [plugins, setPlugins] = useState<NyPlugin[]>([]);
  const [sel, setSel] = useState<string>("");
  const [values, setValues] = useState<NyValues>({});
  const [working, setWorking] = useState(false);
  // "■ Stop" lit while the rendered WAV plays (owner 3.11: a silent button looks broken).
  const [playing, setPlaying] = useState(false);
  const timer = useRef<number | null>(null);
  const plugin = plugins.find((p) => p.id === sel) ?? null;
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  useEffect(() => {
    api
      .nyList()
      .then((l) => setPlugins(l.filter((p) => p.kind === "generate")))
      .catch(fail);
  }, []);

  const pick = (id: string) => {
    setSel(id);
    const p = plugins.find((x) => x.id === id);
    if (p) setValues(nyDefaults(p));
  };
  const render = async (): Promise<SampleEntry | null> => {
    if (!plugin) return null;
    const out = await api.nyGenerateWav(plugin.id, nyValues(plugin, values), null, plugin.name.replace(/^EthnoFlow: /, ""));
    const st = useStore.getState();
    const folders = st.prefs.sampleFolders ?? [];
    if (!folders.includes(out.folder)) st.setPref("sampleFolders", [...folders, out.folder]);
    const name = out.path.split(/[\\/]/).pop() ?? "sample.wav";
    return { path: out.path, name, folder: out.folder, ext: "wav", bytes: 0, modified_ms: Date.now(), duration_s: out.seconds, sample_rate: out.sample_rate, channels: out.channels };
  };
  const listen = async () => {
    if (playing) {
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = null;
      setPlaying(false);
      await api.samplePreviewStop().catch(() => undefined);
      return;
    }
    setWorking(true);
    try {
      const e = await render();
      if (e) {
        const secs = await api.samplePreview(e.path, s.prefs.selectedOutput);
        setPlaying(true);
        timer.current = window.setTimeout(() => setPlaying(false), secs * 1000 + 200);
      }
    } catch (err) {
      fail(err);
    } finally {
      setWorking(false);
    }
  };
  const add = async () => {
    setWorking(true);
    try {
      const e = await render();
      if (e) await onGenerated(e);
    } catch (err) {
      fail(err);
    } finally {
      setWorking(false);
    }
  };

  return (
    <div className="ny-generate">
      <div className="analysis-row">
        <span className="small">⚡ {t.nyGenTitle}</span>
        <select value={sel} onChange={(e) => pick(e.target.value)} aria-label={t.nyGenTitle}>
          <option value="">—</option>
          {plugins.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </div>
      {plugin && (
        <div className="fx-form">
          <NyControls plugin={plugin} values={values} onChange={setValues} />
          <div className="analysis-row">
            <button className={`small ${playing ? "on primary" : ""}`} onClick={() => void listen()} disabled={working || busy}>
              {working ? `⏳ ${t.nyChecking}` : playing ? `■ ${t.nyStop}` : `▶ ${t.nyPreviewGen}`}
            </button>
            <button className="small primary" onClick={() => void add()} disabled={working || busy || !s.project}>
              + {t.nyGenAdd}
            </button>
            <span className="muted small">{t.nyGenHint}</span>
          </div>
        </div>
      )}
    </div>
  );
}
