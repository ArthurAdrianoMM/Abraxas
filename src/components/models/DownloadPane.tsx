import { useEffect, useState } from "react";
import { useFormat, useT } from "../../lib/i18n";
import { useDiskStore } from "../../stores/disk";
import { useDownloadsStore } from "../../stores/downloads";
import { useModelStore } from "../../stores/model";
import { useUiStore } from "../../stores/ui";
import { ErrorAction, ErrorCard, ErrorLink } from "./ErrorCard";
import { SealDial } from "./SealDial";
import { StorageRow } from "./StorageRow";
import styles from "./DownloadPane.module.css";

export function DownloadPane() {
  const t = useT().downloadPane;
  const f = useFormat();
  const session = useDownloadsStore((s) => s.session);
  const start = useDownloadsStore((s) => s.start);
  const pause = useDownloadsStore((s) => s.pause);
  const reset = useDownloadsStore((s) => s.reset);
  const load = useModelStore((s) => s.load);
  const setModelsPane = useUiStore((s) => s.setModelsPane);

  const usage = useDiskStore((s) => s.usage);
  const refreshDisk = useDiskStore((s) => s.refresh);
  const [recoveryDismissed, setRecoveryDismissed] = useState(false);

  const phaseForEffects = session?.phase;
  useEffect(() => {
    void refreshDisk();
    // Free space shrinks while bytes land — keep the meter and the
    // disk-critical warning honest during an active download.
    if (phaseForEffects !== "downloading") return;
    const timer = setInterval(() => void refreshDisk(), 5000);
    return () => clearInterval(timer);
  }, [refreshDisk, phaseForEffects]);

  // Nothing to show — the user landed here without picking a model.
  useEffect(() => {
    if (!session) setModelsPane("catalog");
  }, [session, setModelsPane]);
  if (!session) return null;

  const { entry, phase, downloadedBytes, totalBytes, hashedBytes, speedBps, resumedFrom } = session;
  const model = entry.model;

  const backToCatalog = () => {
    reset();
    setModelsPane("catalog");
  };
  const abandon = () => {
    // Cancel on the backend if still moving; the `.part` stays for a resume.
    if (phase === "downloading" || phase === "starting") void pause();
    backToCatalog();
  };

  const pct =
    phase === "verifying"
      ? totalBytes > 0
        ? (hashedBytes / totalBytes) * 100
        : 0
      : totalBytes > 0
        ? (downloadedBytes / totalBytes) * 100
        : 0;

  const remaining = Math.max(0, totalBytes - downloadedBytes);
  const etaText =
    phase === "downloading"
      ? speedBps && speedBps > 0
        ? f.eta(remaining / speedBps)
        : t.estimating
      : phase === "verifying"
        ? t.underAMinute
        : phase === "paused"
          ? t.pausedShort
          : phase === "starting"
            ? t.estimating
            : "—";

  const origin = model.url.replace(/^https?:\/\//, "");

  return (
    <section className={styles.page}>
      {/* ============== LEFT — LEDGER ============== */}
      <div className={styles.leafL}>
        <div className={styles.ledgerStack}>
          <h1 className={styles.verb}>
            <em>{model.name}</em>
          </h1>

          <div className={styles.chosen}>
            <span className={styles.chosenWho}>
              <span className={styles.chosenName}>{model.name}</span>
              <span className={styles.chosenMono}>
                {model.publisher} · {model.params_b}b · {model.quantization.toLowerCase()}
              </span>
            </span>
            {phase === "confirm" && (
              <button className={styles.reverseLink} onClick={backToCatalog}>
                {t.change}
              </button>
            )}
          </div>

          <div className={styles.entries}>
            <div className={styles.entry}>
              <span className={styles.entryLabel}>{t.params}</span>
              <span className={styles.entryValue}>
                {model.params_b} B · {model.quantization}
              </span>
            </div>
            <div className={styles.entry}>
              <span className={styles.entryLabel}>{t.size}</span>
              <span className={styles.entryValue}>
                <b>{f.gb(model.size_bytes)} GB</b>
              </span>
            </div>
            <div className={styles.entry}>
              <span className={styles.entryLabel}>{t.time}</span>
              <span className={styles.entryValue}>
                {phase === "downloading" && speedBps
                  ? `${etaText} · ${f.mbps(speedBps)} mb/s`
                  : phase === "confirm"
                    ? t.estimatedOnStart
                    : etaText}
              </span>
            </div>
            <div className={styles.entry}>
              <span className={styles.entryLabel}>{t.origin}</span>
              <span className={`${styles.entryValue} ${styles.entryMono}`}>{origin}</span>
            </div>
          </div>

          <StorageRow
            modelsDir={usage?.models_dir ?? null}
            freeBytes={usage && usage.total_bytes > 0 ? usage.free_bytes : null}
            totalBytes={usage && usage.total_bytes > 0 ? usage.total_bytes : null}
            remainingBytes={phase === "confirm" ? model.size_bytes : remaining}
          />
        </div>
      </div>

      {/* ============== RIGHT — THE WORK ============== */}
      <div className={styles.leafR}>
        {resumedFrom != null && resumedFrom > 0 && !recoveryDismissed && phase !== "completed" && (
          <div className={styles.recovery} role="status" aria-live="polite">
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path
                d="M3 8a5 5 0 1 0 1.5-3.6M3 4v3h3"
                stroke="currentColor"
                strokeWidth="1.3"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            {t.resumedAt}{" "}
            <b>
              {f.gb(resumedFrom, 1)} GB / {f.gb(totalBytes)} GB
            </b>
            <button className={styles.recoveryDismiss} onClick={() => setRecoveryDismissed(true)}>
              {t.dismiss}
            </button>
          </div>
        )}

        {phase === "failed" ? (
          <DownloadFailure />
        ) : (
          <div className={styles.work}>
            <div className={styles.stateCaption} data-state={phase}>
              <span className={styles.stateBadge}>
                <span className={styles.stateGlyph} />
                {t.badge[phase]}
              </span>
              <span className={styles.verbLine}>{t.verb[phase]}</span>
            </div>

            {phase === "confirm" ? (
              <SealDial progress={0} state="confirm" armed>
                <div className="pct">
                  {f.gb(model.size_bytes)}
                  <span className="sym"> gb</span>
                </div>
                <div className="below">
                  <span>{t.toDescend}</span>
                </div>
              </SealDial>
            ) : phase === "completed" ? (
              <SealDial progress={100} state="complete" showCheck>
                <div className="below" style={{ marginTop: 62 }}>
                  <b>{f.gb(totalBytes)} gb</b> · {t.intact}
                </div>
              </SealDial>
            ) : (
              <SealDial
                progress={pct}
                state={
                  phase === "starting"
                    ? "connecting"
                    : (phase as "downloading" | "verifying" | "paused")
                }
              >
                <div className="pct">
                  {Math.max(0, Math.min(100, pct)).toFixed(0)}
                  <span className="sym"> %</span>
                </div>
                <div className="below">
                  <span>
                    <b>{f.size(phase === "verifying" ? hashedBytes : downloadedBytes)}</b>{" "}
                    {t.of} <b>{f.gb(totalBytes)} gb</b>
                  </span>
                </div>
              </SealDial>
            )}

            {phase !== "confirm" && phase !== "completed" && (
              <div className={styles.metrics}>
                <div className={`${styles.metric} ${phase !== "downloading" ? styles.metricDim : ""}`}>
                  <span className={styles.metricK}>{t.throughput}</span>
                  <span className={styles.metricV}>
                    {phase === "downloading" && speedBps ? f.mbps(speedBps) : "—"}{" "}
                    <span className={styles.metricSub}>mb/s</span>
                  </span>
                </div>
                <div className={styles.metric}>
                  <span className={styles.metricK}>{t.remaining}</span>
                  <span className={styles.metricV}>{etaText}</span>
                </div>
                <div className={styles.metric}>
                  <span className={styles.metricK}>{t.situationLabel}</span>
                  <span className={styles.metricV}>{t.situation[phase] ?? "—"}</span>
                </div>
              </div>
            )}

            {phase === "confirm" && (
              <div className={styles.actionsRow}>
                <button className={styles.btn} onClick={backToCatalog}>
                  <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path
                      d="M13 8H3M7 4 3 8l4 4"
                      stroke="currentColor"
                      strokeWidth="1.4"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                  <span>{t.backToCatalog}</span>
                </button>
                <button className={`${styles.btn} ${styles.btnPrimary}`} onClick={() => void start()}>
                  <span>{t.begin}</span>
                  <span className={styles.btnArrow} aria-hidden="true">
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
            )}

            {(phase === "starting" ||
              phase === "downloading" ||
              phase === "verifying" ||
              phase === "paused") && (
              <div className={styles.actionsRow}>
                <button
                  className={styles.btn}
                  disabled={phase !== "paused"}
                  title={
                    phase !== "paused"
                      ? t.pauseFirst
                      : t.returnsToCatalog
                  }
                  onClick={backToCatalog}
                >
                  <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                    <path
                      d="M13 8H3M7 4 3 8l4 4"
                      stroke="currentColor"
                      strokeWidth="1.4"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                  <span>{t.chooseAnother}</span>
                </button>
                <button
                  className={`${styles.btn} ${styles.btnWarn}`}
                  disabled={phase === "verifying"}
                  onClick={abandon}
                >
                  {t.cancel}
                </button>
                <button
                  className={`${styles.btn} ${styles.btnPrimary}`}
                  disabled={phase === "verifying" || phase === "starting"}
                  onClick={() => (phase === "paused" ? void start() : void pause())}
                >
                  <span>{phase === "paused" ? t.resume : t.pause}</span>
                </button>
              </div>
            )}

            {phase === "completed" && (
              <div className={styles.actionsRow}>
                <button className={styles.ghostLink} onClick={backToCatalog}>
                  {t.backToCatalog}
                </button>
                <button
                  className={`${styles.btn} ${styles.btnPrimary}`}
                  onClick={() => {
                    const id = model.id;
                    reset();
                    setModelsPane("manager");
                    void load(id, "ritual");
                  }}
                >
                  <span>{t.awaken}</span>
                  <span className={styles.btnArrow} aria-hidden="true">
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
            )}
          </div>
        )}
      </div>
    </section>
  );
}

/** Failure card variants inside the download spread (Error States II / VI). */
function DownloadFailure() {
  const t = useT().downloadPane;
  const f = useFormat();
  const session = useDownloadsStore((s) => s.session);
  const start = useDownloadsStore((s) => s.start);
  const reset = useDownloadsStore((s) => s.reset);
  const setModelsPane = useUiStore((s) => s.setModelsPane);

  if (!session) return null;
  const { entry, errorKind, errorMessage, downloadedBytes, totalBytes } = session;
  const pct = totalBytes > 0 ? Math.round((downloadedBytes / totalBytes) * 100) : 0;

  const discard = () => {
    reset();
    setModelsPane("catalog");
  };

  if (errorKind === "ChecksumMismatch") {
    return (
      <ErrorCard
        badge={t.checksumFailed.badge}
        code="err.integrity.sha256"
        title={t.checksumFailed.title}
        quiet={t.checksumFailed.quiet}
        gloss={t.checksumFailed.gloss}
        diag={[
          { k: t.checksumFailed.expected, v: entry.model.sha256.slice(0, 24) + "…" },
          { k: t.checksumFailed.file, v: t.checksumFailed.discarded, italic: true },
        ]}
        actions={
          <>
            <ErrorAction onClick={() => void start()}>
              {t.checksumFailed.redownload}
            </ErrorAction>
            <ErrorLink onClick={discard}>{t.backToCatalog}</ErrorLink>
          </>
        }
      />
    );
  }

  return (
    <ErrorCard
      badge={t.networkFailed.badge}
      code="err.download.network"
      title={t.networkFailed.title}
      quiet={pct > 0 ? t.networkFailed.quietProgress(pct) : t.networkFailed.quietNone}
      gloss={t.networkFailed.gloss(errorMessage ?? t.networkFailed.reason)}
      diag={[
        {
          k: t.networkFailed.progress,
          v: (
            <>
              <b>
                {f.gb(downloadedBytes)} / {f.gb(totalBytes)}
              </b>{" "}
              GB · {pct}%
            </>
          ),
        },
        { k: t.networkFailed.link, v: entry.model.url.replace(/^https?:\/\//, "") },
      ]}
      actions={
        <>
          <ErrorAction onClick={() => void start()}>
            {pct > 0 ? t.networkFailed.resumeFrom(pct) : t.networkFailed.retry}
          </ErrorAction>
          <ErrorLink onClick={discard}>{t.networkFailed.cancelDownload}</ErrorLink>
        </>
      }
      pulse
    />
  );
}
