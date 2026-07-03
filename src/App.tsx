import { useEffect } from "react";
import { OnboardingFlow } from "./components/onboarding/OnboardingFlow";
import { AppShell } from "./components/shell/AppShell";
import { useUiStore } from "./stores/ui";

function App() {
  const screen = useUiStore((s) => s.screen);
  const initScreen = useUiStore((s) => s.initScreen);

  useEffect(() => {
    void initScreen();
  }, [initScreen]);

  // The first-run decision is one settings read — a blank beat, not a splash.
  if (screen === "boot") {
    return null;
  }

  if (screen === "onboarding") {
    return <OnboardingFlow />;
  }

  return <AppShell />;
}

export default App;
