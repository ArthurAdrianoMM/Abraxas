import { create } from "zustand";
import { commands, type HardwareDetection } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";

interface HardwareState {
  detection: HardwareDetection | null;
  error: string | null;
  /** Cached-detection read (Fase 2.4); safe to call from several mounts. */
  init: () => Promise<void>;
}

let inFlight: Promise<void> | null = null;

export const useHardwareStore = create<HardwareState>((set, get) => ({
  detection: null,
  error: null,

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
}));
