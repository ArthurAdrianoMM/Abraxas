import { useEffect, useMemo, useState } from "react";
import fibonacci from "../../assets/fibonacci.gif";
import type { HardwareDetection } from "../../lib/tauri/bindings";
import type { Format } from "../../lib/format";
import { useFormat, useT, type Dict } from "../../lib/i18n";
import { useDiskStore } from "../../stores/disk";
import { useHardwareStore } from "../../stores/hardware";
import styles from "./CheckStep.module.css";

const ROW_DEFS = [
  { key: "ram", roman: "I." },
  { key: "cpu", roman: "II." },
  { key: "gpu", roman: "III." },
  { key: "vram", roman: "IV." },
  { key: "storage", roman: "V." },
] as const;

type RowKey = (typeof ROW_DEFS)[number]["key"];
type RowState = "ok" | "warn" | "fail";
type Row = { detail: string; state: RowState };

type Tier = "optimal" | "medium" | "low" | "error";

function buildRows(
  detection: HardwareDetection | null,
  disk: { free_bytes: number; total_bytes: number } | null,
  t: Dict["onboarding"]["check"],
  f: Format,
): Record<RowKey, Row> {
  /** VRAM arrives in mebibytes, not bytes, so it can't go through `f.gb`. */
  const gib = (mb: number) => f.decimal(mb / 1024, 1);

  if (!detection) {
    const fail: Row = { detail: t.interrupted, state: "fail" };
    return { ram: fail, cpu: fail, gpu: fail, vram: fail, storage: fail };
  }
  const { memory, cpu } = detection.system;
  const gpu = detection.gpu;

  const ram: Row = {
    detail: t.free(f.gb(memory.available_bytes, 1), f.gb(memory.total_bytes, 0)),
    state: memory.total_bytes >= 12e9 ? "ok" : "warn",
  };
  const cpuRow: Row = {
    detail: t.cores(cpu.brand.trim(), cpu.logical_cores),
    state: "ok",
  };

  let gpuRow: Row;
  let vramRow: Row;
  switch (gpu.kind) {
    case "metal":
      gpuRow = { detail: "Apple Silicon · Metal", state: "ok" };
      vramRow = { detail: t.unified, state: "ok" };
      break;
    case "cuda":
      gpuRow = { detail: `${gpu.name} · CUDA`, state: "ok" };
      vramRow = { detail: t.dedicated(gib(gpu.vram_mb)), state: "ok" };
      break;
    case "vulkan":
      gpuRow = { detail: `${gpu.name} · Vulkan`, state: "ok" };
      vramRow =
        gpu.vram_mb != null
          ? { detail: t.dedicated(gib(gpu.vram_mb)), state: "ok" }
          : { detail: t.shared, state: "warn" };
      break;
    default:
      gpuRow = { detail: t.noGpu, state: "warn" };
      vramRow = { detail: t.shared, state: "warn" };
  }

  const storage: Row =
    disk && disk.total_bytes > 0
      ? {
          detail: t.free(f.gb(disk.free_bytes, 0), f.gb(disk.total_bytes, 0)),
          state: disk.free_bytes >= 20e9 ? "ok" : "warn",
        }
      : { detail: t.diskUnreadable, state: "warn" };

  return { ram, cpu: cpuRow, gpu: gpuRow, vram: vramRow, storage };
}

/** Presentation tier for the verdict copy. The real per-model verdict is
 *  the backend's CompatibilityTier on the next step — this only sets the
 *  tone of the exame's conclusion. */
function tierFor(detection: HardwareDetection | null, failed: boolean): Tier {
  if (failed || !detection) return "error";
  const hasGpu = detection.choice.backend !== "cpu";
  const bigRam = detection.system.memory.total_bytes >= 16e9;
  if (hasGpu && bigRam) return "optimal";
  if (hasGpu || bigRam) return "medium";
  return "low";
}

/** Split-leaf hardware exam per "Abraxas Computer Check.html". Detection is
 *  fast; the ledger animation sets the pace, settling one row at a time and
 *  only revealing the verdict when the last entry lands. */
