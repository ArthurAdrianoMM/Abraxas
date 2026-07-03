import { AppShell } from "./components/shell/AppShell";
import { useUiStore } from "./stores/ui";

function App() {
  const screen = useUiStore((s) => s.screen);

  // Fase 6: the onboarding flow renders here, full-window, before the shell.
  if (screen === "onboarding") {
    return null;
  }

  return <AppShell />;
}

export default App;
