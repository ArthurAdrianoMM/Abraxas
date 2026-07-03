import { create } from "zustand";
import { commands, type HardwareDetection } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";

interface HardwareState {
  detection: HardwareDetection | null;
  error: string | null;
  /** true while a forced re-detection ("refazer o exame") is in flight. */
  redetecting: boolean;
  /** Cached-detection read (Fase 2.4); safe to call from several mounts. */
  init: () => Promise<void>;
  /** Force a fresh detection, bypassing the fingerprint cache. */
  redetect: () => Promise<void>;
}

let inFlight: Promise<void> | null = null;

export const useHardwareStore = create<HardwareState>((set, get) => ({
  detection: null,
  error: null,
  redetecting: false,

  init: async () => {
    if (get().detection || inFlight) return inFlight ?? undefined;
    inFlight = (async () => {
      try {
        const detection = await unwrap(commands.detectHardware(false));
        set({ detection, error: null });
      } catch (e) {
        set({ error: describeError(e) });
      } finally {
        inFlight = null;
      }
    })();
    return inFlight;
  },

  redetect: async () => {
    if (get().redetecting) return;
    set({ redetecting: true });
    try {
      const detection = await unwrap(commands.detectHardware(true));
      set({ detection, error: null });
    } catch (e) {
      set({ error: describeError(e) });
    } finally {
      set({ redetecting: false });
    }
  },
}));