export function CheckStep({ onContinue, onSkip }: { onContinue: () => void; onSkip: () => void }) {
  const t = useT().onboarding.check;
  const f = useFormat();
  const detection = useHardwareStore((s) => s.detection);
  const hwError = useHardwareStore((s) => s.error);
  const initHardware = useHardwareStore((s) => s.init);
  const redetect = useHardwareStore((s) => s.redetect);
  const usage = useDiskStore((s) => s.usage);
  const refreshDisk = useDiskStore((s) => s.refresh);

  /** Rows settled so far (0..5); the verdict shows at 5. */
  const [revealed, setRevealed] = useState(0);
  const [scanId, setScanId] = useState(0);

  useEffect(() => {
    void initHardware();
    void refreshDisk();
  }, [initHardware, refreshDisk]);

  // Advance the ledger one row per beat, but never past what we can honestly
  // fill — the beat waits for detection (or its failure) to resolve.
  useEffect(() => {
    if (revealed >= ROW_DEFS.length) return;
    const timer = window.setInterval(() => {
      const hw = useHardwareStore.getState();
      if (hw.detection || hw.error) {
        setRevealed((r) => Math.min(ROW_DEFS.length, r + 1));
      }
    }, 640);
    return () => window.clearInterval(timer);
  }, [revealed, scanId]);

  const failed = !detection && hwError !== null;
  const rows = useMemo(() => buildRows(detection, usage, t, f), [detection, usage, t, f]);
  const done = revealed >= ROW_DEFS.length;
  const tier = tierFor(detection, failed);
  const verdict = t.verdicts[tier];
  const scanningRow = !done ? ROW_DEFS[revealed]?.key : null;

  const retry = () => {
    setRevealed(0);
    setScanId((n) => n + 1);
    void redetect();
    void refreshDisk();
  };

  return (
    <div className={styles.check}>
      {/* ============== LEFT LEAF — ceremony ============== */}
      <section className={styles.leafL}>
        <div className={styles.figure} aria-hidden="true">
          <img src={fibonacci} alt="" />
        </div>

        <div className={styles.stack}>
          <div className={styles.markRow}>
            <svg width="24" height="24" viewBox="0 0 32 32" fill="none" aria-hidden="true">
              <circle cx="16" cy="16" r="13" stroke="#b89968" strokeWidth="0.9" fill="none" />
              <circle cx="16" cy="16" r="9.5" stroke="#7d2233" strokeWidth="0.7" fill="none" />
              <line x1="16" y1="1.5" x2="16" y2="30.5" stroke="#b89968" strokeWidth="0.9" />
              <circle cx="16" cy="11" r="1.3" fill="#b89968" />
            </svg>
          </div>

          {!done ? (
            <div className={styles.pre}>
              <h1 className={styles.title}>
                <span>{t.titleLead}</span>
                <span className={styles.titleQuiet}>{t.titleQuiet}</span>
              </h1>
            </div>
          ) : (
            <div className={styles.verdict} data-tier={tier} aria-live="polite">
              <span className={styles.vTier}>
                <span className={styles.vBullet} />
                {verdict.tier}
              </span>
              <p className={styles.conclusion}>{verdict.main}</p>
              <span className={styles.vRule} />
              <p className={styles.gloss}>{verdict.gloss}</p>
            </div>
          )}

          {!done && (
            <div className={styles.whisper}>
              <span>
                {scanningRow ? t.scanning(t.rows[scanningRow]) : t.scanningIdle}
              </span>
              <span className={styles.barwrap} />
            </div>
          )}
        </div>
      </section>

      {/* ============== RIGHT LEAF — ledger ============== */}
      <section className={styles.leafR}>
        <div className={styles.ledger}>
          <div className={styles.head}>
            <span className={styles.kicker}>{t.ledger}</span>
          </div>

          <div className={styles.entries}>
            {ROW_DEFS.map((def, i) => {
              const state: "pending" | "scanning" | RowState =
                i < revealed ? rows[def.key].state : i === revealed && !done ? "scanning" : "pending";
              const detail =
                i < revealed ? rows[def.key].detail : i === revealed && !done ? t.reading : t.waiting;
              return (
                <div className={styles.entry} data-state={state} key={def.key}>
                  <span className={styles.num}>{def.roman}</span>
                  <span className={styles.mark}>
                    <span className={styles.dot} />
                  </span>
                  <div className={styles.body}>
                    <span className={styles.name}>{t.rows[def.key]}</span>
                    <span className={styles.detail}>{detail}</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        <div className={styles.actions}>
          <div className={styles.actionsLeft}>
            <button className={styles.ghostLink} disabled={!done} onClick={retry}>
              <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <path
                  d="M13 8a5 5 0 1 1-1.5-3.6M13 2v3h-3"
                  stroke="currentColor"
                  strokeWidth="1.3"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
              {t.retry}
            </button>
            {done && tier === "error" && (
              <button className={styles.ghostLink} onClick={onSkip}>
                {t.skip}
              </button>
            )}
          </div>

          <button
            className={styles.continueBtn}
            data-ready={done}
            disabled={!done}
            onClick={onContinue}
          >
            <span>{done ? verdict.cta : t.wait}</span>
            <span className={styles.arrow} aria-hidden="true">
              <svg width="13" height="13" viewBox="0 0 16 16" fill="none">
                <path
                  d="M3 8h10M9 4l4 4-4 4"
                  stroke="currentColor"
                  strokeWidth="1.4"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </span>
          </button>
        </div>
      </section>
    </div>
  );
}
