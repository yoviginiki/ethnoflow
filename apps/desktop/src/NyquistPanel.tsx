import { useCallback, useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, type EditOp, type NyAnalysis, type NyPlugin, type PlayerStatus } from "./api";
import { NyControls, nyDefaults, nyValues, type NyValues } from "./NyControls";
import { messages } from "./i18n";
import { useStore } from "./store";
import { stopPlaying } from "./takeActions";

interface Props {
  /** The selection, or the whole take. */
  region: () => [number, number];
  selection: [number, number] | null;
  cursor: number;
  busy: boolean;
  run: (op: EditOp) => Promise<void>;
  onPlaying: (p: { from: number; st: PlayerStatus } | null) => void;
  selectedOutput: string | null;
  fail: (e: unknown) => void;
  /** Select [a, b) in the editor (the generated range after "Generate"). */
  select: (a: number, b: number) => void;
}

type Values = NyValues;

/** A9: Audacity Nyquist plug-ins on the editor's selection — list, controls from the header, preview / apply / analyze. */
export function NyquistPanel({ region, selection, cursor, busy, run, onPlaying, selectedOutput, fail, select }: Props) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const [plugins, setPlugins] = useState<NyPlugin[]>([]);
  const [folder, setFolder] = useState("");
  const [sel, setSel] = useState<string | null>(null);
  const [values, setValues] = useState<Values>({});
  const [result, setResult] = useState<NyAnalysis | null>(null);
  const [code, setCode] = useState<string | null>(null);
  const plugin = plugins.find((p) => p.id === sel) ?? null;

  const refresh = useCallback(async () => {
    try {
      const [list, dir] = await Promise.all([api.nyList(), api.nyFolder()]);
      setPlugins(list);
      setFolder(dir);
    } catch (e) {
      fail(e);
    }
  }, [fail]);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const pick = (id: string) => {
    const p = plugins.find((x) => x.id === id);
    setSel(id);
    setResult(null);
    setCode(null);
    if (p) setValues(nyDefaults(p));
  };
  const target = (): [number, number] => {
    if (plugin?.kind === "generate") return selection ?? [cursor, cursor];
    return region();
  };
  const op = (): EditOp | null => {
    if (!plugin) return null;
    const r = target();
    return { op: "nyquist", a: r[0], b: r[1], plugin: plugin.id, values: nyValues(plugin, values) };
  };
  const preview = async () => {
    const o = op();
    if (!o) return;
    try {
      await stopPlaying().catch(() => undefined);
      const st = await api.editPreview(o, 4, selectedOutput);
      onPlaying({ from: target()[0], st });
    } catch (e) {
      fail(e);
    }
  };
  const apply = async () => {
    const o = op();
    if (!o || !plugin) return;
    if (plugin.kind !== "generate") {
      void run(o);
      return;
    }
    // Generate: insert, then select what came in so "▶ Selection" plays it and the sample export takes it.
    try {
      const before = await api.editStatus();
      await run(o);
      const after = await api.editStatus();
      if (before && after) {
        const r = target();
        const grown = after.seconds - before.seconds + (r[1] - r[0]);
        if (grown > 1e-6) select(r[0], r[0] + grown);
      }
    } catch (e) {
      fail(e);
    }
  };
  const saveSample = async () => {
    const r = selection ?? region();
    try {
      const out = await api.editExportWav(r[0], r[1], plugin ? plugin.name.replace(/^EthnoFlow: /, "") : "sample");
      const st = useStore.getState();
      const folders = st.prefs.sampleFolders ?? [];
      if (!folders.includes(out.folder)) st.setPref("sampleFolders", [...folders, out.folder]);
      st.setMessage(`${t.nySampleSaved}: ${out.path.split(/[\\/]/).pop()}`);
    } catch (e) {
      fail(e);
    }
  };
  const analyze = async () => {
    if (!plugin) return;
    const r = region();
    try {
      setResult(await api.nyAnalyze(plugin.id, nyValues(plugin, values), r[0], r[1]));
    } catch (e) {
      fail(e);
    }
  };
  const importFile = async () => {
    const f = await openDialog({ title: t.nyImport, multiple: false, filters: [{ name: "Nyquist", extensions: ["ny"] }] });
    if (!f) return;
    try {
      const p = await api.nyImport(String(f));
      await refresh();
      setSel(p.id);
      setValues(nyDefaults(p));
      useStore.getState().setMessage(`${t.nyImported}: ${p.name}`);
    } catch (e) {
      fail(e);
    }
  };
  const toggleCode = async () => {
    if (code !== null || !plugin) {
      setCode(null);
      return;
    }
    try {
      setCode(await api.nySource(plugin.id));
    } catch (e) {
      fail(e);
    }
  };
  const kindLabel = (k: string) => (k === "generate" ? t.nyKindGenerate : k === "analyze" ? t.nyKindAnalyze : k === "tool" ? t.nyKindTool : t.nyKindProcess);

  return (
    <section className="panel" aria-label={t.nyTitle}>
      <div className="analysis-row">
        <h3>{t.nyTitle}</h3>
        <select value={sel ?? ""} onChange={(e) => pick(e.target.value)} aria-label={t.nyTitle}>
          <option value="">{plugins.length ? "—" : t.nyEmpty}</option>
          {plugins.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name} · {kindLabel(p.kind)}{p.bundled ? ` · ${t.nyBundled}` : ""}
            </option>
          ))}
        </select>
        <button className="small" onClick={() => void refresh()}>{t.nyRefresh}</button>
        <button className="small" onClick={() => void importFile()}>{t.nyImport}</button>
        <button className="small" onClick={() => void api.nyOpenFolder().catch(fail)} title={folder}>
          {t.nyFolder}
        </button>
      </div>
      <div className="muted small">{t.nyHint}</div>
      {plugin && (
        <div className="fx-form">
          <div className="muted small">
            {plugin.info}
            {plugin.author ? ` — ${plugin.author}` : ""}
            {plugin.codetype.startsWith("sal") ? ` · ${t.nySal}` : ""}
          </div>
          <NyControls plugin={plugin} values={values} onChange={setValues} />
          <div className="analysis-row">
            {plugin.kind === "analyze" ? (
              <button className="small primary" onClick={() => void analyze()} disabled={busy}>
                {t.nyAnalyze}
              </button>
            ) : (
              <>
                {plugin.preview !== "disabled" && (
                  <button className="small" onClick={() => void preview()} disabled={busy}>
                    ▶ {plugin.kind === "generate" ? t.nyPreviewGen : t.fxPreview}
                  </button>
                )}
                <button className="small primary" onClick={() => void apply()} disabled={busy}>
                  ✓ {plugin.kind === "generate" ? t.nyGenerate : t.fxApply}
                </button>
              </>
            )}
            <button className="small" onClick={() => void saveSample()} disabled={busy} title={t.nyUseHint}>
              💾 {t.nySaveSample}
            </button>
            <button className="small" onClick={() => void toggleCode()}>
              {code !== null ? t.nyHideCode : t.nyShowCode}
            </button>
            <span className="muted small">{plugin.bundled ? t.nyBundled : plugin.file}</span>
          </div>
          {result && (
            <div className="small">
              {result.message && <pre className="ny-out">{result.message}</pre>}
              {result.labels.length > 0 && (
                <div>
                  <strong>{t.nyLabels}:</strong>{" "}
                  {result.labels.map((l, i) => (
                    <span key={i} className="badge" style={{ marginRight: 4 }}>
                      {l.start.toFixed(3)}{l.end !== null ? `–${l.end.toFixed(3)}` : ""} s {l.text}
                    </span>
                  ))}
                </div>
              )}
              {result.printed && (
                <pre className="ny-out muted">
                  {t.nyPrinted}: {result.printed}
                </pre>
              )}
              {result.returned_audio && <div className="muted">{t.nyReturnedAudio}</div>}
              {!result.message && !result.labels.length && !result.printed && !result.returned_audio && <div className="muted">{t.nyNothing}</div>}
            </div>
          )}
          <div className="muted small">{t.nyUseHint}</div>
          {code !== null && <pre className="ny-code">{code}</pre>}
        </div>
      )}
    </section>
  );
}
