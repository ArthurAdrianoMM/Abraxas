import { useEffect, useState } from "react";
import { commands, type AppInfo, type SystemInfo } from "./lib/tauri/bindings";
import "./App.css";

function formatBytes(bytes: number): string {
  const gb = bytes / 1_073_741_824;
  return `${gb.toFixed(2)} GB`;
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [sysError, setSysError] = useState<string | null>(null);

  useEffect(() => {
    commands.appInfo().then((result) => {
      if (result.status === "ok") setInfo(result.data);
      else setError(`${result.error.kind}: ${result.error.message}`);
    });
  }, []);

  async function handleDetect() {
    setDetecting(true);
    setSysError(null);
    try {
      setSystem(await commands.detectSystem());
    } catch (e) {
      setSysError(e instanceof Error ? e.message : String(e));
    } finally {
      setDetecting(false);
    }
  }

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

      <section style={{ marginTop: "2rem" }}>
        <button onClick={handleDetect} disabled={detecting}>
          {detecting ? "Detecting..." : "Detect hardware"}
        </button>
        {sysError && <p role="alert">{sysError}</p>}
        {system && (
          <dl>
            <dt>OS</dt>
            <dd>
              {system.os.family} ({system.os.arch})
              {system.os.version ? ` — ${system.os.version}` : ""}
            </dd>
            <dt>CPU</dt>
            <dd>
              {system.cpu.brand || "(unknown)"}
              {system.cpu.vendor ? ` — ${system.cpu.vendor}` : ""}
            </dd>
            <dt>Cores</dt>
            <dd>
              {system.cpu.physical_cores} physical / {system.cpu.logical_cores} logical
            </dd>
            <dt>Features</dt>
            <dd>
              AVX2: {system.cpu.features.avx2 ? "yes" : "no"} · AVX-512F:{" "}
              {system.cpu.features.avx512f ? "yes" : "no"}
            </dd>
            <dt>Memory</dt>
            <dd>
              {formatBytes(system.memory.available_bytes)} available /{" "}
              {formatBytes(system.memory.total_bytes)} total
            </dd>
          </dl>
        )}
      </section>
    </main>
  );
}

export default App;
