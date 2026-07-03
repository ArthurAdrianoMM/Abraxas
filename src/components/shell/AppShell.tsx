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
  const ActiveView = VIEWS[view];

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
