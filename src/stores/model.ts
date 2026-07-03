import { create } from "zustand";
import { commands, type InstalledModel } from "../lib/tauri/bindings";
import { describeError, unwrap } from "../lib/tauri/result";
import { useSettingsStore } from "./settings";

export type ModelStatus =
  | "unknown" // haven't asked the backend yet
  | "initializing" // init() in flight
  | "none-installed" // registry is empty — chat is degraded
  | "idle" // models installed but none resident
  | "loading" // load_installed_model in flight
  | "loaded" // a model is resident and can generate
  | "error"; // install list or load failed

/** How the in-flight load presents itself: the full "despertando" ritual
 *  (Models view / post-download) or the quiet switching toast (topbar pill). */
export type LoadPresentation = "ritual" | "toast";

interface ModelState {
  status: ModelStatus;
  installed: InstalledModel[];
  /** id of the model currently resident in the inference engine — ground
   *  truth from `get_loaded_model`, not frontend bookkeeping. */
  loadedId: string | null;
  /** target of the in-flight load, while status === "loading". */
  loadingId: string | null;
  loadPresentation: LoadPresentation;
  error: string | null;

  /** List installed models and reconcile with what the engine reports as
   *  loaded. Auto-loads the configured default model when nothing is
   *  resident, falling back to the first installed. */
  init: () => Promise<void>;
  refreshInstalled: () => Promise<void>;
  load: (modelId: string, presentation?: LoadPresentation) => Promise<boolean>;
  /** Throws the CommandError on failure so callers can branch on `kind`
   *  (e.g. "ModelLoaded" when the backend refuses to delete the loaded model). */
  remove: (modelId: string) => Promise<void>;
  dismissError: () => void;
}

function statusFor(installed: InstalledModel[], loadedId: string | null): ModelStatus {
  if (loadedId) return "loaded";
  return installed.length === 0 ? "none-installed" : "idle";
}

export const useModelStore = create<ModelState>((set, get) => ({
  status: "unknown",
  installed: [],
  loadedId: null,
  loadingId: null,
  loadPresentation: "ritual",
  error: null,

  init: async () => {
    // "error" stays retryable — a transient failure here would otherwise
    // permanently skip the default-model auto-load until restart.
    const status = get().status;
    if (status !== "unknown" && status !== "error") return;
    set({ status: "initializing" });
    try {
      const [installed, loadedId] = await Promise.all([
        unwrap(commands.listInstalledModels()),
        unwrap(commands.getLoadedModel()),
      ]);
      set({ installed, loadedId, status: statusFor(installed, loadedId) });
      if (!loadedId && installed.length > 0) {
        await useSettingsStore.getState().init();
        const defaultId = useSettingsStore.getState().settings?.default_model_id;
        const target = installed.find((m) => m.id === defaultId) ?? installed[0];
        await get().load(target.id, "toast");
      }
    } catch (e) {
      set({ status: "error", error: describeError(e) });
    }
  },

  refreshInstalled: async () => {
    try {
      const [installed, loadedId] = await Promise.all([
        unwrap(commands.listInstalledModels()),
        unwrap(commands.getLoadedModel()),
      ]);
      set((prev) => ({
        installed,
        loadedId,
        status: prev.status === "loading" ? prev.status : statusFor(installed, loadedId),
      }));
    } catch (e) {
      set({ error: describeError(e) });
    }
  },

  load: async (modelId, presentation = "ritual") => {
    if (get().status === "loading") return false;
    set({ status: "loading", loadingId: modelId, loadPresentation: presentation, error: null });
    try {
      await unwrap(commands.loadInstalledModel(modelId));
      set({ status: "loaded", loadedId: modelId, loadingId: null });
      return true;
    } catch (e) {
      // A failed swap unloads the previous model first — re-read ground truth.
      const loadedId = await unwrap(commands.getLoadedModel()).catch(() => null);
      set((prev) => ({
        status: statusFor(prev.installed, loadedId),
        loadedId,
        loadingId: null,
        error: describeError(e),
      }));
      return false;
    }
  },

  remove: async (modelId) => {
    await unwrap(commands.deleteModel(modelId));
    await get().refreshInstalled();
  },

  dismissError: () => set({ error: null }),
}));
