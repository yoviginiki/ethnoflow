import { useEffect, useMemo, useState } from "react";
import { api, describeError, type InstrumentSummary, MAX_EXTRAS, type ExtraKind } from "./api";
import { newExtraPart } from "./extraParts";
import { messages } from "./i18n";
import { CAT_LABEL, CATS, instrumentName, type Cat } from "./InstrumentDesigner";
import { DEFAULT_RHYTHM, useStore } from "./store";

type Part = "drums" | "bass" | "ostinato" | "arp";
const EXTRA_OF: Partial<Record<Cat, ExtraKind>> = { tapan: "tapan", chordpad: "chord_pad", lead: "lead" };

/**
 * The instrument list of the Rhythm studio (owner 1.11 evening: "choose on the left what
 * to add"): every instrument preset by category, ▶ auditions it with its own pattern at
 * the project tempo, "+" puts it on a part (drums → drums, bass → bass, synth → ostinato
 * or arp) or adds it as a rack part (tapan, chord pad, lead).
 */
export function InstrumentPicker({ onPick }: { onPick?: (id: string) => void }) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const [list, setList] = useState<InstrumentSummary[]>([]);
  const [cat, setCat] = useState<Cat | null>(null);
  const [query, setQuery] = useState("");
  const [auditioning, setAuditioning] = useState<string | null>(null);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  useEffect(() => {
    api
      .instrumentList(cat, query.trim() || null)
      .then(setList)
      .catch(() => setList([]));
  }, [cat, query]);
  useEffect(
    () => () => {
      void api.instrumentAuditionStop().catch(() => undefined);
    },
    [],
  );
  const shown = useMemo(() => list, [list]);

  const audition = async (p: InstrumentSummary) => {
    if (auditioning === p.id) {
      await api.instrumentAuditionStop().catch(() => undefined);
      setAuditioning(null);
      return;
    }
    try {
      const preset = await api.instrumentGet(p.id);
      await api.instrumentAuditionStart(preset.voice, s.prefs.selectedOutput, null, -6);
      setAuditioning(p.id);
    } catch (e) {
      fail(e);
    }
  };
  const assign = (p: InstrumentSummary, part: Part) => {
    const r = useStore.getState().prefs.rhythm ?? DEFAULT_RHYTHM;
    s.setPref("rhythm", { ...r, instruments: { ...(r.instruments ?? {}), [part]: p.id }, [part]: { ...r[part], on: true } });
    s.setMessage(`${t.igUsed}: ${instrumentName(p, t)}`);
    onPick?.(p.id);
  };
  const addExtra = (p: InstrumentSummary, kind: ExtraKind) => {
    const r = useStore.getState().prefs.rhythm ?? DEFAULT_RHYTHM;
    const extras = r.extras ?? [];
    if (extras.length >= MAX_EXTRAS) {
      s.setMessage(t.rackFull);
      return;
    }
    s.setPref("rhythm", { ...r, extras: [...extras, { ...newExtraPart(kind, extras), instrument: p.id }] });
    s.setMessage(`${t.igAddedExtra}: ${instrumentName(p, t)}`);
    onPick?.(p.id);
  };

  return (
    <aside className="panel instrument-picker" aria-label={t.rhythmPicker}>
      <h3>{t.rhythmPicker}</h3>
      <div className="muted small">{t.rhythmPickerHint}</div>
      <div className="style-row">
        <button className={`ghost small ${cat === null ? "on" : ""}`} onClick={() => setCat(null)}>
          {t.libKindAll}
        </button>
        {CATS.map((c) => (
          <button key={c} className={`ghost small ${cat === c ? "on" : ""}`} onClick={() => setCat(cat === c ? null : c)}>
            {t[CAT_LABEL[c]]}
          </button>
        ))}
      </div>
      <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t.libSearchHint} aria-label={t.search} />
      <div className="sample-list">
        {shown.map((p) => {
          const c = p.category as Cat;
          const extra = EXTRA_OF[c];
          return (
            <div key={p.id} className={`sample-row ${auditioning === p.id ? "playing" : ""}`}>
              <button className={`ghost small ${auditioning === p.id ? "on" : ""}`} onClick={() => void audition(p)} title={t.igAudition} aria-label={t.igAudition}>
                {auditioning === p.id ? "■" : "▶"}
              </button>
              <span className="grow small" title={p.id}>
                {instrumentName(p, t)}
                {!p.factory && " ★"}
                <span className="muted"> · {t[CAT_LABEL[c] ?? "igCatSynth"]}</span>
              </span>
              {c === "drums" && (
                <button className="small" onClick={() => assign(p, "drums")} title={t.rhythmAddTo}>
                  + {t.igPartDrums}
                </button>
              )}
              {c === "bass" && (
                <button className="small" onClick={() => assign(p, "bass")} title={t.rhythmAddTo}>
                  + {t.igPartBass}
                </button>
              )}
              {c === "synth" && (
                <>
                  <button className="small" onClick={() => assign(p, "ostinato")} title={t.rhythmAddTo}>
                    + {t.igPartOstinato}
                  </button>
                  <button className="small" onClick={() => assign(p, "arp")} title={t.rhythmAddTo}>
                    + {t.igPartArp}
                  </button>
                </>
              )}
              {extra && (
                <button className="small" onClick={() => addExtra(p, extra)} title={t.rhythmAddTo}>
                  + {t.rhythmAddPart}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </aside>
  );
}
