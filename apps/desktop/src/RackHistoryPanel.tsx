import { useCallback, useEffect } from "react";
import type { Ambient } from "./api";
import { isTypingTarget } from "./format";
import { messages } from "./i18n";
import { RACKS, type RackKind } from "./rackAdapter";
import { canRedo, canUndo, jumpHistory, redoHistory, undoHistory, useHistoryStore } from "./rackHistory";
import { useStore } from "./store";

interface Props {
  kind: RackKind;
  /** Applies a rack from the history without recording a new step. */
  apply: (rack: Ambient) => void;
}

/**
 * Undo / Redo and the list of steps for one rack (the owner's "history of changes,
 * a timeline to go back and forth when it gets messed up"). Ctrl+Z / Ctrl+Y work while the
 * rack's screen is open (not inside a text field).
 */
export function RackHistoryPanel({ kind, apply }: Props) {
  const t = messages[useStore((s) => s.prefs.lang)];
  const h = useHistoryStore((s) => s.byRack[kind]);
  const set = useHistoryStore((s) => s.set);
  const R = RACKS[kind];

  const go = useCallback(
    (r: { history: typeof h; rack: Ambient } | null) => {
      if (!r) return;
      set(kind, r.history);
      apply(r.rack);
    },
    [apply, kind, set],
  );
  const undo = useCallback(() => go(undoHistory(useHistoryStore.getState().byRack[kind])), [go, kind]);
  const redo = useCallback(() => go(redoHistory(useHistoryStore.getState().byRack[kind])), [go, kind]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || isTypingTarget(e.target)) return;
      const k = e.key.toLowerCase();
      if (k === "z" && !e.shiftKey) {
        e.preventDefault();
        undo();
      } else if (k === "y" || (k === "z" && e.shiftKey)) {
        e.preventDefault();
        redo();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [undo, redo]);

  const n = h.entries.length;
  return (
    <details className="panel history" data-coach={`${R.kind}-history`}>
      <summary>
        <span className="analysis-row">
          <strong>{t.rackHistoryTitle}</strong>
          <button className="ghost small" disabled={!canUndo(h)} onClick={(e) => (e.preventDefault(), undo())} title={`${t.historyUndo} (Ctrl+Z)`}>
            ↶ {t.historyUndo}
          </button>
          <button className="ghost small" disabled={!canRedo(h)} onClick={(e) => (e.preventDefault(), redo())} title={`${t.historyRedo} (Ctrl+Y)`}>
            ↷ {t.historyRedo}
          </button>
          <span className="muted small">{n === 0 ? t.historyEmpty : t.historyStep(h.index + 1, n)}</span>
        </span>
      </summary>
      <div className="muted small">{t.historyHint}</div>
      {n > 0 && (
        <ol className="history-list" reversed>
          {h.entries
            .map((e, i) => ({ e, i }))
            .reverse()
            .map(({ e, i }) => (
              <li key={`${i}-${e.at}`} className={i === h.index ? "current" : i > h.index ? "future" : ""}>
                <span className="mono-num small">{new Date(e.at).toLocaleTimeString()}</span> {e.label}
                {i !== h.index && (
                  <button className="ghost small" onClick={() => go(jumpHistory(h, i))}>
                    {t.historyJump}
                  </button>
                )}
              </li>
            ))}
        </ol>
      )}
    </details>
  );
}
