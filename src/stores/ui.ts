import { create } from "zustand";
import { commands } from "../lib/tauri/bindings";
import { DEMO_MODE } from "../lib/demo";
import { unwrap } from "../lib/tauri/result";
import { useSettingsStore } from "./settings";

export type View = "chat" | "models" | "settings";

/** Sub-navigation inside the Models view: the ateliê (installed models),
 *  the compêndio (remote catalog), the download spread, and the import
 *  spread (a GGUF from disk or from a pasted link). */
export type ModelsPane = "manager" | "catalog" | "download" | "import";

/**
 * Top-level screen. "boot" is the pre-decision blank while the first-run
 * rule resolves; "shell" renders the sidebar + topbar chrome with the
 * active view inside; "onboarding" (Fase 6) takes over the whole window
 * before the shell ever appears. "curtain" only ever appears in a recording
 * build (`lib/demo.ts`) — it holds the same blank indefinitely, waiting for
 * the key that starts a take.
 */
export type Screen = "boot" | "curtain" | "onboarding" | "shell";

interface UiState {
  screen: Screen;
  view: View;
  modelsPane: ModelsPane;
  /**
   * Recording builds only: bumping this remounts the whole screen tree, so
   * entrance animations replay from their first frame. Stays 0 otherwise.
   */
  take: number;
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
  /** Recording builds only: leave the curtain and run the first-run rule. */
  startTake: () => Promise<void>;
  /** Recording builds only: back to the curtain, next mount fresh. */
  resetTake: () => void;
  setView: (view: View) => void;
  /** Navigate straight to a pane of the Models view. */
  openModels: (pane?: ModelsPane) => void;
  setModelsPane: (pane: ModelsPane) => void;
  setOrdersOpen: (open: boolean) => void;
  setSwitcherOpen: (open: boolean) => void;
  /** Leave onboarding for the shell and persist the flag (fire-and-forget). */
  completeOnboarding: () => void;
}

/** The first-run rule itself, factored out so a recording build can run it
 *  when the take starts rather than at launch. Never throws. */
async function resolveScreen(): Promise<Screen> {
  // Dev-only: `VITE_FORCE_ONBOARDING=1 pnpm tauri dev` replays the first run
  // without touching the real settings or data on this machine.
  if (import.meta.env.DEV && import.meta.env.VITE_FORCE_ONBOARDING === "1") {
    return "onboarding";
  }
  try {
    await useSettingsStore.getState().init();
    if (useSettingsStore.getState().settings?.onboarding_complete) {
      return "shell";
    }
    const [models, conversations] = await Promise.all([
      unwrap(commands.listInstalledModels()).catch(() => []),
      unwrap(commands.listConversations()).catch(() => []),
    ]);
    if (models.length > 0 || conversations.length > 0) {
      void useSettingsStore.getState().save({ onboarding_complete: true });
      return "shell";
    }
    return "onboarding";
  } catch {
    return "shell";
  }
}

export const useUiStore = create<UiState>((set, get) => ({
  screen: "boot",
  view: "chat",
  modelsPane: "manager",
  take: 0,
  ordersOpen: false,
  switcherOpen: false,

  initScreen: async () => {
    if (get().screen !== "boot") return;
    // A recording build stops here: the rule runs when the take starts, so
    // whatever it resolves to is mounted on camera and animates from zero.
    if (DEMO_MODE) {
      set({ screen: "curtain" });
      return;
    }
    set({ screen: await resolveScreen() });
  },

  startTake: async () => {
    if (get().screen !== "curtain") return;
    set({ screen: await resolveScreen() });
  },

  resetTake: () =>
    set((s) => ({
      screen: "curtain",
      take: s.take + 1,
      view: "chat",
      modelsPane: "manager",
      ordersOpen: false,
      switcherOpen: false,
    })),

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
