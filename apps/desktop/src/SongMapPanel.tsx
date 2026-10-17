import { useEffect, useMemo, useRef, useState } from "react";
import { api, describeError, type Section, type SongMap } from "./api";
import { messages, type StringMessageKey } from "./i18n";
import { useStore } from "./store";

export const ROLE_LABEL: Record<string, StringMessageKey> = {
  intro: "roleIntro",
  establish: "roleEstablish",
  build: "roleBuild",
  drop: "roleDrop",
  variation: "roleVariation",
  break: "roleBreak",
  bridge: "roleBridge",
  outro: "roleOutro",
  custom: "roleCustom",
};
const ROLES = Object.keys(ROLE_LABEL);
const ROLE_COLOR: Record<string, string> = {
  intro: "#4d6b8a",
  establish: "#4f7f6a",
  build: "#8a7a3d",
  drop: "#a04f4f",
  variation: "#8a5a8a",
  break: "#4f6f8a",
  bridge: "#6a6a6a",
  outro: "#4d6b8a",
  custom: "#5a5a5a",
};

/** Pure helpers over the map (ticks ↔ seconds at one tempo segment chain). */
export function tickToSeconds(m: SongMap, tick: number): number {
  let secs = 0;
  for (let i = 0; i < m.tempo_map.length; i++) {
    const seg = m.tempo_map[i];
    if (seg.tick >= tick) break;
    const end = Math.min(m.tempo_map[i + 1]?.tick ?? Infinity, tick);
    secs += ((end - seg.tick) / m.ppq) * (60 / seg.bpm);
  }
  return secs;
}
export function secondsToTick(m: SongMap, seconds: number): number {
  let left = Math.max(0, seconds);
  let tick = 0;
  for (let i = 0; i < m.tempo_map.length; i++) {
    const seg = m.tempo_map[i];
    const next = m.tempo_map[i + 1]?.tick;
    const spt = 60 / seg.bpm / m.ppq;
    if (next !== undefined && left >= (next - seg.tick) * spt) {
      left -= (next - seg.tick) * spt;
      tick = next;
    } else return seg.tick + Math.floor(left / spt);
  }
  return tick;
}
export function barTicks(m: SongMap, tick: number): number {
  const seg = [...m.meter_map].reverse().find((x) => x.tick <= tick) ?? m.meter_map[0];
  return (m.ppq * 4 * seg.numerator) / seg.unit;
}
export function tickToBar(m: SongMap, tick: number): number {
  let bar = 0;
  for (let i = 0; i < m.meter_map.length; i++) {
    const seg = m.meter_map[i];
    const end = m.meter_map[i + 1]?.tick ?? Infinity;
    const bt = (m.ppq * 4 * seg.numerator) / seg.unit;
    if (tick < end) return bar + Math.floor((tick - seg.tick) / bt) + 1;
    bar += Math.floor((end - seg.tick) / bt);
  }
  return bar + 1;
}
export function barToTick(m: SongMap, bar: number): number {
  let remaining = Math.max(0, bar - 1);
  for (let i = 0; i < m.meter_map.length; i++) {
    const seg = m.meter_map[i];
    const end = m.meter_map[i + 1]?.tick;
    const bt = (m.ppq * 4 * seg.numerator) / seg.unit;
    if (end !== undefined) {
      const n = Math.floor((end - seg.tick) / bt);
      if (remaining >= n) {
        remaining -= n;
        continue;
      }
    }
    return seg.tick + remaining * bt;
  }
  return 0;
}

interface Props {
  /** Current transport position in seconds (arrangement transport), if any. */
  headSeconds?: number | null;
  onSeek?: (seconds: number) => void;
  compact?: boolean;
}

/**
 * Song map (P3 Phase 1 / P2 Phase C): the song's sections on a bar ruler, editable by
 * whole bars, with the transport position. The map lives in the project (ticks, tempo
 * map, meter map); markers in seconds are kept in step by the shell.
 */
