import { useEffect, useState } from "react";
import {
  commands,
  type AppInfo,
  type GpuBackend,
  type HardwareDetection,
} from "./lib/tauri/bindings";
import "./App.css";

function formatBytes(bytes: number): string {
  const gb = bytes / 1_073_741_824;
  return `${gb.toFixed(2)} GB`;
}

function describeGpu(gpu: GpuBackend): string {
  switch (gpu.kind) {
    case "metal":
      return "Metal (macOS)";
    case "cuda":
      return `${gpu.name} · ${gpu.vram_mb} MB · CC ${gpu.compute_capability.major}.${gpu.compute_capability.minor}`;
    case "vulkan": {
      const vram = gpu.vram_mb !== null ? `${gpu.vram_mb} MB` : "unknown VRAM";
      return `${gpu.vendor} · ${gpu.name} · ${vram} · ${gpu.device_type}`;
    }
    case "none":
      return "No GPU detected — CPU fallback";
  }
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [appInfoError, setAppInfoError] = useState<string | null>(null);

  const [hw, setHw] = useState<HardwareDetection | null>(null);
  const [hwLoading, setHwLoading] = useState(false);
  const [hwError, setHwError] = useState<string | null>(null);

  useEffect(() => {
    commands.appInfo().then((result) => {
      if (result.status === "ok") setInfo(result.data);
      else setAppInfoError(`${result.error.kind}: ${result.error.message}`);
    });
  }, []);

  async function runDetect(force: boolean) {
    setHwLoading(true);
    setHwError(null);
    try {
      const result = await commands.detectHardware(force);
      if (result.status === "ok") setHw(result.data);
      else setHwError(`${result.error.kind}: ${result.error.message}`);
    } catch (e) {
      setHwError(e instanceof Error ? e.message : String(e));
    } finally {
      setHwLoading(false);
    }
  }

  useEffect(() => {
    runDetect(false);
  }, []);

  return (
    <main className="container">
      <h1>Abraxas</h1>
      <p>Tauri v2 scaffold ready — Fase 2.4 complete.</p>
      {appInfoError && <p role="alert">{appInfoError}</p>}
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
        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          <button onClick={() => runDetect(false)} disabled={hwLoading}>
            {hwLoading ? "Detecting..." : "Detect hardware"}
          </button>
          <button onClick={() => runDetect(true)} disabled={hwLoading}>
            Re-detect (force)
          </button>
          {hw && (
            <span style={{ fontSize: "0.875rem", opacity: 0.75 }}>
              {hw.from_cache ? "Cached" : "Fresh"} · {hw.detected_at}
            </span>
          )}
        </div>
        {hwError && <p role="alert">{hwError}</p>}
        {hw && (
          <>
            <h2 style={{ fontSize: "1rem", marginTop: "1rem", marginBottom: "0.25rem" }}>System</h2>
            <dl>
              <dt>OS</dt>
              <dd>
                {hw.system.os.family} ({hw.system.os.arch})
                {hw.system.os.version ? ` — ${hw.system.os.version}` : ""}
              </dd>
              <dt>CPU</dt>
              <dd>
                {hw.system.cpu.brand || "(unknown)"}
                {hw.system.cpu.vendor ? ` — ${hw.system.cpu.vendor}` : ""}
              </dd>
              <dt>Cores</dt>
              <dd>
                {hw.system.cpu.physical_cores} physical / {hw.system.cpu.logical_cores} logical
              </dd>
              <dt>Features</dt>
              <dd>
                AVX2: {hw.system.cpu.features.avx2 ? "yes" : "no"} · AVX-512F:{" "}
                {hw.system.cpu.features.avx512f ? "yes" : "no"}
              </dd>
              <dt>Memory</dt>
              <dd>
                {formatBytes(hw.system.memory.available_bytes)} available /{" "}
                {formatBytes(hw.system.memory.total_bytes)} total
              </dd>
            </dl>

            <h2 style={{ fontSize: "1rem", marginTop: "1rem", marginBottom: "0.25rem" }}>GPU</h2>
            <dl>
              <dt>Backend</dt>
              <dd>{hw.gpu.kind}</dd>
              <dt>Detected</dt>
              <dd>{describeGpu(hw.gpu)}</dd>
              {hw.gpu.kind === "cuda" && (
                <>
                  <dt>UUID</dt>
                  <dd>{hw.gpu.uuid || "(unavailable)"}</dd>
                </>
              )}
              {hw.gpu.kind === "vulkan" && (
                <>
                  <dt>Vendor ID</dt>
                  <dd>0x{hw.gpu.vendor_id.toString(16).toUpperCase().padStart(4, "0")}</dd>
                </>
              )}
            </dl>

            <h2 style={{ fontSize: "1rem", marginTop: "1rem", marginBottom: "0.25rem" }}>
              Selected backend
            </h2>
            <dl>
              <dt>Backend</dt>
              <dd>{hw.choice.backend}</dd>
              <dt>Reason</dt>
              <dd>{hw.choice.reason}</dd>
              <dt>Fingerprint</dt>
              <dd style={{ fontFamily: "monospace", fontSize: "0.75rem" }}>{hw.fingerprint}</dd>
            </dl>
          </>
        )}
      </section>
    </main>
  );
}

export default App;
