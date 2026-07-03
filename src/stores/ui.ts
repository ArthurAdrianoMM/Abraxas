import { create } from "zustand";

export type View = "chat" | "models" | "settings";

/**
 * Top-level screen. "shell" renders the sidebar + topbar chrome with the
 * active view inside; "onboarding" (Fase 6) takes over the whole window
 * before the shell ever appears.
 */
export type Screen = "onboarding" | "shell";

interface UiState {
  screen: Screen;
  view: View;
  /** Chat-only "ordens desta conversa" drawer. */
  ordersOpen: boolean;
  setView: (view: View) => void;
  setOrdersOpen: (open: boolean) => void;
  completeOnboarding: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  // Fase 6 will start first runs at "onboarding"; until then the shell is home.
  screen: "shell",
  view: "chat",
  ordersOpen: false,
  setView: (view) => set({ view, ordersOpen: false }),
  setOrdersOpen: (ordersOpen) => set({ ordersOpen }),
  completeOnboarding: () => set({ screen: "shell" }),
}));
