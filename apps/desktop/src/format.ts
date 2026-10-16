// Pure display helpers (unit-tested; no Tauri dependency).

export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) seconds = 0;
  const m = Math.floor(seconds / 60);
  const s = seconds - m * 60;
  const whole = Math.floor(s);
  const tenths = Math.floor((s - whole) * 10);
  return `${m}:${String(whole).padStart(2, "0")}.${tenths}`;
}

export function formatDb(db: number): string {
  if (db <= -120) return "-∞ dB";
  return `${db.toFixed(1)} dB`;
}

export function linearToDb(x: number): number {
  if (x <= 0) return -120;
  return Math.max(-120, 20 * Math.log10(x));
}

/** Map a dBFS value onto a 0..1 meter position with a -60 dB floor. */
export function meterFraction(db: number, floorDb = -60): number {
  if (db <= floorDb) return 0;
  if (db >= 0) return 1;
  return (db - floorDb) / -floorDb;
}

/** Parse a beat grouping like "2+2+3" (spaces allowed). Empty → []. Invalid → null. */
export function parseGrouping(text: string): number[] | null {
  const t = text.trim();
  if (t === "") return [];
  const parts = t.split("+").map((p) => p.trim());
  if (parts.some((p) => !/^[1-9]\d*$/.test(p))) return null;
  return parts.map(Number);
}

export function formatGrouping(g: number[]): string {
  return g.join("+");
}

/** Display name of a take: its label, or "<prefix> <number>". */
export function takeName(take: { label: string | null; number: number }, prefix: string): string {
  return take.label && take.label.trim() !== "" ? take.label : `${prefix} ${take.number}`;
}

/** Keyboard shortcuts must not fire while typing in a field. */
export function isTypingTarget(el: EventTarget | null): boolean {
  if (!el || typeof (el as HTMLElement).tagName !== "string") return false;
  const h = el as HTMLElement;
  const tag = h.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || h.isContentEditable === true;
}

// ---------- pitch helpers (mirror crates/music-core; pure, unit-tested) ----------

export function hzToMidi(hz: number, a4 = 440): number {
  return 69 + 12 * Math.log2(hz / a4);
}

export function midiToHz(midi: number, a4 = 440): number {
  return a4 * Math.pow(2, (midi - 69) / 12);
}

export interface NoteParts {
  /** 0 = C … 11 = B */
  pc: number;
  midi: number;
  octave: number;
  /** −50 ≤ cents < 50 */
  cents: number;
}

/** Nearest tempered note to a fractional MIDI number. */
export function noteParts(midiFrac: number): NoteParts {
  let midi = Math.round(midiFrac);
  let cents = (midiFrac - midi) * 100;
  if (cents >= 50) {
    midi += 1;
    cents -= 100;
  }
  const pc = ((midi % 12) + 12) % 12;
  return { pc, midi, octave: Math.floor(midi / 12) - 1, cents };
}

/** Signed distance (cents) from the nearest degree of `scale`, octave-independent. */
export function centsFromScale(scale: { tonic_pc: number; degrees_cents: number[] }, midiFrac: number): number {
  const c = (((midiFrac - scale.tonic_pc) * 100) % 1200 + 1200) % 1200;
  let best = Infinity;
  for (const d of scale.degrees_cents) {
    let off = c - d;
    if (off > 600) off -= 1200;
    else if (off < -600) off += 1200;
    if (Math.abs(off) < Math.abs(best)) best = off;
  }
  return best;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}
