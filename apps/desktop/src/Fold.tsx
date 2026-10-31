import type { ReactNode } from "react";
import { useStore } from "./store";

interface Props {
  /** Key under which the open state is remembered (prefs.studioOpen). */
  id: string;
  label: string;
  /** One-line read-only summary shown while folded ("Drone: playing · F♯2"). */
  chip?: string | null;
  children: ReactNode;
}

/**
 * A collapsible block for the secondary panels of "Play & record" (devices, keyboard,
 * drone, pad): folded by default so the transport and the take stay in view, with a
 * chip that says what the panel is doing. The open state persists per block.
 */
export function Fold({ id, label, chip, children }: Props) {
  const open = useStore((s) => (s.prefs.studioOpen ?? []).includes(id));
  const setPref = useStore((s) => s.setPref);
  return (
    <details
      className="fold"
      open={open}
      onToggle={(e) => {
        const o = e.currentTarget.open;
        if (o === open) return;
        const cur = useStore.getState().prefs.studioOpen ?? [];
        setPref("studioOpen", o ? [...cur, id] : cur.filter((x) => x !== id));
      }}
    >
      <summary>
        {label}
        {chip ? (
          <>
            : <b>{chip}</b>
          </>
        ) : null}
      </summary>
      {children}
    </details>
  );
}
