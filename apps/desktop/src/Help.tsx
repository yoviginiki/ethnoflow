import { useMemo, useState } from "react";
import guideBg from "../../../docs/guide/bg.md?raw";
import guideEn from "../../../docs/guide/en.md?raw";
import { parseMarkdown, sections } from "./md";
import { BlockView } from "./MdView";
import { messages } from "./i18n";
import { useStore } from "./store";

/**
 * Help: the user guide (docs/guide/bg.md, en.md) rendered in the app, with a table of
 * contents and search across sections. The same files are the documentation in the
 * repository, so there is one place to keep up to date.
 */
export function Help() {
  const lang = useStore((s) => s.prefs.lang);
  const t = messages[lang];
  const [query, setQuery] = useState("");
  const [active, setActive] = useState<string | null>(null);
  const secs = useMemo(() => sections(parseMarkdown(lang === "bg" ? guideBg : guideEn)), [lang]);
  const q = query.trim().toLowerCase();
  const shown = q ? secs.filter((s) => s.text.includes(q)) : active ? secs.filter((s) => s.id === active || s.level === 1) : secs;

  return (
    <section className="help">
      <div className="analysis-row">
        <input className="grow" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t.helpSearchHint} aria-label={t.search} autoFocus />
        {q && <span className="muted small">{shown.length}</span>}
      </div>
      <div className="help-layout">
        <nav className="panel help-toc" aria-label={t.helpContents}>
          <button className={`ghost small ${active === null ? "on" : ""}`} onClick={() => setActive(null)}>
            {t.helpAll}
          </button>
          {secs
            .filter((s) => s.level === 2)
            .map((s) => (
              <button key={s.id} className={`ghost small ${active === s.id ? "on" : ""}`} onClick={() => setActive(active === s.id ? null : s.id)}>
                {s.title}
              </button>
            ))}
        </nav>
        <div className="panel help-body">
          {shown.length === 0 && <div className="muted">{t.helpNoMatch}</div>}
          {shown.map((s) => (
            <div key={s.id} className="help-section">
              {s.title && (s.level === 1 ? <h2 id={s.id}>{s.title}</h2> : <h3 id={s.id}>{s.title}</h3>)}
              {s.blocks.map((b, i) => (
                <BlockView key={i} b={b} />
              ))}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
