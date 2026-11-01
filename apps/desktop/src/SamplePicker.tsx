import { useEffect, useMemo, useState } from "react";
import { api, describeError, type LayerKind, type SampleEntry } from "./api";
import { defaultSource, makeLayer, MAX_AMBIENT_LAYERS } from "./ambientPresets";
import { formatTime } from "./format";
import { messages } from "./i18n";
import { RACKS, type RackKind } from "./rackAdapter";
import { recordChange } from "./rackHistory";
import { useStore } from "./store";

const SHOWN = 200;
const AS: LayerKind[] = ["loop", "cloud", "sampler"];

/**
 * The sample list of the Soundscapes studio (owner 1.11: "a list on the left, so I can
 * hear before I add, with a play button on the samples"): every audio file of the
 * folders in My samples, searchable, ▶ previews through the output, "+" imports the
 * file as a take and adds it to the rack as a loop, a cloud or a sampler layer.
 */
export function SamplePicker({ kind }: { kind: RackKind }) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const folders = s.prefs.sampleFolders ?? [];
  const [entries, setEntries] = useState<SampleEntry[]>([]);
  const [scanning, setScanning] = useState(false);
  const [query, setQuery] = useState("");
  const [previewing, setPreviewing] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [as, setAs] = useState<LayerKind>("loop");
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  useEffect(() => {
    let cancelled = false;
    if (folders.length === 0) {
      setEntries([]);
      return;
    }
    setScanning(true);
    api
      .samplesScan(folders, 5000)
      .then((r) => {
        if (!cancelled) setEntries(r.entries);
      })
      .catch(fail)
      .finally(() => {
        if (!cancelled) setScanning(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folders.join("|")]);

  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    const f = q ? entries.filter((e) => e.name.toLowerCase().includes(q) || e.folder.toLowerCase().includes(q)) : entries;
    return f.slice(0, SHOWN);
  }, [entries, query]);

  const preview = async (e: SampleEntry) => {
    if (previewing === e.path) {
      await api.samplePreviewStop().catch(() => undefined);
      setPreviewing(null);
      return;
    }
    try {
      setPreviewing(e.path);
      const secs = await api.samplePreview(e.path, s.prefs.selectedOutput);
      window.setTimeout(() => setPreviewing((p) => (p === e.path ? null : p)), secs * 1000 + 200);
    } catch (err) {
      setPreviewing(null);
      fail(err);
    }
  };

  const add = async (e: SampleEntry) => {
    if (!s.project) return;
    setBusy(e.path);
    try {
      let pv = await api.importAudio(e.path);
      const id = pv.loaded_take;
      if (id) {
        const tk = pv.takes.find((x) => x.id === id);
        if (tk && !tk.label) pv = await api.renameTake(id, e.name);
      }
      useStore.getState().setProject(pv);
      if (!id) return;
      const R = RACKS[kind];
      const rack = R.read(useStore.getState()) ?? (await R.get()).ambient;
      if (rack.layers.length >= MAX_AMBIENT_LAYERS) {
        useStore.getState().setMessage(t.maxLayers);
        return;
      }
      const layer = makeLayer(e.name, defaultSource(as, id), { gain_db: -18 });
      const v = await R.set({ ...rack, layers: [...rack.layers, layer] });
      recordChange(kind, rack, v.ambient, t.hLayerAdded(e.name), t.hStart);
      R.store(v.ambient);
      useStore.getState().setMessage(`${t.importedOk}: ${e.name}`);
    } catch (err) {
      fail(err);
    } finally {
      setBusy(null);
    }
  };

  const shortFolder = (f: string) => f.split(/[\\/]/).filter(Boolean).slice(-1)[0] ?? f;

  return (
    <aside className="panel sample-picker" aria-label={t.scapesSamples}>
      <h3>{t.scapesSamples}</h3>
      <div className="muted small">{t.scapesSamplesHint}</div>
      {folders.length === 0 && (
        <div className="muted small">
          {t.scapesNoFolders}{" "}
          <button className="ghost small" onClick={() => s.setScreen("samples")}>
            {t.areaSamples}
          </button>
        </div>
      )}
      <div className="analysis-row">
        <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t.search} aria-label={t.search} />
        <label className="inline small">
          +
          <select value={as} onChange={(e) => setAs(e.target.value as LayerKind)} aria-label={t.scapesAddAs}>
            {AS.map((k) => (
              <option key={k} value={k}>
                {k === "loop" ? t.kindLoop : k === "cloud" ? t.kindCloud : t.kindSampler}
              </option>
            ))}
          </select>
        </label>
        {scanning && <span className="muted small">…</span>}
      </div>
      <div className="sample-list">
        {list.map((e) => (
          <div key={e.path} className={`sample-row ${previewing === e.path ? "playing" : ""}`}>
            <button className={`ghost small ${previewing === e.path ? "on" : ""}`} onClick={() => void preview(e)} title={t.scapesPreview} aria-label={t.scapesPreview}>
              {previewing === e.path ? "■" : "▶"}
            </button>
            <span className="grow small" title={e.path}>
              {e.name}
              <span className="muted"> · {e.duration_s !== null ? formatTime(e.duration_s) : "?"} · {shortFolder(e.folder)}</span>
            </span>
            <button className="small" onClick={() => void add(e)} disabled={busy !== null || !s.project} title={t.scapesAddAs}>
              +
            </button>
          </div>
        ))}
        {entries.length > SHOWN && list.length === SHOWN && <div className="muted small">{t.scapesMoreSamples}</div>}
      </div>
    </aside>
  );
}
