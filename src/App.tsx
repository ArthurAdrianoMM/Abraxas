import { useEffect, useRef, useState } from "react";
import { type UnlistenFn } from "@tauri-apps/api/event";
import {
  commands,
  events,
  type AppInfo,
  type ClassifiedCatalogResponse,
  type CommandError,
  type DownloadEvent,
  type GpuBackend,
  type HardwareDetection,
  type InstalledModel,
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

function HardwarePanel() {
  const [hw, setHw] = useState<HardwareDetection | null>(null);
  const [hwLoading, setHwLoading] = useState(false);
  const [hwError, setHwError] = useState<string | null>(null);

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
    <>
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
          <h3 style={{ fontSize: "1rem", marginTop: "1rem", marginBottom: "0.25rem" }}>System</h3>
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

          <h3 style={{ fontSize: "1rem", marginTop: "1rem", marginBottom: "0.25rem" }}>GPU</h3>
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

          <h3 style={{ fontSize: "1rem", marginTop: "1rem", marginBottom: "0.25rem" }}>
            Selected backend
          </h3>
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
    </>
  );
}

function InferencePanel() {
  const [installed, setInstalled] = useState<InstalledModel[]>([]);
  const [selectedId, setSelectedId] = useState<string>("");
  const [modelLoaded, setModelLoaded] = useState(false);
  const [loadStatus, setLoadStatus] = useState<string>("no model loaded");
  const [loading, setLoading] = useState(false);

  async function refreshInstalled() {
    const r = await commands.listInstalledModels();
    if (r.status === "ok") {
      setInstalled(r.data);
      if (r.data.length > 0 && !selectedId) {
        setSelectedId(r.data[0].id);
      }
    }
  }

  useEffect(() => {
    refreshInstalled();
  }, []);

  // BUG-3: auto-refresh the installed list when a download completes so the
  // user doesn't have to click ↻ manually after each download.
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;
    events.downloadEvent
      .listen((event) => {
        if (event.payload.type === "completed") {
          refreshInstalled();
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const [prompt, setPrompt] = useState("");
  const [tokens, setTokens] = useState("");
  const [status, setStatus] = useState<string>("idle");
  const [currentId, setCurrentId] = useState<string | null>(null);
  const currentIdRef = useRef<string | null>(null);

  // Mirror currentId into a ref so the (mount-only) listener closure can read
  // the latest id without re-subscribing on every change.
  useEffect(() => {
    currentIdRef.current = currentId;
  }, [currentId]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;
    events.generationEvent
      .listen((event) => {
        const p = event.payload;
        if (currentIdRef.current && p.generation_id !== currentIdRef.current) {
          return;
        }
        switch (p.type) {
          case "started":
            setStatus("generating");
            break;
          case "token":
            setTokens((t) => t + p.text);
            break;
          case "end":
            setStatus(`end: ${p.reason}`);
            setCurrentId(null);
            break;
          case "failed":
            setStatus(`failed: ${p.kind}: ${p.message}`);
            setCurrentId(null);
            break;
          case "cancelled":
            setStatus("cancelled");
            setCurrentId(null);
            break;
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  async function handleLoad() {
    if (!selectedId) return;
    setLoading(true);
    setLoadStatus("loading...");
    const result = await commands.loadInstalledModel(selectedId);
    if (result.status === "ok") {
      setModelLoaded(true);
      setLoadStatus(`loaded: ${selectedId}`);
    } else {
      setModelLoaded(false);
      setLoadStatus(`load failed: ${formatErr(result.error)}`);
    }
    setLoading(false);
  }

  async function handleGenerate() {
    if (!prompt.trim() || !modelLoaded) return;
    setTokens("");
    setStatus("starting...");
    const result = await commands.startGeneration(prompt, 256);
    if (result.status === "ok") {
      setCurrentId(result.data);
    } else {
      setStatus(`failed to start: ${formatErr(result.error)}`);
    }
  }

  async function handleCancel() {
    if (!currentId) return;
    const result = await commands.cancelGeneration(currentId);
    if (result.status === "error") {
      setStatus(`cancel failed: ${formatErr(result.error)}`);
    }
  }

  return (
    <>
      <div style={{ display: "flex", gap: "0.5rem", alignItems: "center", flexWrap: "wrap" }}>
        <select
          value={selectedId}
          onChange={(e) => setSelectedId(e.target.value)}
          disabled={loading || installed.length === 0}
          style={{ flex: "1 1 24rem", minWidth: "20rem", fontFamily: "monospace" }}
        >
          {installed.length === 0 && <option value="">(no installed models — download one first)</option>}
          {installed.map((m) => (
            <option key={m.id} value={m.id}>
              {m.id} — {m.filename}
            </option>
          ))}
        </select>
        <button onClick={refreshInstalled} disabled={loading} title="Refresh list">
          ↻
        </button>
        <button onClick={handleLoad} disabled={loading || !selectedId}>
          {loading ? "Loading..." : "Load model"}
        </button>
      </div>
      <p style={{ fontSize: "0.875rem", opacity: 0.75, marginTop: "0.25rem" }}>{loadStatus}</p>

      <div style={{ marginTop: "1rem" }}>
        <textarea
          placeholder="Prompt"
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          rows={3}
          style={{ width: "100%", fontFamily: "monospace" }}
          disabled={!modelLoaded}
        />
        <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.5rem" }}>
          <button
            onClick={handleGenerate}
            disabled={!modelLoaded || !prompt.trim() || currentId !== null}
          >
            Generate
          </button>
          {currentId !== null && (
            <button onClick={handleCancel}>Cancel</button>
          )}
          <span style={{ fontSize: "0.875rem", opacity: 0.75, alignSelf: "center" }}>
            {status}
          </span>
        </div>
      </div>

      <pre
        style={{
          marginTop: "1rem",
          padding: "0.75rem",
          background: "rgba(127,127,127,0.08)",
          border: "1px solid rgba(127,127,127,0.2)",
          borderRadius: "4px",
          minHeight: "10rem",
          maxHeight: "30rem",
          overflow: "auto",
          whiteSpace: "pre-wrap",
          fontFamily: "monospace",
          fontSize: "0.875rem",
        }}
      >
        {tokens || <span style={{ opacity: 0.4 }}>(output will stream here)</span>}
      </pre>
    </>
  );
}

function formatErr(e: CommandError): string {
  return `${e.kind}: ${e.message}`;
}

type DownloadPhase =
  | { kind: "idle" }
  | { kind: "downloading"; downloaded: number; total: number }
  | { kind: "verifying"; hashed: number; total: number }
  | { kind: "done"; final_path: string }
  | { kind: "failed"; message: string }
  | { kind: "cancelled" };

function pct(a: number, b: number): string {
  if (b === 0) return "0%";
  return `${Math.round((a / b) * 100)}%`;
}

function DownloadPanel() {
  const [catalog, setCatalog] = useState<ClassifiedCatalogResponse | null>(null);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [catalogErr, setCatalogErr] = useState<string | null>(null);

  // keyed by model_id
  const [phases, setPhases] = useState<Record<string, DownloadPhase>>({});
  const [activeId, setActiveId] = useState<string | null>(null);

  function setPhase(model_id: string, phase: DownloadPhase) {
    setPhases((p) => ({ ...p, [model_id]: phase }));
  }

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;

    events.downloadEvent
      .listen((event) => {
        const p: DownloadEvent = event.payload;
        switch (p.type) {
          case "started":
            setPhase(p.model_id, { kind: "downloading", downloaded: 0, total: p.total_bytes });
            setActiveId(p.model_id);
            break;
          case "progress":
            setPhase(p.model_id, { kind: "downloading", downloaded: p.downloaded_bytes, total: p.total_bytes });
            break;
          case "verifying":
            setPhase(p.model_id, { kind: "verifying", hashed: p.hashed_bytes, total: p.total_bytes });
            break;
          case "completed":
            setPhase(p.model_id, { kind: "done", final_path: p.final_path });
            setActiveId(null);
            break;
          case "failed":
            setPhase(p.model_id, { kind: "failed", message: `${p.kind}: ${p.message}` });
            setActiveId(null);
            break;
          case "cancelled":
            setPhase(p.model_id, { kind: "cancelled" });
            setActiveId(null);
            break;
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  async function loadCatalog() {
    setCatalogLoading(true);
    setCatalogErr(null);
    try {
      const r = await commands.fetchClassifiedCatalog();
      if (r.status === "ok") setCatalog(r.data);
      else setCatalogErr(formatErr(r.error));
    } catch (e) {
      setCatalogErr(e instanceof Error ? e.message : String(e));
    } finally {
      setCatalogLoading(false);
    }
  }

  async function startDownload(model_id: string) {
    // Set activeId immediately so other Download buttons disable before
    // the DownloadEvent::Started event arrives from the backend.
    setActiveId(model_id);
    setPhase(model_id, { kind: "downloading", downloaded: 0, total: 0 });
    const r = await commands.startModelDownload(model_id);
    if (r.status === "error") {
      setPhase(model_id, { kind: "failed", message: formatErr(r.error) });
      setActiveId(null);
    }
  }

  async function cancelDownload(model_id: string) {
    const r = await commands.cancelModelDownload(model_id);
    if (r.status === "error") {
      console.error("cancel failed:", formatErr(r.error));
    }
  }

  return (
    <>
      <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
        <button onClick={loadCatalog} disabled={catalogLoading}>
          {catalogLoading ? "Fetching..." : "Fetch catalog"}
        </button>
        {catalog && (
          <span style={{ fontSize: "0.875rem", opacity: 0.75 }}>
            {catalog.models.length} model(s)
          </span>
        )}
      </div>
      {catalogErr && <p role="alert">{catalogErr}</p>}
      {catalog &&
        catalog.models.map((cm) => {
          const phase: DownloadPhase = phases[cm.model.id] ?? { kind: "idle" };
          const isActive = activeId === cm.model.id;

          return (
            <div
              key={cm.model.id}
              style={{
                marginTop: "0.5rem",
                padding: "0.5rem",
                background: "rgba(127,127,127,0.08)",
                border: "1px solid rgba(127,127,127,0.2)",
                borderRadius: "4px",
                fontSize: "0.875rem",
              }}
            >
              <div style={{ display: "flex", gap: "0.5rem", alignItems: "center", flexWrap: "wrap" }}>
                <strong>{cm.model.name}</strong>
                <span style={{ opacity: 0.6 }}>
                  {(cm.model.size_bytes / 1_073_741_824).toFixed(2)} GB · {cm.tier}
                </span>

                {phase.kind === "idle" && (
                  <button
                    disabled={activeId !== null}
                    onClick={() => startDownload(cm.model.id)}
                  >
                    Download
                  </button>
                )}
                {(phase.kind === "downloading" || phase.kind === "verifying") && isActive && (
                  <button onClick={() => cancelDownload(cm.model.id)}>Cancel</button>
                )}
                {phase.kind === "done" && (
                  <span style={{ color: "#22c55e", fontWeight: 600 }}>✓ Downloaded</span>
                )}
                {phase.kind === "cancelled" && (
                  <button onClick={() => startDownload(cm.model.id)} disabled={activeId !== null}>
                    Resume
                  </button>
                )}
              </div>

              {phase.kind === "downloading" && phase.total > 0 && (
                <div style={{ marginTop: "0.35rem" }}>
                  <div style={{ fontSize: "0.75rem", opacity: 0.75, marginBottom: "0.2rem" }}>
                    Downloading {pct(phase.downloaded, phase.total)} (
                    {(phase.downloaded / 1_048_576).toFixed(1)} /{" "}
                    {(phase.total / 1_048_576).toFixed(1)} MB)
                  </div>
                  <progress value={phase.downloaded} max={phase.total} style={{ width: "100%" }} />
                </div>
              )}

              {phase.kind === "verifying" && (
                <div style={{ marginTop: "0.35rem" }}>
                  <div style={{ fontSize: "0.75rem", opacity: 0.75, marginBottom: "0.2rem" }}>
                    Verifying SHA-256 {pct(phase.hashed, phase.total)} (
                    {(phase.hashed / 1_048_576).toFixed(1)} /{" "}
                    {(phase.total / 1_048_576).toFixed(1)} MB)
                  </div>
                  <progress value={phase.hashed} max={phase.total} style={{ width: "100%" }} />
                </div>
              )}

              {phase.kind === "done" && (
                <div style={{ marginTop: "0.35rem", fontSize: "0.75rem", opacity: 0.6, fontFamily: "monospace" }}>
                  {phase.final_path}
                </div>
              )}

              {phase.kind === "failed" && (
                <p role="alert" style={{ marginTop: "0.35rem", fontSize: "0.75rem" }}>
                  {phase.message}
                </p>
              )}

              {phase.kind === "cancelled" && (
                <div style={{ marginTop: "0.35rem", fontSize: "0.75rem", opacity: 0.6 }}>
                  Cancelled — .part file preserved for resume
                </div>
              )}
            </div>
          );
        })}
    </>
  );
}

const TIER_COLORS: Record<string, string> = {
  Recommended: "#22c55e",
  Viable: "#3b82f6",
  Heavy: "#f59e0b",
  NotSupported: "#ef4444",
};

function CatalogPanel() {
  const [resp, setResp] = useState<ClassifiedCatalogResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function loadCatalog() {
    setLoading(true);
    setErr(null);
    try {
      const r = await commands.fetchClassifiedCatalog();
      if (r.status === "ok") setResp(r.data);
      else setErr(formatErr(r.error));
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
        <button onClick={loadCatalog} disabled={loading}>
          {loading ? "Fetching..." : "Fetch catalog"}
        </button>
        {resp && (
          <span style={{ fontSize: "0.875rem", opacity: 0.75 }}>
            {resp.source === "network" ? "Network" : "Cache"} · schema v{resp.catalog_schema_version} · {resp.models.length} model(s)
          </span>
        )}
      </div>
      {err && <p role="alert">{err}</p>}
      {resp && resp.models.map((cm) => (
        <div
          key={cm.model.id}
          style={{
            marginTop: "0.5rem",
            padding: "0.5rem",
            background: "rgba(127,127,127,0.08)",
            border: "1px solid rgba(127,127,127,0.2)",
            borderRadius: "4px",
            fontFamily: "monospace",
            fontSize: "0.75rem",
          }}
        >
          <div style={{ display: "flex", gap: "0.5rem", alignItems: "center", marginBottom: "0.25rem" }}>
            <strong>{cm.model.name}</strong>
            <span style={{ color: TIER_COLORS[cm.tier] ?? "inherit", fontWeight: 600 }}>
              {cm.tier}
            </span>
            {cm.gpu_offload && (
              <span style={{ color: "#a855f7", fontWeight: 600 }}>GPU</span>
            )}
          </div>
          <pre style={{ margin: 0, maxHeight: "12rem", overflow: "auto" }}>
            {JSON.stringify(cm.model, null, 2)}
          </pre>
        </div>
      ))}
    </>
  );
}

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [appInfoError, setAppInfoError] = useState<string | null>(null);

  useEffect(() => {
    commands.appInfo().then((result) => {
      if (result.status === "ok") setInfo(result.data);
      else setAppInfoError(`${result.error.kind}: ${result.error.message}`);
    });
  }, []);

  return (
    <main className="container">
      <h1>Abraxas</h1>
      <p>Tauri v2 scaffold — Fase 3.5 (token streaming + cancel).</p>
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

      <details style={{ marginTop: "2rem" }}>
        <summary style={{ cursor: "pointer", fontWeight: 600 }}>
          Hardware diagnostic (Fase 2.4)
        </summary>
        <div style={{ marginTop: "0.75rem" }}>
          <HardwarePanel />
        </div>
      </details>

      <details style={{ marginTop: "2rem" }}>
        <summary style={{ cursor: "pointer", fontWeight: 600 }}>
          Catalog (Fase 4.1)
        </summary>
        <div style={{ marginTop: "0.75rem" }}>
          <CatalogPanel />
        </div>
      </details>

      <details style={{ marginTop: "2rem" }}>
        <summary style={{ cursor: "pointer", fontWeight: 600 }}>
          Download (Fase 4.3 / 4.4 — progress + SHA-256 verify)
        </summary>
        <div style={{ marginTop: "0.75rem" }}>
          <DownloadPanel />
        </div>
      </details>

      <section style={{ marginTop: "2rem" }}>
        <h2 style={{ fontSize: "1.125rem", marginBottom: "0.5rem" }}>
          Inference (Fase 3.5 dev)
        </h2>
        <InferencePanel />
      </section>
    </main>
  );
}

export default App;
