import { create } from "zustand";

export type View = "chat" | "models" | "settings";

/** Sub-navigation inside the Models view: the ateliê (installed models),
 *  the compêndio (remote catalog), and the download spread. */
export type ModelsPane = "manager" | "catalog" | "download";

/**
 * Top-level screen. "shell" renders the sidebar + topbar chrome with the
 * active view inside; "onboarding" (Fase 6) takes over the whole window
 * before the shell ever appears.
 */
export type Screen = "onboarding" | "shell";

interface UiState {
  screen: Screen;
  view: View;
  modelsPane: ModelsPane;
  /** Chat-only "ordens desta conversa" drawer. */
  ordersOpen: boolean;
  /** Topbar model-switcher popover. */
  switcherOpen: boolean;
  setView: (view: View) => void;
  /** Navigate straight to a pane of the Models view. */
  openModels: (pane?: ModelsPane) => void;
  setModelsPane: (pane: ModelsPane) => void;
  setOrdersOpen: (open: boolean) => void;
  setSwitcherOpen: (open: boolean) => void;
  completeOnboarding: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  // Fase 6 will start first runs at "onboarding"; until then the shell is home.
  screen: "shell",
  view: "chat",
  modelsPane: "manager",
  ordersOpen: false,
  switcherOpen: false,
  setView: (view) => set({ view, ordersOpen: false, switcherOpen: false }),
  openModels: (pane = "manager") =>
    set({ view: "models", modelsPane: pane, ordersOpen: false, switcherOpen: false }),
  setModelsPane: (modelsPane) => set({ modelsPane }),
  setOrdersOpen: (ordersOpen) => set({ ordersOpen }),
  setSwitcherOpen: (switcherOpen) => set({ switcherOpen }),
  completeOnboarding: () => set({ screen: "shell" }),
}));
