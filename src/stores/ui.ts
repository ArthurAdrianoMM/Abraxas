import { create } from "zustand";
import { commands } from "../lib/tauri/bindings";
import { unwrap } from "../lib/tauri/result";
import { useSettingsStore } from "./settings";

export type View = "chat" | "models" | "settings";

/** Sub-navigation inside the Models view: the ateliê (installed models),
 *  the compêndio (remote catalog), and the download spread. */
export type ModelsPane = "manager" | "catalog" | "download";

/**
 * Top-level screen. "boot" is the pre-decision blank while the first-run
 * rule resolves; "shell" renders the sidebar + topbar chrome with the
 * active view inside; "onboarding" (Fase 6) takes over the whole window
 * before the shell ever appears.
 */
export type Screen = "boot" | "onboarding" | "shell";

interface UiState {
  screen: Screen;
  view: View;
  modelsPane: ModelsPane;
  /** Chat-only "ordens desta conversa" drawer. */
  ordersOpen: boolean;
  /** Topbar model-switcher popover. */
  switcherOpen: boolean;
  /**
   * First-run rule: onboard only when the flag is unset AND the install is
   * genuinely empty (no installed models, no conversations). Installs that
   * predate the flag have data, so they auto-complete silently instead of
   * being onboarded retroactively. Any failure resolves to the shell —
   * onboarding must never trap the app.
   */
  initScreen: () => Promise<void>;
  setView: (view: View) => void;
  /** Navigate straight to a pane of the Models view. */
  openModels: (pane?: ModelsPane) => void;
  setModelsPane: (pane: ModelsPane) => void;
  setOrdersOpen: (open: boolean) => void;
  setSwitcherOpen: (open: boolean) => void;
  /** Leave onboarding for the shell and persist the flag (fire-and-forget). */
  completeOnboarding: () => void;
}

export const useUiStore = create<UiState>((set, get) => ({
  screen: "boot",
  view: "chat",
  modelsPane: "manager",
  ordersOpen: false,
  switcherOpen: false,

  initScreen: async () => {
    if (get().screen !== "boot") return;
    try {
      await useSettingsStore.getState().init();
      if (useSettingsStore.getState().settings?.onboarding_complete) {
        set({ screen: "shell" });
        return;
      }
      const [models, conversations] = await Promise.all([
        unwrap(commands.listInstalledModels()).catch(() => []),
        unwrap(commands.listConversations()).catch(() => []),
      ]);
      if (models.length > 0 || conversations.length > 0) {
        void useSettingsStore.getState().save({ onboarding_complete: true });
        set({ screen: "shell" });
      } else {
        set({ screen: "onboarding" });
      }
    } catch {
      set({ screen: "shell" });
    }
  },

  setView: (view) => set({ view, ordersOpen: false, switcherOpen: false }),
  openModels: (pane = "manager") =>
    set({ view: "models", modelsPane: pane, ordersOpen: false, switcherOpen: false }),
  setModelsPane: (modelsPane) => set({ modelsPane }),
  setOrdersOpen: (ordersOpen) => set({ ordersOpen }),
  setSwitcherOpen: (switcherOpen) => set({ switcherOpen }),
  completeOnboarding: () => {
    set({ screen: "shell" });
    void useSettingsStore.getState().save({ onboarding_complete: true });
  },
}));
