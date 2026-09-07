import { useEffect } from "react";
import { DemoCurtain } from "./components/demo/DemoCurtain";
import { OnboardingFlow } from "./components/onboarding/OnboardingFlow";
import { AppShell } from "./components/shell/AppShell";
import { DEMO_MODE } from "./lib/demo";
import { useUiStore } from "./stores/ui";

function App() {
  const screen = useUiStore((s) => s.screen);
  const take = useUiStore((s) => s.take);
  const initScreen = useUiStore((s) => s.initScreen);
  const resetTake = useUiStore((s) => s.resetTake);

  useEffect(() => {
    void initScreen();
  }, [initScreen]);

  // Recording builds: ⌘⇧R drops back to the curtain. `take` keys the tree
  // below, so the next take is a fresh mount and every entrance animation
  // runs again — no relaunch between takes.
  useEffect(() => {
    if (!DEMO_MODE) return;
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || !e.shiftKey) return;
      if (e.key.toLowerCase() !== "r") return;
      e.preventDefault();
      resetTake();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [resetTake]);

  // The first-run decision is one settings read — a blank beat, not a splash.
  if (screen === "boot") {
    return null;
  }

  // The DEMO_MODE guard is what lets Rollup drop the curtain from a normal
  // bundle: without it the branch stays reachable and the component ships.
  if (DEMO_MODE && screen === "curtain") {
    return <DemoCurtain />;
  }

  if (screen === "onboarding") {
    return <OnboardingFlow key={take} />;
  }

  return <AppShell key={take} />;
}

export default App;
