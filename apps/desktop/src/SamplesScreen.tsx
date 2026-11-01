import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, describeError, type LayerKind, type SampleEntry } from "./api";
import { defaultSource, makeLayer, MAX_AMBIENT_LAYERS } from "./ambientPresets";
import { formatTime } from "./format";
import { messages } from "./i18n";
import { useStore } from "./store";
import { planZones, zonesRegister } from "./multisample";
import { RACKS, type RackKind } from "./rackAdapter";
import { recordChange } from "./rackHistory";

type SortKey = "name" | "date" | "duration";
const PAGE = 300;

/**
 * Sample browser (backlog #6): folders such as Soundly's download folder are scanned
 * for audio; files can be searched, sorted, previewed through the graph, and imported
 * as a take, a looped layer or a cloud. Nothing is copied until "+".
 */
export function SamplesScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const folders = s.prefs.sampleFolders ?? [];
  const [entries, setEntries] = useState<SampleEntry[]>([]);
  const [scanning, setScanning] = useState(false);
  const [truncated, setTruncated] = useState(false);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortKey>("name");
  const [shown, setShown] = useState(PAGE);
  const [previewing, setPreviewing] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  /** Which rack a new layer goes to: the atmosphere or the Soundscapes rack. */
  const [target, setTarget] = useState<RackKind>("scapes");

  /** Import every note-named sample of a folder as a take and make one multisample Sampler layer. */
  const instrumentFromFolder = async (folder: string) => {
    if (!project) return;
    const plan = planZones(entries, folder);
    if (plan.length === 0) {
      useStore.getState().setMessage(t.multisampleNone);
      return;
    }
    setBusy(folder);
    try {
      const zones: { take_id: string; root_midi: number }[] = [];
      let pv = project;
      for (const z of plan.slice(0, 64)) {
        pv = await api.importAudio(z.entry.path);
        const id = pv.loaded_take;
        if (!id) continue;
        const tk = pv.takes.find((x) => x.id === id);
        if (tk && !tk.label) pv = await api.renameTake(id, z.entry.name);
        zones.push({ take_id: id, root_midi: z.root_midi });
      }
      useStore.getState().setProject(pv);
      const R = RACKS[target];
      const rack = R.read(useStore.getState()) ?? (await R.get()).ambient;
      if (rack.layers.length >= MAX_AMBIENT_LAYERS) {
        useStore.getState().setMessage(t.maxLayers);
        return;
      }
      const name = folder.split(/[\\/]/).filter(Boolean).pop() ?? t.kindSampler;
      const src = defaultSource("sampler", zones[0]?.take_id ?? "");
      const layer = makeLayer(name, { ...src, zones, root_midi: zones[0]?.root_midi ?? 60, register: zonesRegister(zones) } as typeof src, { gain_db: -16 });
      const next = { ...rack, layers: [...rack.layers, layer] };
      const v = await R.set(next);
      recordChange(target, rack, v.ambient, t.hLayerAdded(name), t.hStart);
      R.store(v.ambient);
      useStore.getState().setMessage(t.multisampleMade(name, zones.length));
    } catch (err) {
      fail(err);
    } finally {
      setBusy(null);
    }
  };
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));
  const project = s.project;

  const scan = async (list = folders) => {
    setScanning(true);
    try {
      const r = await api.samplesScan(list, 5000);
      setEntries(r.entries);
      setTruncated(r.truncated);
    } catch (e) {
      fail(e);
    } finally {
      setScanning(false);
    }
  };
  useEffect(() => {
    (async () => {
      let list = folders;
      if (list.length === 0) {
        const def = await api.samplesDefaultFolders().catch(() => [] as string[]);
        if (def.length > 0) {
          list = def;
          s.setPref("sampleFolders", def);
        }
      }
      await scan(list);
    })();
    return () => {
      void api.samplePreviewStop().catch(() => undefined);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const addFolder = async () => {
    const dir = await open({ title: t.samplesAddFolder, directory: true, multiple: false });
    if (!dir || Array.isArray(dir)) return;
    if (folders.includes(dir)) return;
    const next = [...folders, dir];
    s.setPref("sampleFolders", next);
    await scan(next);
  };
  const removeFolder = async (f: string) => {
    const next = folders.filter((x) => x !== f);
    s.setPref("sampleFolders", next);
    await scan(next);
  };

  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = q ? entries.filter((e) => e.name.toLowerCase().includes(q) || e.path.toLowerCase().includes(q)) : entries;
    const sorted = [...filtered];
    if (sort === "date") sorted.sort((a, b) => b.modified_ms - a.modified_ms);
    else if (sort === "duration") sorted.sort((a, b) => (b.duration_s ?? -1) - (a.duration_s ?? -1));
    return sorted;
  }, [entries, query, sort]);

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
  const importAs = async (e: SampleEntry, as: "take" | LayerKind) => {
    if (!project) return;
    setBusy(e.path);
    try {
      let pv = await api.importAudio(e.path);
      const id = pv.loaded_take;
      if (id) {
        const tk = pv.takes.find((x) => x.id === id);
        if (tk && !tk.label) pv = await api.renameTake(id, e.name);
      }
      useStore.getState().setProject(pv);
      if (as !== "take" && id) {
        const R = RACKS[target];
        const rack = R.read(useStore.getState()) ?? (await R.get()).ambient;
        if (rack.layers.length >= MAX_AMBIENT_LAYERS) {
          useStore.getState().setMessage(t.maxLayers);
        } else {
          const layer = makeLayer(e.name, defaultSource(as, id), { gain_db: -18 });
          const v = await R.set({ ...rack, layers: [...rack.layers, layer] });
          recordChange(target, rack, v.ambient, t.hLayerAdded(e.name), t.hStart);
          R.store(v.ambient);
        }
      }
      useStore.getState().setMessage(`${t.importedOk}: ${e.name}`);
    } catch (err) {
      fail(err);
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="samples">
      <div className="muted small">{t.samplesIntro}</div>
      <div className="panel">
        <div className="analysis-row">
          <strong>{t.samplesFolders}</strong>
          <button className="ghost small" onClick={() => void addFolder()}>
            + {t.samplesAddFolder}
          </button>
          <button className="ghost small" onClick={() => void scan()} disabled={scanning}>
            {scanning ? t.loading : `↻ ${t.samplesRescan}`}
          </button>
          <span className="muted small">
            {entries.length} {t.samplesFiles}
            {truncated ? ` · ${t.samplesTruncated}` : ""}
          </span>
          <label className="inline small" title={t.samplesTargetHint}>
            {t.samplesTarget}
            <select value={target} onChange={(e) => setTarget(e.target.value as RackKind)}>
              <option value="scapes">{t.scapesTitle}</option>
              <option value="ambient">{t.areaAmbient}</option>
            </select>
          </label>
        </div>
        <ul className="samples-folders">
          {folders.length === 0 && <li className="muted small">{t.samplesNoFolders}</li>}
          {folders.map((f) => (
            <li key={f}>
              <span className="small path">{f}</span>
              <button className="ghost small" disabled={!project || busy !== null || planZones(entries, f).length === 0} onClick={() => void instrumentFromFolder(f)} title={t.multisampleHint}>
                🎹 {t.multisampleMake} ({planZones(entries, f).length})
              </button>
              <button className="ghost small" onClick={() => void removeFolder(f)} title={t.delete}>
                ✕
              </button>
            </li>
          ))}
        </ul>
      </div>
      <div className="panel">
        <div className="analysis-row">
          <input className="grow" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t.samplesSearchHint} aria-label={t.search} />
          <select value={sort} onChange={(e) => setSort(e.target.value as SortKey)} aria-label={t.libSort}>
            <option value="name">{t.libSortName}</option>
            <option value="date">{t.libSortNewest}</option>
            <option value="duration">{t.samplesSortDuration}</option>
          </select>
          <span className="muted small">{list.length}</span>
        </div>
        {list.length === 0 && <div className="muted small">{t.samplesEmpty}</div>}
        <ul className="samples-list">
          {list.slice(0, shown).map((e) => (
            <li key={e.path} className={previewing === e.path ? "playing" : ""}>
              <button className="ghost small" onClick={() => void preview(e)} title={t.samplesPreview}>
                {previewing === e.path ? "■" : "▶"}
              </button>
              <span className="grow">
                <strong>{e.name}</strong>
                <span className="muted small">
                  {" "}
                  · {e.ext}
                  {e.duration_s !== null ? ` · ${formatTime(e.duration_s)}` : ""}
                  {e.sample_rate ? ` · ${(e.sample_rate / 1000).toFixed(1)} kHz` : ""}
                  {e.channels ? ` · ${e.channels === 1 ? "mono" : "stereo"}` : ""}
                </span>
                <span className="muted small path"> {e.path}</span>
              </span>
              <button className="ghost small" disabled={!project || busy !== null} onClick={() => void importAs(e, "take")} title={t.samplesAsTake}>
                + {t.take}
              </button>
              <button className="ghost small" disabled={!project || busy !== null} onClick={() => void importAs(e, "loop")} title={t.samplesAsLoop}>
                + {t.kindLoop}
              </button>
              <button className="ghost small" disabled={!project || busy !== null} onClick={() => void importAs(e, "sampler")} title={t.samplesAsSampler}>
                + {t.kindSampler}
              </button>
              <button className="ghost small" disabled={!project || busy !== null} onClick={() => void importAs(e, "cloud")} title={t.samplesAsCloud}>
                + {t.kindCloud}
              </button>
            </li>
          ))}
        </ul>
        {list.length > shown && (
          <button className="ghost" onClick={() => setShown((n) => n + PAGE)}>
            {t.samplesMore} ({list.length - shown})
          </button>
        )}
      </div>
    </section>
  );
}
