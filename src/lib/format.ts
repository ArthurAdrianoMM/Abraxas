/** Locale-aware formatting for sizes, throughput, and durations.
 *
 *  These were pt-BR-only helpers (hardcoded comma decimals and "há 6 min").
 *  They are now built per locale by [`createFormat`], which components reach
 *  through `useFormat()` in `lib/i18n` — a hook, so switching the language
 *  re-renders and reformats in the same pass as the translated strings.
 *
 *  Units (gb, mb, min, s) are left untranslated on purpose: they read the same
 *  in both supported locales, and the design uses them as typographic marks
 *  rather than words. */

import type { Locale } from "./tauri/bindings";

export interface Format {
  /** Raw number with the locale's decimal mark. For quantities that aren't
   *  plain byte counts — VRAM, for one, is reported in mebibytes. */
  decimal: (value: number, digits: number) => string;
  /** "7.30" / "7,30" — GB with the locale's decimal mark, no unit. */
  gb: (bytes: number, digits?: number) => string;
  /** "7.30 gb" | "412 mb" — picks the unit. */
  size: (bytes: number) => string;
  /** "12.4" — MB/s value for the metrics row. */
  mbps: (bytesPerSecond: number) => string;
  /** "≈ 5 min 42 s" */
  eta: (seconds: number) => string;
  /** "6 min ago" / "há 6 min" — relative past time from an RFC3339 stamp. */
  ago: (iso: string) => string;
  /** 0-based index → "I", "II", … (falls back to arabic past XII). */
  roman: (index: number) => string;
  /** "128k" — context length shorthand. */
  contextK: (tokens: number) => string;
}

const ROMANS = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII"];

/** BCP-47 tag for `Intl`. The stored locale is a bare language code; the
 *  regional tag is what gives pt its comma decimals and "há" phrasing. */
const INTL_TAG: Record<Locale, string> = { en: "en-US", pt: "pt-BR" };

export function createFormat(locale: Locale): Format {
  const tag = INTL_TAG[locale];

  const decimal = (value: number, digits: number) =>
    new Intl.NumberFormat(tag, {
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    }).format(value);

  // `narrow` keeps the compact register the design relies on ("há 6 min"
  // rather than "há 6 minutos"); `numeric: auto` is what yields "agora" /
  // "now" instead of "há 0 segundos" for a just-now stamp.
  const relative = new Intl.RelativeTimeFormat(tag, { numeric: "auto", style: "narrow" });

  const gb: Format["gb"] = (bytes, digits = 2) => decimal(bytes / 1e9, digits);

  return {
    decimal,
    gb,
    size: (bytes) => (bytes >= 1e9 ? `${gb(bytes)} gb` : `${Math.round(bytes / 1e6)} mb`),
    mbps: (bytesPerSecond) => decimal(bytesPerSecond / 1e6, 1),

    eta: (seconds) => {
      if (!Number.isFinite(seconds) || seconds <= 0) return "—";
      if (seconds < 60) return `≈ ${Math.round(seconds)} s`;
      const m = Math.floor(seconds / 60);
      const s = Math.round(seconds % 60);
      if (m < 60) return `≈ ${m} min ${s.toString().padStart(2, "0")} s`;
      const h = Math.floor(m / 60);
      return `≈ ${h} h ${m % 60} min`;
    },

    ago: (iso) => {
      const t = Date.parse(iso);
      if (Number.isNaN(t)) return "—";
      const s = Math.max(0, (Date.now() - t) / 1000);
      if (s < 60) return relative.format(0, "second");
      const m = Math.floor(s / 60);
      if (m < 60) return relative.format(-m, "minute");
      const h = Math.floor(m / 60);
      if (h < 24) return relative.format(-h, "hour");
      const d = Math.floor(h / 24);
      if (d < 30) return relative.format(-d, "day");
      return relative.format(-Math.floor(d / 30), "month");
    },

    roman: (index) => ROMANS[index] ?? String(index + 1),
    contextK: (tokens) => (tokens >= 1000 ? `${Math.round(tokens / 1000)}k` : String(tokens)),
  };
}
