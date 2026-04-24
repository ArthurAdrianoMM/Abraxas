import { useEffect, useState } from "react";
import {
  commands,
  type AppInfo,
  type GpuBackend,
  type SystemInfo,
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
  const [error, setError] = useState<string | null>(null);

  const [system, setSystem] = useState<SystemInfo | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [sysError, setSysError] = useState<string | null>(null);

  const [gpu, setGpu] = useState<GpuBackend | null>(null);
  const [detectingGpu, setDetectingGpu] = useState(false);
  const [gpuError, setGpuError] = useState<string | null>(null);

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

  async function handleDetectGpu() {
    setDetectingGpu(true);
    setGpuError(null);
    try {
      setGpu(await commands.detectGpu());
    } catch (e) {
      setGpuError(e instanceof Error ? e.message : String(e));
    } finally {
      setDetectingGpu(false);
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

      <section style={{ marginTop: "2rem" }}>
        <button onClick={handleDetectGpu} disabled={detectingGpu}>
          {detectingGpu ? "Detecting..." : "Detect GPU"}
        </button>
        {gpuError && <p role="alert">{gpuError}</p>}
        {gpu && (
          <dl>
            <dt>Backend</dt>
            <dd>{gpu.kind}</dd>
            <dt>Detected</dt>
            <dd>{describeGpu(gpu)}</dd>
            {gpu.kind === "cuda" && (
              <>
                <dt>UUID</dt>
                <dd>{gpu.uuid || "(unavailable)"}</dd>
              </>
            )}
            {gpu.kind === "vulkan" && (
              <>
                <dt>Vendor ID</dt>
                <dd>0x{gpu.vendor_id.toString(16).toUpperCase().padStart(4, "0")}</dd>
              </>
            )}
          </dl>
        )}
      </section>
    </main>
  );
}

export default App;
