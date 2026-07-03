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
  setView: (view: View) => void;
  completeOnboarding: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  // Fase 6 will start first runs at "onboarding"; until then the shell is home.
  screen: "shell",
  view: "chat",
  setView: (view) => set({ view }),
  completeOnboarding: () => set({ screen: "shell" }),
}));
