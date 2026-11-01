import { useState } from "react";
import { TransportBar } from "./TransportBar";
import { FollowMap } from "./FollowMap";
import { messages } from "./i18n";
import { InstrumentDesigner } from "./InstrumentDesigner";
import { InstrumentPicker } from "./InstrumentPicker";
import { RhythmPanel } from "./RhythmPanel";
import { useStore } from "./store";
import { TimingPanel } from "./TimingPanel";

/**
 * The rhythm section as a studio (owner 1.11 evening: "choose on the left what to add,
 * edit each added part in its own window"): the instrument list on the left (▶ audition,
 * + onto a part), the parts in the middle, and the part's instrument editor on this
 * screen (✎ on a part opens it; a saved copy is wired back to the part).
 */
export function RhythmScreen() {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const project = s.project!;
  const [editing, setEditing] = useState<{ presetId: string; assign: (id: string) => void } | null>(null);
  return (
    <main className="compose">
      <section className="panel">
        <h2>{t.rhythmTitle}</h2>
        <div className="muted small">{t.rhythmStudioHint}</div>
        {!project.scale && <div className="muted small">{t.noScaleYet}</div>}
      </section>
      <TimingPanel />
      <section className="panel analysis-row">
        <strong>{t.transportTitle}</strong>
        <TransportBar />
        <FollowMap />
      </section>
      <div className="rhythm-studio">
        <InstrumentPicker />
        <div className="rhythm-parts">
          <RhythmPanel onEdit={(presetId, assign) => setEditing({ presetId, assign })} />
          {editing && (
            <section className="panel part-editor" data-coach="part-editor">
              <div className="panel-head">
                <h3>{t.rhythmEditPart}</h3>
                <span className="muted small">{t.rhythmEditHint}</span>
                <button className="ghost small" onClick={() => setEditing(null)} aria-label={t.close}>
                  ✕
                </button>
              </div>
              <InstrumentDesigner compact presetId={editing.presetId} onSaved={(id) => editing.assign(id)} />
            </section>
          )}
        </div>
      </div>
      <section className="panel analysis-row">
        <button className="ghost" onClick={() => s.setScreen("play")}>
          {t.toStudio}
        </button>
        <button className="ghost" onClick={() => s.setScreen("studio")}>
          {t.areaStudio}
        </button>
      </section>
    </main>
  );
}
