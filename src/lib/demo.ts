import type { Locale } from "./tauri/bindings";
import { LOCALES } from "./i18n/locale";

/** Demo / recording mode.
 *
 *  `VITE_DEMO_MODE=1` makes the app hold on a blank curtain instead of
 *  resolving the first-run rule at launch. The recorder starts rolling against
 *  a still frame, one key press starts the take, and every entrance animation
 *  — the golden-ratio plate above all, which draws itself in 2.4s and is long
 *  over by the time a human reaches the record button — plays from its first
 *  frame on camera. ⌘⇧R drops back to the curtain for another take without
 *  relaunching the app.
 *
 *  Unlike `VITE_FORCE_ONBOARDING` (see `stores/ui.ts`) this is deliberately
 *  *not* gated on `import.meta.env.DEV`: the point is to record the real
 *  bundled app, icon in the Dock and all.
 *
 *    VITE_DEMO_MODE=1 pnpm tauri build --features metal --bundles app
 *
 *  Vite inlines the value at build time, so a normal build leaves it undefined
 *  and every branch behind this flag is dead code the bundler drops. A bundle
 *  built with it must never be released.
 */
export const DEMO_MODE = import.meta.env.VITE_DEMO_MODE === "1";

/** The UI language a recording build pins itself to (`VITE_DEMO_LOCALE=en`).
 *
 *  Without it the app adopts the host machine's language on first run, so a
 *  Mac set to Portuguese can only ever film a Portuguese take. Pinning beats
 *  changing the system language for an afternoon, and it applies from the
 *  first paint — the language never flips on camera.
 *
 *  `null` (the normal case) leaves `resolveHostLocale` in charge.
 */
export const DEMO_LOCALE: Locale | null = (() => {
  const raw = import.meta.env.VITE_DEMO_LOCALE;
  if (!raw) return null;
  const tag = raw.toLowerCase();
  if (!LOCALES.includes(tag as Locale)) {
    console.warn(`[demo] VITE_DEMO_LOCALE="${raw}" is not a shipped locale; ignoring it`);
    return null;
  }
  return tag as Locale;
})();
