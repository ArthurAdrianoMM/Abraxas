import { useEffect, useState } from "react";
import { commands, type AppInfo } from "./lib/tauri/bindings";
import "./App.css";

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    commands.appInfo().then((result) => {
      if (result.status === "ok") setInfo(result.data);
      else setError(`${result.error.kind}: ${result.error.message}`);
    });
  }, []);

  return (
    <main className="container">
      <h1>Abraxas</h1>
      <p>Tauri v2 scaffold ready — Fase 1.5 complete.</p>
      {error && <p role="alert">{error}</p>}
      {info && (
        <dl>
          <dt>Version</dt>
          <dd>{info.version}</dd>
          <dt>Data dir</dt>
          <dd>{info.app_data_dir}</dd>
          <dt>Log dir</dt>
          <dd>{info.log_dir}</dd>
        </dl>
      )}
    </main>
  );
}

export default App;
