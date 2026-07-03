import { create } from "zustand";
import { commands, type InstalledModel } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";

export type ModelStatus =
  | "unknown" // haven't asked the backend yet
  | "none-installed" // registry is empty — chat is degraded
  | "loading" // load_installed_model in flight
  | "loaded" // a model is resident and can generate
  | "error"; // install list or load failed

interface ModelState {
  status: ModelStatus;
  installed: InstalledModel[];
  /** id of the model currently resident in the inference engine. */
  loadedId: string | null;
  error: string | null;
  /** List installed models and auto-load the first one (Fase 6.2 will make
   *  "last used" configurable; first-installed is the Phase-1 stand-in). */
  init: () => Promise<void>;
}

export const useModelStore = create<ModelState>((set, get) => ({
  status: "unknown",
  installed: [],
  loadedId: null,
  error: null,

  init: async () => {
    if (get().status !== "unknown") return;
    set({ status: "loading" });
    try {
      const installed = await unwrap(commands.listInstalledModels());
      if (installed.length === 0) {
        set({ status: "none-installed", installed });
        return;
      }
      const first = installed[0];
      set({ installed });
      await unwrap(commands.loadInstalledModel(first.id));
      set({ status: "loaded", loadedId: first.id });
    } catch (e) {
      set({ status: "error", error: describeError(e) });
    }
  },
}));
