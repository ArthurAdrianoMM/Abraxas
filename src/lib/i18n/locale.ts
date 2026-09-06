/** Locale resolution, with no React and no store imports.
 *
 *  Kept as a leaf module on purpose: the settings store needs
 *  `resolveHostLocale` during `init`, and `i18n/index` needs the store to read
 *  the current locale. Putting both helpers here breaks what would otherwise
 *  be an import cycle between them. */

import type { Locale } from "../tauri/bindings";

export const DEFAULT_LOCALE: Locale = "en";

export const LOCALES: readonly Locale[] = ["en", "pt"];

/** Endonyms — a language picker that names languages in the reader's own
 *  language is the one case where translating the label is wrong. */
export const LOCALE_NAMES: Record<Locale, string> = {
  en: "english",
  pt: "português",
};

/** Host preference → a locale we actually ship. Anything Portuguese maps to
 *  `pt`; everything else falls back to English rather than guessing at a
 *  near-match. Only consulted once, when no locale has been persisted yet. */
export function resolveHostLocale(): Locale {
  const tags =
    typeof navigator === "undefined"
      ? []
      : navigator.languages?.length
        ? navigator.languages
        : [navigator.language];
  return tags.some((tag) => tag.toLowerCase().startsWith("pt")) ? "pt" : DEFAULT_LOCALE;
}
