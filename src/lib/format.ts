/** Shared pt-BR formatting helpers for sizes, throughput, and durations. */

/** "7,30" — GB with comma decimal, no unit. */
export function gb(bytes: number, digits = 2): string {
  return (bytes / 1e9).toFixed(digits).replace(".", ",");
}

/** "7,30 gb" | "412 mb" — picks the unit. */
export function size(bytes: number): string {
  if (bytes >= 1e9) return `${gb(bytes)} gb`;
  return `${Math.round(bytes / 1e6)} mb`;
}

/** "12.4" — MB/s value for the metrics row. */
export function mbps(bytesPerSecond: number): string {
  return (bytesPerSecond / 1e6).toFixed(1);
}

/** "≈ 5 min 42 s" */
export function eta(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "—";
  if (seconds < 60) return `≈ ${Math.round(seconds)} s`;
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  if (m < 60) return `≈ ${m} min ${s.toString().padStart(2, "0")} s`;
  const h = Math.floor(m / 60);
  return `≈ ${h} h ${m % 60} min`;
}

/** "há 6 min" | "há 3 dias" — relative past time from an RFC3339 stamp. */
export function ago(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "—";
  const s = Math.max(0, (Date.now() - t) / 1000);
  if (s < 60) return "agora";
  const m = Math.floor(s / 60);
  if (m < 60) return `há ${m} min`;
  const h = Math.floor(m / 60);
  if (h < 24) return `há ${h} h`;
  const d = Math.floor(h / 24);
  if (d === 1) return "há 1 dia";
  if (d < 30) return `há ${d} dias`;
  const mo = Math.floor(d / 30);
  return mo === 1 ? "há 1 mês" : `há ${mo} meses`;
}

const ROMANS = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII"];

/** 0-based index → "I", "II", … (falls back to arabic past XII). */
export function roman(index: number): string {
  return ROMANS[index] ?? String(index + 1);
}

/** "128k" — context length shorthand. */
export function contextK(tokens: number): string {
  return tokens >= 1000 ? `${Math.round(tokens / 1000)}k` : String(tokens);
}
