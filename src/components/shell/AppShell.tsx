import { useEffect } from "react";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore, type View } from "../../stores/ui";
import { ChatView } from "../../views/ChatView";
import { ModelsView } from "../../views/ModelsView";
import { SettingsView } from "../../views/SettingsView";
import { LoadRitual } from "../models/LoadRitual";
import { SwitchingToast } from "../models/SwitchingToast";
import { Sidebar } from "./Sidebar";
import { Topbar } from "./Topbar";

const VIEWS: Record<View, () => React.ReactNode> = {
  chat: ChatView,
  models: ModelsView,
  settings: SettingsView,
};

export function AppShell() {
  const view = useUiStore((s) => s.view);
  const initSettings = useSettingsStore((s) => s.init);
  const ActiveView = VIEWS[view];

  // Settings gate presentation (font size) and the default model, so they
  // load with the shell rather than with any particular view.
  useEffect(() => {
    void initSettings();
  }, [initSettings]);

  return (
    <div className="shell">
      <Sidebar />
      <main className="main" style={{ position: "relative" }}>
        <Topbar />
        <ActiveView />
      </main>
      <LoadRitual />
      <SwitchingToast />
    </div>
  );
}