export function SongMapPanel({ headSeconds = null, onSeek, compact = false }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project;
  const map = project?.song_map ?? null;
  const [selected, setSelected] = useState<string | null>(null);
  const [zoom, setZoom] = useState(compact ? 18 : 28); // px per bar
  const [saving, setSaving] = useState(false);
  const dragRef = useRef<{ id: string; edge: "start" | "end"; startX: number; origin: number } | null>(null);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));
  const hasTiming = !!project && project.timing.bpm !== null && project.timing.numerator !== null;

  const commit = async (next: SongMap) => {
    setSaving(true);
    try {
      const pv = await api.setSongMap(next);
      useStore.getState().setProject(pv);
    } catch (e) {
      fail(e);
    } finally {
      setSaving(false);
    }
  };
  const patchSection = (id: string, patch: Partial<Section>) => {
    if (!map) return;
    void commit({ ...map, sections: map.sections.map((x) => (x.id === id ? { ...x, ...patch } : x)) });
  };
  const addSection = (role: string) => {
    if (!map) return;
    const bt = barTicks(map, 0);
    const last = map.sections[map.sections.length - 1];
    const start = last ? last.end_tick : 0;
    const id = `s${Date.now().toString(36)}`;
    const sec: Section = { id, role, name: t[ROLE_LABEL[role]], start_tick: start, end_tick: start + 8 * bt, energy: 0.5, density: 0.5, tension: 0.5, roles: [], transition_out: null, locked: false, notes: "" };
    void commit({ ...map, sections: [...map.sections, sec] });
    setSelected(id);
  };
  const splitAtHead = () => {
    if (!map || headSeconds === null) return;
    const tick = secondsToTick(map, headSeconds);
    const bt = barTicks(map, tick);
    const at = Math.floor(tick / bt) * bt;
    const i = map.sections.findIndex((x) => x.start_tick < at && at < x.end_tick);
    if (i < 0) return;
    const left = map.sections[i];
    const right: Section = { ...left, id: `${left.id}b${at}`, start_tick: at, locked: false };
    const next = [...map.sections];
    next.splice(i, 1, { ...left, end_tick: at }, right);
    void commit({ ...map, sections: next });
  };
  const removeSection = (id: string) => {
    if (!map) return;
    void commit({ ...map, sections: map.sections.filter((x) => x.id !== id) });
    if (selected === id) setSelected(null);
  };
  const mergeNext = (id: string) => {
    if (!map) return;
    const i = map.sections.findIndex((x) => x.id === id);
    if (i < 0 || i + 1 >= map.sections.length) return;
    const next = [...map.sections];
    next[i] = { ...next[i], end_tick: next[i + 1].end_tick };
    next.splice(i + 1, 1);
    void commit({ ...map, sections: next });
  };

  // Drag a boundary by whole bars; commit on release.
  const [dragPreview, setDragPreview] = useState<SongMap | null>(null);
  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      const d = dragRef.current;
      if (!d || !map) return;
      const bars = Math.round((e.clientX - d.startX) / zoom);
      const bt = barTicks(map, d.origin);
      const tick = Math.max(0, d.origin + bars * bt);
      const i = map.sections.findIndex((x) => x.id === d.id);
      if (i < 0) return;
      const next = map.sections.map((x) => ({ ...x }));
      if (d.edge === "end") {
        const min = next[i].start_tick + bt;
        const max = next[i + 1] ? next[i + 1].end_tick - bt : Infinity;
        next[i].end_tick = Math.min(Math.max(tick, min), max);
        if (next[i + 1]) next[i + 1].start_tick = next[i].end_tick;
      } else {
        const max = next[i].end_tick - bt;
        const min = next[i - 1] ? next[i - 1].start_tick + bt : 0;
        next[i].start_tick = Math.min(Math.max(tick, min), max);
        if (next[i - 1]) next[i - 1].end_tick = next[i].start_tick;
      }
      setDragPreview({ ...map, sections: next });
    };
    const onUp = () => {
      if (dragRef.current && dragPreview) void commit(dragPreview);
      dragRef.current = null;
      setDragPreview(null);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [map, zoom, dragPreview]);

  const shown = dragPreview ?? map;
  const endTick = shown ? Math.max(shown.sections[shown.sections.length - 1]?.end_tick ?? 0, 16 * barTicks(shown, 0)) : 0;
  const totalBars = shown ? tickToBar(shown, endTick) : 0;
  const sel = shown?.sections.find((x) => x.id === selected) ?? null;
  const headTick = shown && headSeconds !== null ? secondsToTick(shown, headSeconds) : null;
  const headBar = shown && headTick !== null ? tickToBar(shown, headTick) : null;
  const nextSection = useMemo(() => (shown && headTick !== null ? shown.sections.find((x) => x.start_tick > headTick) ?? null : null), [shown, headTick]);
  const barsToNext = nextSection && headTick !== null && shown ? tickToBar(shown, nextSection.start_tick) - tickToBar(shown, headTick) : null;

  if (!project || !shown) return null;
  const pxPerBar = zoom;
  const xOf = (tick: number) => ((tickToBar(shown, tick) - 1) * pxPerBar);

  return (
    <div className={`songmap ${compact ? "compact" : ""}`}>
      <div className="analysis-row">
        <strong>{t.songMapTitle}</strong>
        {!hasTiming && <span className="badge warn">{t.songMapNoTiming}</span>}
        {project.song_map && <span className="muted small">{project.timing.bpm} BPM · {project.timing.numerator}/{project.timing.unit}</span>}
        <label className="inline small">
          {t.songMapAdd}
          <select value="" onChange={(e) => e.target.value && addSection(e.target.value)} disabled={saving}>
            <option value="">…</option>
            {ROLES.map((r) => (
              <option key={r} value={r}>
                {t[ROLE_LABEL[r]]}
              </option>
            ))}
          </select>
        </label>
        <button className="ghost small" onClick={splitAtHead} disabled={headSeconds === null || saving} title={t.songMapSplitHint}>
          ✂ {t.songMapSplit}
        </button>
        <label className="inline small">
          🔍
          <input type="range" min={6} max={80} value={zoom} onChange={(e) => setZoom(Number(e.target.value))} style={{ width: 90 }} />
        </label>
        {headBar !== null && (
          <span className="mono-num small">
            {t.barShort} {headBar}
            {barsToNext !== null && nextSection ? ` · ${t.songMapNext}: ${nextSection.name} ${t.songMapIn} ${barsToNext} ${t.barsWord}` : ""}
          </span>
        )}
      </div>
      <div className="songmap-scroll">
        <div className="songmap-ruler" style={{ width: totalBars * pxPerBar }}>
          {Array.from({ length: totalBars }, (_, i) => i + 1)
            .filter((b) => pxPerBar >= 24 || b % 4 === 1)
            .map((b) => (
              <span key={b} className="songmap-bar" style={{ left: (b - 1) * pxPerBar }}>
                {b}
              </span>
            ))}
        </div>
        <div className="songmap-lane" style={{ width: totalBars * pxPerBar }} onClick={(e) => {
          if (!onSeek) return;
          const rect = (e.currentTarget as HTMLDivElement).getBoundingClientRect();
          const bar = Math.floor((e.clientX - rect.left) / pxPerBar) + 1;
          onSeek(tickToSeconds(shown, barToTick(shown, bar)));
        }}>
          {shown.sections.map((sec) => (
            <div
              key={sec.id}
              className={`songmap-section ${selected === sec.id ? "selected" : ""} ${sec.locked ? "locked" : ""}`}
              style={{ left: xOf(sec.start_tick), width: Math.max(4, xOf(sec.end_tick) - xOf(sec.start_tick)), background: ROLE_COLOR[sec.role] ?? ROLE_COLOR.custom }}
              onClick={(e) => {
                e.stopPropagation();
                setSelected(sec.id);
              }}
              title={`${sec.name} · ${t[ROLE_LABEL[sec.role]]} · ${tickToBar(shown, sec.start_tick)}–${tickToBar(shown, sec.end_tick) - 1}`}
            >
              <span className="songmap-name">{sec.name}</span>
              <span className="songmap-energy" style={{ height: `${Math.round(sec.energy * 100)}%` }} aria-hidden="true" />
              {!sec.locked && <span className="songmap-handle left" onMouseDown={(e) => { e.stopPropagation(); dragRef.current = { id: sec.id, edge: "start", startX: e.clientX, origin: sec.start_tick }; }} />}
              {!sec.locked && <span className="songmap-handle right" onMouseDown={(e) => { e.stopPropagation(); dragRef.current = { id: sec.id, edge: "end", startX: e.clientX, origin: sec.end_tick }; }} />}
            </div>
          ))}
          {headTick !== null && <div className="songmap-head" style={{ left: xOf(headTick) }} aria-hidden="true" />}
        </div>
      </div>
      {sel && (
        <div className="analysis-row songmap-inspector">
          <input value={sel.name} maxLength={40} onChange={(e) => patchSection(sel.id, { name: e.target.value })} aria-label={t.presetName} />
          <select value={sel.role} onChange={(e) => patchSection(sel.id, { role: e.target.value })}>
            {ROLES.map((r) => (
              <option key={r} value={r}>
                {t[ROLE_LABEL[r]]}
              </option>
            ))}
          </select>
          <span className="mono-num small">
            {t.barShort} {tickToBar(shown, sel.start_tick)}–{tickToBar(shown, sel.end_tick) - 1}
          </span>
          <label className="inline small" title={t.songMapEnergyHint}>
            {t.songMapEnergy}
            <input type="range" min={0} max={1} step={0.05} value={sel.energy} onChange={(e) => patchSection(sel.id, { energy: Number(e.target.value) })} style={{ width: 80 }} />
          </label>
          <label className="inline small" title={t.songMapDensityHint}>
            {t.songMapDensity}
            <input type="range" min={0} max={1} step={0.05} value={sel.density} onChange={(e) => patchSection(sel.id, { density: Number(e.target.value) })} style={{ width: 80 }} />
          </label>
          <button className={`ghost small ${sel.locked ? "on" : ""}`} onClick={() => patchSection(sel.id, { locked: !sel.locked })} title={t.songMapLockHint}>
            {sel.locked ? "🔒" : "🔓"}
          </button>
          <button className="ghost small" onClick={() => mergeNext(sel.id)} title={t.songMapMerge}>
            ⇥
          </button>
          <button className="ghost small" onClick={() => removeSection(sel.id)} title={t.removeLayer}>
            ✕
          </button>
        </div>
      )}
      <div className="muted small">{t.songMapHint}</div>
    </div>
  );
}
