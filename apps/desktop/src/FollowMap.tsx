import { messages } from "./i18n";
import { ROLE_LABEL } from "./SongMapPanel";
import { DEFAULT_RHYTHM, useStore } from "./store";

/**
 * Section follower switch + badge (P2 E / P3 first slice): "Follow the map" lets the song
 * map's sections shape the rhythm parts and the background buses; the badge says which
 * section is active and exactly what it does right now, so nothing happens silently.
 */
export function FollowMap({ compact = false }: { compact?: boolean }) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const r = useStore((s) => s.prefs.rhythm ?? DEFAULT_RHYTHM);
  const setPref = useStore((s) => s.setPref);
  const section = useStore((s) => s.rhythmStatus?.section ?? null);
  const transportSection = useStore((s) => s.transport?.section ?? null);
  const project = useStore((s) => s.project);
  const on = r.follow_map ?? true;
  const hasSections = (project?.song_map.sections.length ?? 0) > 0;
  const roleLabel = (role: string) => (role in ROLE_LABEL ? t[ROLE_LABEL[role]] : role);
  const yesNo = (v: boolean) => (v ? "✓" : "✗");
  const db = (v: number) => (Math.abs(v) < 0.05 ? "0" : `${v > 0 ? "+" : ""}${v.toFixed(0)}`) + " dB";
  return (
    <span className={"follow-map" + (compact ? " compact" : "")} title={t.followMapHint}>
      <label className="inline small">
        <input type="checkbox" checked={on} onChange={(e) => setPref("rhythm", { ...r, follow_map: e.target.checked })} />
        🗺 {t.followMap}
      </label>
      {!on && <span className="muted small">{t.followOff}</span>}
      {on && !hasSections && <span className="muted small">{t.followNoSections}</span>}
      {on && hasSections && section && (
        <span className="small follow-badge">
          <strong>
            {section.index + 1}. {section.name}
          </strong>{" "}
          <span className="muted">
            ({roleLabel(section.role)}, {Math.round(section.progress * 100)}%)
          </span>
          {!compact && (
            <>
              {" · "}
              {t.drumsTitle} {yesNo(section.effect.drums)} · {t.layerBass} {yesNo(section.effect.bass)} · {t.layerOstinato} {yesNo(section.effect.ostinato)} · {t.arpTitle} {yesNo(section.effect.arp)}
              {" · "}
              {t.followRhythmBus} {db(section.effect.bus_db[3])}, {t.followPadBus} {db(section.effect.bus_db[1])}
              {section.effect.density_scale < 0.95 && <> · {t.followDensity} ×{section.effect.density_scale.toFixed(2)}</>}
              {section.effect.seed_offset > 0 && <> · {t.followVariation}</>}
            </>
          )}
        </span>
      )}
      {on && hasSections && !section && transportSection && (
        <span className="small muted">
          {t.followSection}: {transportSection}
        </span>
      )}
      {on && hasSections && !section && !transportSection && <span className="muted small">{t.followIdle}</span>}
    </span>
  );
}
