import { create } from "zustand";
import { commands, type AppSettings } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";

export type SettingsStatus = "idle" | "loading" | "ready" | "error";

interface SettingsState {
  status: SettingsStatus;
  /** null until the first successful load. */
  settings: AppSettings | null;
  error: string | null;

  /** Load persisted settings. Safe to call repeatedly; concurrent calls
   *  coalesce into the in-flight request. */
  init: () => Promise<void>;
  /** Merge `patch` over the current settings and persist. Optimistic:
   *  the UI updates immediately and reverts if the backend write fails. */
  save: (patch: Partial<AppSettings>) => Promise<boolean>;
  /** Re-read from the backend (after clear_all_data etc.). */
  refresh: () => Promise<void>;
  dismissError: () => void;
}

/** Reading-size preset → root data attribute the design system scales
 *  `--prose-size` on. */
function applyFontSize(settings: AppSettings | null) {
  const size = settings?.font_size ?? "comoda";
  document.documentElement.dataset.fontSize = size;
}

let inFlight: Promise<void> | null = null;

export const useSettingsStore = create<SettingsState>((set, get) => ({
  status: "idle",
  settings: null,
  error: null,

  init: async () => {
    if (get().status === "ready") return;
    if (inFlight) return inFlight;
    inFlight = (async () => {
      set({ status: "loading", error: null });
      try {
        const settings = await unwrap(commands.getAppSettings());
        applyFontSize(settings);
        set({ status: "ready", settings, error: null });
      } catch (e) {
        set({ status: "error", error: describeError(e) });
      } finally {
        inFlight = null;
      }
    })();
    return inFlight;
  },

  save: async (patch) => {
    const previous = get().settings;
    if (!previous) return false;
    const next = { ...previous, ...patch };
    applyFontSize(next);
    set({ settings: next, error: null });
    try {
      const persisted = await unwrap(commands.setAppSettings(next));
      applyFontSize(persisted);
      set({ settings: persisted });
      return true;
    } catch (e) {
      // Don't restore the snapshot wholesale — a concurrent save (e.g.
      // "tornar padrão" from the manager) may have landed since; re-read
      // the backend's truth instead.
      set({ error: describeError(e) });
      await get().refresh();
      return false;
    }
  },

  refresh: async () => {
    try {
      const settings = await unwrap(commands.getAppSettings());
      applyFontSize(settings);
      set({ status: "ready", settings, error: null });
    } catch (e) {
      set({ status: "error", error: describeError(e) });
    }
  },

  dismissError: () => set({ error: null }),
}));
