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

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}
