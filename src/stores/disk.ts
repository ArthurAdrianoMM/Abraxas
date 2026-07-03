import { create } from "zustand";
import { commands, type DiskUsage } from "../lib/tauri/bindings";
import { unwrap } from "../lib/tauri/result";

interface DiskState {
  /** null until the first successful read; total_bytes === 0 means the
   *  backend couldn't match a disk (meters should hide, not divide by 0). */
  usage: DiskUsage | null;
  /** Re-read free/total space. Cheap; consumers call it on mount and after
   *  anything that changes disk contents (download, delete). */
  refresh: () => Promise<void>;
}

let inFlight: Promise<void> | null = null;

export const useDiskStore = create<DiskState>((set) => ({
  usage: null,

  refresh: async () => {
    if (inFlight) return inFlight;
    inFlight = (async () => {
      try {
        const usage = await unwrap(commands.diskUsage());
        set({ usage });
      } catch {
        // Non-critical chrome — keep the last reading (or null) on failure.
      } finally {
        inFlight = null;
      }
    })();
    return inFlight;
  },
}));
