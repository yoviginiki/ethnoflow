import { type NyControl, type NyPlugin, type NyValue } from "./api";
import { messages } from "./i18n";
import { useStore } from "./store";

export type NyValues = Record<string, number | string>;

const isText = (c: NyControl) => c.kind === "string" || c.kind === "text" || c.kind === "file";

/** The header defaults of a plug-in as a values map. */
export function nyDefaults(p: NyPlugin): NyValues {
  const v: NyValues = {};
  for (const c of p.controls) v[c.var] = isText(c) ? c.default_text : c.initial;
  return v;
}

/** The values map as the shell wants it (every control, missing ones at their default). */
export function nyValues(p: NyPlugin, v: NyValues): NyValue[] {
  return p.controls.map((c) => ({ var: c.var, value: v[c.var] ?? (isText(c) ? c.default_text : c.initial) }));
}

/** A9: the controls of a Nyquist plug-in, generated from its header (sliders, fields, choices, text). */
export function NyControls({ plugin, values, onChange }: { plugin: NyPlugin; values: NyValues; onChange: (v: NyValues) => void }) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const setV = (c: NyControl, v: number | string) => onChange({ ...values, [c.var]: v });
  if (plugin.controls.length === 0) return <div className="muted small">{t.nyNoControls}</div>;
  return (
    <>
      {plugin.controls.map((c) => {
        const v = values[c.var];
        const num = typeof v === "number" ? v : Number(v) || 0;
        return (
          <label className="inline small" key={c.var} style={{ marginRight: 12 }}>
            {c.label}
            {c.kind === "choice" ? (
              <select value={num} onChange={(e) => setV(c, Number(e.target.value))}>
                {c.choices.map((ch, i) => (
                  <option key={i} value={i}>
                    {ch}
                  </option>
                ))}
              </select>
            ) : c.kind === "int" || c.kind === "float" ? (
              <>
                <input
                  type="range"
                  min={c.min ?? 0}
                  max={c.max ?? 1}
                  step={c.kind === "int" ? 1 : Math.max(0.001, ((c.max ?? 1) - (c.min ?? 0)) / 200)}
                  value={num}
                  onChange={(e) => setV(c, Number(e.target.value))}
                />
                <input type="number" min={c.min ?? undefined} max={c.max ?? undefined} step={c.kind === "int" ? 1 : "any"} value={num} onChange={(e) => setV(c, Number(e.target.value))} style={{ width: 80 }} />
                <span className="muted">{c.text}</span>
              </>
            ) : c.kind === "int_text" || c.kind === "float_text" || c.kind === "time" ? (
              <>
                <input type="number" min={c.min ?? undefined} max={c.max ?? undefined} step={c.kind === "int_text" ? 1 : "any"} value={num} onChange={(e) => setV(c, Number(e.target.value))} style={{ width: 100 }} />
                <span className="muted">{c.text}</span>
              </>
            ) : (
              <input type="text" value={typeof v === "string" ? v : String(v ?? "")} onChange={(e) => setV(c, e.target.value)} style={{ width: 200 }} />
            )}
          </label>
        );
      })}
    </>
  );
}
