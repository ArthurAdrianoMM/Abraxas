/** UI translations.
 *
 *  Components read copy through `useT()` and numbers through `useFormat()`.
 *  Both are subscribed to the persisted locale, so switching the language
 *  re-renders text and figures together — there is no module-level "current
 *  locale" that could drift out of step with React.
 *
 *  `en` defines the dictionary type; `pt` is checked against it. */

import { useMemo } from "react";

import { createFormat, type Format } from "../format";
import type { Locale } from "../tauri/bindings";
import { useSettingsStore } from "../../stores/settings";
import { DEFAULT_LOCALE } from "./locale";
import { en } from "./en";
import { pt } from "./pt";

export type Dict = typeof en;

const DICTS: Record<Locale, Dict> = { en, pt };

export { DEFAULT_LOCALE, LOCALES, LOCALE_NAMES, resolveHostLocale } from "./locale";

/** The locale in force. Falls back to English while settings are still
 *  loading, and for installs that predate the setting. */
export function useLocale(): Locale {
  return useSettingsStore((s) => s.settings?.locale ?? DEFAULT_LOCALE);
}

export function useT(): Dict {
  return DICTS[useLocale()];
}

/** Locale-bound number, size and duration formatting. Memoised per locale so
 *  the `Intl` formatters inside are built once, not on every render. */
export function useFormat(): Format {
  const locale = useLocale();
  return useMemo(() => createFormat(locale), [locale]);
}
